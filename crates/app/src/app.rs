use anyhow::Result;
use egui::{Color32, Pos2, Rect, RichText, Sense, Vec2};
use kervesh_core::{
    Host, Rates, RemoteCapabilities, Settings, Snapshot, Store, bytes,
    secrets::{self, Credentials},
};
use kervesh_ssh::{
    Command, Event, FileOperation, RemoteEntry, Session, TransferRequest, TransferState,
};
use kervesh_terminal::{Terminal, TerminalFontConfig, TerminalFontManager};
use std::{
    collections::VecDeque,
    sync::{Arc, mpsc as std_mpsc},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::icons::{
    UiIcon, paint_arrow_down, paint_arrow_up, paint_progress_bar, paint_sparkline, render_monogram,
    ui_icon_label_button, ui_icon_texture,
};
use crate::theme::colors;

pub(crate) struct Tab {
    pub id: u64,
    pub host: Host,
    pub session: Session,
    pub terminal: Terminal,
    pub connected: bool,
    pub sftp_available: bool,
    pub follow_suspended: bool,
    pub last_followed: Option<String>,
    pub reveal_name: Option<String>,
    pub status: String,
    pub credentials: Credentials,
    pub retry_at: Option<Instant>,
    pub retries: u8,
    pub path: String,
    pub path_input: String,
    pub history: Vec<String>,
    pub entries: Vec<RemoteEntry>,
    pub filter: String,
    pub selected: Option<RemoteEntry>,
    pub busy: bool,
    pub snapshot: Option<Box<Snapshot>>,
    pub rates: Rates,
    pub capabilities: Option<RemoteCapabilities>,
    pub paused: bool,
    pub error: Option<String>,
    pub transfers: Vec<TransferRow>,
    pub editor: Option<EditorState>,
    pub connected_at: Option<String>,
    pub cpu_history: Vec<f32>,
    pub mem_history: Vec<f32>,
    pub net_rx_history: Vec<f32>,
    pub net_tx_history: Vec<f32>,
}

pub(crate) struct EditorState {
    pub path: String,
    pub name: String,
    pub content: String,
    pub dirty: bool,
    pub saving: bool,
    pub error: Option<String>,
}

pub(crate) struct TransferRow {
    pub request: TransferRequest,
    pub state: TransferState,
    pub done: u64,
    pub total: u64,
    pub speed: f64,
}

pub(crate) struct Login {
    pub host: Host,
    pub credentials: Credentials,
}

pub(crate) struct TrustPrompt {
    pub tab: u64,
    pub host: String,
    pub port: u16,
    pub fingerprint: String,
    pub reply: tokio::sync::oneshot::Sender<bool>,
}

pub(crate) enum Confirmation {
    DeleteHost(Host),
    CloseTab(u64),
    File(u64, FileOperation),
    Transfer(u64, TransferRequest),
    ForgetTrust(String, u16),
    Quit,
}

pub(crate) struct FileDialog {
    pub tab: u64,
    pub kind: FileDialogKind,
    pub name: String,
    pub error: Option<String>,
}

pub(crate) enum FileDialogKind {
    CreateFile,
    CreateDirectory,
    Rename(String),
    Permissions(String),
}

fn format_current_time() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let days = (now / 86400) as i64;
    let secs_of_day = now % 86400;
    let hours = secs_of_day / 3600;
    let mins = (secs_of_day % 3600) / 60;
    let secs = secs_of_day % 60;

    let z = days + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u32;
    let yoe = (doe - doe / 1020 + doe / 1461 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };

    const DAYS_OF_WEEK: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let w_idx = (days % 7) as usize;
    let m_idx = (m as usize).saturating_sub(1).min(11);
    format!(
        "{} {} {:2} {:02}:{:02}:{:02} {}",
        DAYS_OF_WEEK[w_idx], MONTHS[m_idx], d, hours, mins, secs, y
    )
}

pub struct App {
    pub(crate) store: Store,
    pub(crate) runtime: tokio::runtime::Runtime,
    pub(crate) settings: Settings,
    pub(crate) hosts: Vec<Host>,
    pub(crate) tabs: Vec<Tab>,
    pub(crate) active: usize,
    pub(crate) query: String,
    pub(crate) host_form: Option<Host>,
    pub(crate) form_error: Option<String>,
    pub(crate) login: Option<Login>,
    pub(crate) trust: VecDeque<TrustPrompt>,
    pub(crate) confirmation: Option<Confirmation>,
    pub(crate) file_dialog: Option<FileDialog>,
    pub(crate) settings_open: bool,
    pub(crate) inspector_open: bool,
    pub(crate) transfers_open: bool,
    pub(crate) notice: Option<String>,
    pub(crate) next_id: u64,
    pub(crate) secret_tx: std_mpsc::Sender<(String, Result<Option<zeroize::Zeroizing<String>>>)>,
    secret_rx: std_mpsc::Receiver<(String, Result<Option<zeroize::Zeroizing<String>>>)>,
    theme: Option<bool>,
    pub(crate) terminal_fonts: TerminalFontManager,
    pub(crate) allow_quit: bool,
}

impl App {
    pub fn new(store: Store, runtime: tokio::runtime::Runtime) -> Result<Self> {
        let settings = store.settings()?;
        let hosts = store.hosts()?;
        let (secret_tx, secret_rx) = std_mpsc::channel();
        Ok(Self {
            store,
            runtime,
            settings,
            hosts,
            tabs: Vec::new(),
            active: 0,
            query: String::new(),
            host_form: None,
            form_error: None,
            login: None,
            trust: VecDeque::new(),
            confirmation: None,
            file_dialog: None,
            settings_open: false,
            inspector_open: false,
            transfers_open: false,
            notice: None,
            next_id: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            secret_tx,
            secret_rx,
            theme: None,
            terminal_fonts: TerminalFontManager::default(),
            allow_quit: false,
        })
    }

    pub(crate) fn register_terminal_fonts(&mut self, ctx: &egui::Context) {
        let mut configs: Vec<_> = self
            .settings
            .terminal_profiles
            .iter()
            .map(TerminalFontConfig::from)
            .collect();
        configs.extend(
            self.tabs
                .iter()
                .map(|t| TerminalFontConfig::from(t.terminal.profile())),
        );
        self.terminal_fonts.register(ctx, &configs);
    }

    pub fn tab_count(&self) -> usize {
        self.tabs.len()
    }

    pub fn open_new_host(&mut self) {
        self.host_form = Some(Host::default());
        self.form_error = None;
    }

    pub(crate) fn refresh_hosts(&mut self) {
        match self.store.hosts() {
            Ok(hosts) => self.hosts = hosts,
            Err(e) => self.notice = Some(e.to_string()),
        }
    }

    pub(crate) fn begin_connect(&mut self, host: Host, ctx: &egui::Context) {
        if let Err(e) = host.validate() {
            self.form_error = Some(e.to_string());
            return;
        }
        let id = host.id.clone();
        let sender = self.secret_tx.clone();
        let wake = ctx.clone();
        self.login = Some(Login {
            host,
            credentials: Credentials::default(),
        });
        self.runtime.spawn_blocking(move || {
            let result = secrets::load(&id);
            let _ = sender.send((id, result));
            wake.request_repaint();
        });
    }

    pub(crate) fn connect(&mut self, login: Login, ctx: &egui::Context) {
        self.next_id += 1;
        let wake = ctx.clone();
        let session = Session::start(
            &self.runtime,
            login.host.clone(),
            login.credentials.clone(),
            self.store.clone(),
            self.settings.monitor_secs,
            Arc::new(move || wake.request_repaint()),
        );
        let profile = self
            .settings
            .terminal_profile(login.host.terminal_profile.as_deref())
            .clone();
        self.tabs.push(Tab {
            id: self.next_id,
            host: login.host,
            session,
            terminal: Terminal::with_profile(100, 30, profile),
            connected: false,
            sftp_available: false,
            follow_suspended: false,
            last_followed: None,
            reveal_name: None,
            status: "Connecting…".into(),
            credentials: login.credentials,
            retry_at: None,
            retries: 0,
            path: ".".into(),
            path_input: ".".into(),
            history: Vec::new(),
            entries: Vec::new(),
            filter: String::new(),
            selected: None,
            busy: true,
            snapshot: None,
            rates: Rates::default(),
            capabilities: None,
            paused: false,
            error: None,
            transfers: Vec::new(),
            editor: None,
            connected_at: None,
            cpu_history: Vec::new(),
            mem_history: Vec::new(),
            net_rx_history: Vec::new(),
            net_tx_history: Vec::new(),
        });
        self.active = self.tabs.len() - 1;
    }

    pub(crate) fn send(&mut self, id: u64, command: Command) {
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id)
            && tab.session.commands.try_send(command).is_err()
        {
            tab.error = Some("Session unavailable or command queue full; retry the action".into());
        }
    }

    fn pump(&mut self, ctx: &egui::Context) {
        while let Ok((id, result)) = self.secret_rx.try_recv() {
            if let Some(login) = &mut self.login
                && login.host.id == id
            {
                match result {
                    Ok(Some(secret)) if login.credentials.secret.is_empty() => {
                        login.credentials.secret = secret
                    }
                    Err(e) => self.notice = Some(e.to_string()),
                    _ => {}
                }
            }
        }
        let mut reload = false;
        for tab in &mut self.tabs {
            for _ in 0..128 {
                let Ok(event) = tab.session.events.try_recv() else {
                    break;
                };
                match event {
                    Event::Trust {
                        host,
                        port,
                        fingerprint,
                        reply,
                    } => self.trust.push_back(TrustPrompt {
                        tab: tab.id,
                        host,
                        port,
                        fingerprint,
                        reply,
                    }),
                    Event::Connected => {
                        tab.connected = true;
                        tab.status = "Connected".into();
                        tab.retry_at = None;
                        tab.retries = 0;
                        tab.connected_at = Some(format_current_time());
                        reload = true;
                    }
                    Event::Output(bytes) => {
                        tab.terminal.feed(&bytes);
                        let replies = tab.terminal.replies();
                        if !replies.is_empty() {
                            let _ = tab.session.commands.try_send(Command::Input(replies));
                        }
                    }
                    Event::Disconnected(reason) => {
                        if tab.connected && tab.host.auto_reconnect && tab.retries < 3 {
                            tab.retry_at = Some(Instant::now() + Duration::from_secs(3));
                        }
                        tab.connected = false;
                        tab.sftp_available = false;
                        tab.busy = false;
                        tab.status = reason;
                        for transfer in &mut tab.transfers {
                            if matches!(
                                transfer.state,
                                TransferState::Queued | TransferState::Running
                            ) {
                                transfer.request.cancel.cancel();
                                transfer.state = TransferState::Cancelled;
                            }
                        }
                    }
                    Event::Error(error) => {
                        if let Some(editor) = &mut tab.editor
                            && editor.saving
                        {
                            editor.saving = false;
                            editor.error = Some(error.clone());
                        }
                        tab.error = Some(error);
                        tab.busy = false;
                    }
                    Event::Capabilities(capabilities) => tab.capabilities = Some(capabilities),
                    Event::Metrics(snapshot, rates) => {
                        if let Some(cpu) = rates.cpu {
                            tab.cpu_history.push(cpu as f32);
                            if tab.cpu_history.len() > 30 {
                                tab.cpu_history.remove(0);
                            }
                        }
                        if let Some(used) = snapshot.memory_used() {
                            let total = snapshot.memory.get("MemTotal").copied().unwrap_or(1);
                            let pct = (used as f32 / total as f32) * 100.0;
                            tab.mem_history.push(pct);
                            if tab.mem_history.len() > 30 {
                                tab.mem_history.remove(0);
                            }
                        }
                        let (rx, tx) = rates
                            .network
                            .iter()
                            .filter(|(name, _)| name.as_str() != "lo")
                            .fold((0.0, 0.0), |(rx, tx), (_, n)| (rx + n.0, tx + n.1));
                        tab.net_rx_history.push(rx as f32);
                        if tab.net_rx_history.len() > 30 {
                            tab.net_rx_history.remove(0);
                        }
                        tab.net_tx_history.push(tx as f32);
                        if tab.net_tx_history.len() > 30 {
                            tab.net_tx_history.remove(0);
                        }
                        tab.snapshot = Some(snapshot);
                        tab.rates = rates;
                    }
                    Event::Directory { path, entries } => {
                        tab.sftp_available = true;
                        tab.path = path.clone();
                        tab.path_input = path;
                        tab.entries = entries;
                        tab.selected = tab
                            .reveal_name
                            .take()
                            .and_then(|name| tab.entries.iter().find(|e| e.name == name).cloned());
                        tab.busy = false;
                    }
                    Event::FileContent { path, content } => {
                        tab.editor = Some(EditorState {
                            name: path.rsplit('/').next().unwrap_or(&path).to_string(),
                            path,
                            content,
                            dirty: false,
                            saving: false,
                            error: None,
                        });
                        tab.busy = false;
                    }
                    Event::OperationComplete => {
                        if let Some(editor) = &mut tab.editor
                            && editor.saving
                        {
                            editor.saving = false;
                            editor.dirty = false;
                        }
                        let _ = tab
                            .session
                            .commands
                            .try_send(Command::File(FileOperation::List(tab.path.clone())));
                    }
                    Event::Transfer {
                        id,
                        done,
                        total,
                        speed,
                        state,
                    } => {
                        if let Some(row) = tab.transfers.iter_mut().find(|t| t.request.id == id) {
                            if done > 0 || matches!(state, TransferState::Running) {
                                row.done = done;
                                row.total = total;
                            }
                            row.speed = speed;
                            row.state = state;
                        }
                    }
                }
            }
            if tab.connected
                && tab.sftp_available
                && !tab.busy
                && !tab.follow_suspended
                && tab.terminal.profile().follow_terminal_directory
                && let Some(directory) = tab.terminal.directory()
                && !directory.host.is_empty()
                && (directory.host.eq_ignore_ascii_case(&tab.host.hostname)
                    || tab
                        .capabilities
                        .as_ref()
                        .is_some_and(|c| directory.host.eq_ignore_ascii_case(&c.hostname)))
                && tab.last_followed.as_deref() != Some(&directory.path)
            {
                let path = directory.path.clone();
                if path == tab.path {
                    tab.last_followed = Some(path);
                } else if tab
                    .session
                    .commands
                    .try_send(Command::File(FileOperation::List(path.clone())))
                    .is_ok()
                {
                    tab.last_followed = Some(path);
                    tab.busy = true;
                }
            }
            if !tab.session.events.is_empty() {
                ctx.request_repaint();
            }
            if let Some(at) = tab.retry_at {
                if Instant::now() >= at {
                    let wake = ctx.clone();
                    tab.session = Session::start(
                        &self.runtime,
                        tab.host.clone(),
                        tab.credentials.clone(),
                        self.store.clone(),
                        self.settings.monitor_secs,
                        Arc::new(move || wake.request_repaint()),
                    );
                    tab.retry_at = None;
                    tab.retries += 1;
                    tab.status = "Reconnecting…".into();
                    tab.paused = false;
                } else {
                    ctx.request_repaint_after(at.saturating_duration_since(Instant::now()));
                }
            }
        }
        self.trust.retain(|prompt| {
            self.tabs.iter().any(|t| t.id == prompt.tab) && !prompt.reply.is_closed()
        });
        if reload {
            self.refresh_hosts();
        }
    }

    pub fn render(&mut self, ctx: &egui::Context) {
        self.register_terminal_fonts(ctx);
        self.pump(ctx);
        let dark = self.settings.dark;

        if self.theme != Some(dark) {
            let mut visuals = if dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            };
            if dark {
                visuals.panel_fill = colors::DARK_PANEL;
                visuals.window_fill = colors::DARK_BG;
                visuals.extreme_bg_color = colors::DARK_BG;
                visuals.faint_bg_color = colors::DARK_PANEL_RAISED;
                visuals.selection.bg_fill = colors::DARK_BORDER;
                visuals.selection.stroke.color = colors::FOREGROUND;
                visuals.hyperlink_color = colors::FOREGROUND;
                visuals.widgets.noninteractive.bg_fill = colors::DARK_PANEL;
                visuals.widgets.noninteractive.bg_stroke.color = colors::DARK_BORDER;
                visuals.widgets.inactive.bg_fill = colors::DARK_PANEL_RAISED;
                visuals.widgets.hovered.bg_fill = colors::DARK_BORDER;
                visuals.widgets.active.bg_fill = colors::DARK_BORDER;
            } else {
                visuals.panel_fill = colors::LIGHT_PANEL;
                visuals.window_fill = colors::LIGHT_BG;
                visuals.extreme_bg_color = colors::LIGHT_PANEL;
                visuals.selection.bg_fill = colors::LIGHT_BORDER;
                visuals.selection.stroke.color = colors::LIGHT_FOREGROUND;
                visuals.hyperlink_color = colors::LIGHT_FOREGROUND;
            }
            visuals.window_corner_radius = egui::CornerRadius::same(6);
            visuals.menu_corner_radius = egui::CornerRadius::same(4);
            ctx.set_visuals(visuals);
            ctx.style_mut(|style| {
                style.spacing.item_spacing = egui::vec2(8.0, 8.0);
                style.spacing.button_padding = egui::vec2(8.0, 4.0);
            });
            self.theme = Some(dark);
        }

        // 1. Top Titlebar / Global Toolbar
        egui::TopBottomPanel::top("title")
            .exact_height(44.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let mark_color = if dark {
                        colors::FOREGROUND
                    } else {
                        colors::LIGHT_FOREGROUND
                    };
                    render_monogram(ui, 20.0, mark_color);
                    ui.add_space(2.0);
                    ui.label(RichText::new("Kervesh").size(17.0).strong());
                    ui.label(RichText::new("by Kernovae").size(11.0).weak());

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Window controls decoration
                        ui.add_space(4.0);
                        ui.label(RichText::new("✕").size(12.0).weak());
                        ui.add_space(6.0);
                        ui.label(RichText::new("□").size(12.0).weak());
                        ui.add_space(6.0);
                        ui.label(RichText::new("—").size(12.0).weak());
                        ui.add_space(16.0);

                        if ui_icon_label_button(ui, UiIcon::Settings, "Settings", dark).clicked() {
                            self.settings_open = true;
                        }
                        if ui_icon_label_button(ui, UiIcon::Monitor, "Monitor", dark).clicked() {
                            self.inspector_open = !self.inspector_open;
                        }
                        if ui_icon_label_button(ui, UiIcon::Sftp, "SFTP", dark).clicked() {
                            self.settings.sftp_panel = !self.settings.sftp_panel;
                            let _ = self.store.save_settings(&self.settings);
                        }
                        let split_btn = ui_icon_label_button(ui, UiIcon::Split, "Split", dark);
                        split_btn.on_hover_text("Planned for v0.3");

                        if ui_icon_label_button(ui, UiIcon::NewConnection, "New", dark).clicked() {
                            self.open_new_host();
                        }
                    });
                });
            });

        // 2. Left Side: Host connections sidebar
        if self.settings.sidebar {
            self.host_sidebar(ctx);
        }

        // 3. Right Side: SFTP file sidebar
        if self.settings.sftp_panel && !self.tabs.is_empty() {
            self.file_sidebar(ctx);
        }

        // 4. Bottom Area: Monitoring & Transfer Panel & Status Bar
        if self.transfers_open {
            self.transfer_panel(ctx);
        }

        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(24.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("Ready").small().weak());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        render_monogram(
                            ui,
                            14.0,
                            if dark {
                                colors::MUTED
                            } else {
                                colors::LIGHT_MUTED
                            },
                        );
                        ui.label(RichText::new("Kernovae").small().weak());
                        ui.separator();
                        ui.label(RichText::new("🔒").small().weak());
                        ui.separator();
                        ui.label(
                            RichText::new(format!(
                                "{} hosts  |  {} sessions",
                                self.hosts.len(),
                                self.tabs.len()
                            ))
                            .small()
                            .weak(),
                        );
                    });
                });
            });

        egui::TopBottomPanel::bottom("remote_monitoring")
            .min_height(90.0)
            .show(ctx, |ui| {
                self.monitoring_widget(ui);
            });

        // 5. Central Panel: Active session tab strip, overview card, terminal
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.inner_margin(6.0))
            .show(ctx, |ui| {
                if self.tabs.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space((ui.available_height() * 0.22).max(24.0));
                        let mark_color = if dark {
                            colors::FOREGROUND
                        } else {
                            colors::LIGHT_FOREGROUND
                        };
                        render_monogram(ui, 64.0, mark_color);
                        ui.add_space(16.0);
                        ui.label(
                            RichText::new("Your hosts. Your keys.\nYour machine.")
                                .size(28.0)
                                .strong(),
                        );
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("SSH, files and system health in one native workspace.")
                                .size(14.0)
                                .weak(),
                        );
                        ui.add_space(20.0);
                        if ui
                            .button(RichText::new("+  Add your first host").size(15.0))
                            .clicked()
                        {
                            self.open_new_host();
                        }
                        ui.add_space(20.0);
                        ui.label(
                            RichText::new("Local-first  ·  No account  ·  No cloud dependency")
                                .small()
                                .weak(),
                        );
                    });
                } else {
                    let mut close = None;

                    // Session Tab Strip
                    ui.horizontal(|ui| {
                        for (i, tab) in self.tabs.iter().enumerate() {
                            let selected = self.active == i;
                            let bg = if selected {
                                if dark {
                                    colors::DARK_PANEL_RAISED
                                } else {
                                    colors::LIGHT_PANEL_RAISED
                                }
                            } else {
                                if dark {
                                    colors::DARK_PANEL
                                } else {
                                    colors::LIGHT_PANEL
                                }
                            };
                            let border = if selected {
                                if dark {
                                    colors::DARK_BORDER
                                } else {
                                    colors::LIGHT_BORDER
                                }
                            } else {
                                Color32::TRANSPARENT
                            };

                            let (rect, response) =
                                ui.allocate_exact_size(Vec2::new(140.0, 28.0), Sense::click());
                            if ui.is_rect_visible(rect) {
                                ui.painter().rect(
                                    rect,
                                    4.0_f32,
                                    bg,
                                    egui::Stroke::new(1.0_f32, border),
                                    egui::StrokeKind::Inside,
                                );

                                // Dot
                                let dot_color = if tab.connected {
                                    colors::SUCCESS
                                } else {
                                    colors::DISCONNECTED
                                };
                                ui.painter().circle_filled(
                                    egui::pos2(rect.min.x + 12.0, rect.center().y),
                                    3.0_f32,
                                    dot_color,
                                );

                                // Tab title
                                let text_rect = Rect::from_min_max(
                                    egui::pos2(rect.min.x + 22.0, rect.min.y + 6.0),
                                    egui::pos2(rect.max.x - 20.0, rect.max.y - 6.0),
                                );
                                ui.painter().with_clip_rect(text_rect).text(
                                    text_rect.min,
                                    egui::Align2::LEFT_TOP,
                                    &tab.host.name,
                                    egui::FontId::proportional(12.0),
                                    if selected {
                                        colors::WHITE
                                    } else {
                                        colors::MUTED
                                    },
                                );

                                // Close button '×'
                                let close_rect = Rect::from_min_size(
                                    egui::pos2(rect.max.x - 18.0, rect.center().y - 8.0),
                                    Vec2::splat(16.0),
                                );
                                let close_resp = ui.allocate_rect(close_rect, Sense::click());
                                if close_resp.hovered() {
                                    ui.painter().rect_filled(close_rect, 2.0_f32, colors::SLATE);
                                }
                                ui.painter().text(
                                    close_rect.center(),
                                    egui::Align2::CENTER_CENTER,
                                    "×",
                                    egui::FontId::proportional(13.0),
                                    colors::MUTED,
                                );
                                if close_resp.clicked() {
                                    close = Some(tab.id);
                                }
                            }

                            if response.clicked() {
                                self.active = i;
                            }
                        }

                        if ui.button("+").on_hover_text("Open new session").clicked() {
                            self.open_new_host();
                        }
                    });

                    if let Some(id) = close {
                        self.confirmation = Some(Confirmation::CloseTab(id));
                    }
                    ui.separator();

                    self.active = self.active.min(self.tabs.len() - 1);
                    let mut reconnect = None;
                    let tab = &mut self.tabs[self.active];

                    // Active Session Subheader
                    ui.horizontal(|ui| {
                        let (icon_rect, _) =
                            ui.allocate_exact_size(Vec2::splat(16.0), Sense::hover());
                        let texture = ui_icon_texture(ui.ctx(), UiIcon::Terminal, dark);
                        ui.painter().image(
                            texture.id(),
                            icon_rect,
                            Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                            Color32::WHITE,
                        );

                        ui.label(
                            RichText::new(format!(
                                "{}@{}:{}",
                                tab.host.username, tab.host.hostname, tab.host.port
                            ))
                            .monospace()
                            .strong(),
                        );

                        // SSH Badge
                        let (badge_rect, _) =
                            ui.allocate_exact_size(Vec2::new(34.0, 18.0), Sense::hover());
                        ui.painter()
                            .rect_filled(badge_rect, 3.0_f32, colors::DARK_PANEL_RAISED);
                        ui.painter().text(
                            badge_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "SSH",
                            egui::FontId::proportional(11.0),
                            colors::MUTED,
                        );

                        if !tab.connected {
                            ui.label(&tab.status);
                            if ui.small_button("Reconnect").clicked() {
                                reconnect = Some(tab.host.clone());
                            }
                        } else {
                            if ui.small_button("Disconnect").clicked() {
                                tab.host.auto_reconnect = false;
                                let _ = tab.session.commands.try_send(Command::Close);
                            }

                            // Profile Selector in subheader
                            ui.separator();
                            let mut profile_id = tab.terminal.profile().id.clone();
                            egui::ComboBox::from_id_salt((tab.id, "terminal-profile"))
                                .selected_text(&tab.terminal.profile().name)
                                .show_ui(ui, |ui| {
                                    for profile in &self.settings.terminal_profiles {
                                        ui.selectable_value(
                                            &mut profile_id,
                                            profile.id.clone(),
                                            &profile.name,
                                        );
                                    }
                                });
                            if profile_id != tab.terminal.profile().id {
                                tab.terminal.set_profile(
                                    self.settings.terminal_profile(Some(&profile_id)).clone(),
                                );
                            }
                            if ui.small_button("Save profile to host").clicked() {
                                let mut host = tab.host.clone();
                                host.terminal_profile = Some(profile_id);
                                match self.store.save_host(&host) {
                                    Ok(()) => {
                                        tab.host = host.clone();
                                        if let Some(saved) =
                                            self.hosts.iter_mut().find(|h| h.id == host.id)
                                        {
                                            *saved = host;
                                        }
                                    }
                                    Err(e) => tab.error = Some(e.to_string()),
                                }
                            }
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let meta_str = if let Some(c) = &tab.capabilities {
                                format!("{}  |  {}  |  {}", tab.host.hostname, c.os, c.kernel)
                            } else {
                                format!("{}  |  Linux  |  6.12.101-1-amd64", tab.host.hostname)
                            };
                            ui.label(RichText::new(meta_str).small().weak());
                        });
                    });

                    if let Some(error) = tab.error.clone() {
                        ui.horizontal_wrapped(|ui| {
                            ui.colored_label(colors::WARNING, error);
                            if ui.small_button("Dismiss").clicked() {
                                tab.error = None;
                            }
                        });
                    }

                    ui.separator();

                    // Terminal Widget
                    let modal = self.host_form.is_some()
                        || self.login.is_some()
                        || !self.trust.is_empty()
                        || self.confirmation.is_some()
                        || self.file_dialog.is_some()
                        || self.settings_open
                        || self.inspector_open
                        || tab.editor.is_some();

                    ui.add_enabled_ui(tab.connected && !modal, |ui| {
                        let action = ui
                            .push_id(tab.id, |ui| tab.terminal.show(ui, tab.sftp_available))
                            .inner;
                        if let Some(path) = action.reveal_path {
                            tab.reveal_name = path.rsplit('/').next().map(str::to_owned);
                            let directory = path
                                .rsplit_once('/')
                                .map(|(p, _)| if p.is_empty() { "/" } else { p })
                                .unwrap_or("/")
                                .to_owned();
                            tab.follow_suspended = true;
                            if tab
                                .session
                                .commands
                                .try_send(Command::File(FileOperation::List(directory)))
                                .is_err()
                            {
                                tab.error = Some("SFTP queue unavailable".into());
                            }
                        }
                        if action.audio_bell {
                            self.runtime.spawn_blocking(crate::bell::play);
                        }
                        if let Some((cols, rows)) = action.resize {
                            let _ = tab.session.commands.try_send(Command::Resize(cols, rows));
                        }
                        if !action.input.is_empty()
                            && tab
                                .session
                                .commands
                                .try_send(Command::Input(action.input))
                                .is_err()
                        {
                            tab.error =
                                Some("Input queue full; terminal input was not sent".into());
                        }
                    });

                    if let Some(host) = reconnect {
                        self.begin_connect(host, ctx);
                    }
                }
            });

        self.host_dialog(ctx);
        self.login_dialog(ctx);
        self.trust_dialog(ctx);
        self.file_action_dialog(ctx);
        self.file_editor_window(ctx);
        self.confirm_dialog(ctx);
        self.settings_dialog(ctx);
        self.register_terminal_fonts(ctx);
        self.inspector(ctx);

        if let Some(notice) = self.notice.clone() {
            let mut open = true;
            egui::Window::new("Notice")
                .open(&mut open)
                .collapsible(false)
                .show(ctx, |ui| {
                    ui.label(notice);
                });
            if !open {
                self.notice = None;
            }
        }

        let dropped = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped.is_empty()
            && let Some(tab) = self.tabs.get(self.active)
            && tab.connected
        {
            let id = tab.id;
            for file in dropped {
                if let Some(path) = file.path {
                    self.prepare_upload(id, path);
                }
            }
        }
    }

    pub(crate) fn monitoring_widget(&mut self, ui: &mut egui::Ui) {
        let dark = self.settings.dark;
        let Some(tab) = self.tabs.get(self.active) else {
            ui.horizontal(|ui| {
                ui.label(RichText::new("Remote Monitoring").strong());
                ui.label(RichText::new("Connect to a host to view real-time telemetry").weak());
            });
            return;
        };

        ui.horizontal(|ui| {
            ui.label(RichText::new("Remote Monitoring").strong());
            ui.label(RichText::new(&tab.host.name).weak());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if tab.connected {
                    ui.colored_label(colors::SUCCESS, "● Live");
                } else {
                    ui.colored_label(colors::DISCONNECTED, "○ Offline");
                }
            });
        });
        ui.separator();

        let available_w = ui.available_width();
        let card_w = ((available_w - 24.0) / 4.0).max(100.0);
        let card_h = 60.0;

        ui.horizontal(|ui| {
            let spark_w = (card_w * 0.38).clamp(45.0, 90.0);

            // 1. CPU Card
            let (cpu_rect, _) = ui.allocate_exact_size(Vec2::new(card_w, card_h), Sense::hover());
            if ui.is_rect_visible(cpu_rect) {
                let bg = if dark {
                    colors::DARK_PANEL_RAISED
                } else {
                    colors::LIGHT_PANEL
                };
                let border = if dark {
                    colors::DARK_BORDER
                } else {
                    colors::LIGHT_BORDER
                };
                ui.painter().rect(
                    cpu_rect,
                    4.0_f32,
                    bg,
                    egui::Stroke::new(1.0_f32, border),
                    egui::StrokeKind::Inside,
                );

                let icon_r = Rect::from_min_size(
                    egui::pos2(cpu_rect.min.x + 8.0, cpu_rect.min.y + 7.0),
                    Vec2::splat(14.0),
                );
                let tex = ui_icon_texture(ui.ctx(), UiIcon::Inspector, dark);
                ui.painter().image(
                    tex.id(),
                    icon_r,
                    Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );

                ui.painter().text(
                    egui::pos2(cpu_rect.min.x + 26.0, cpu_rect.min.y + 6.0),
                    egui::Align2::LEFT_TOP,
                    "CPU",
                    egui::FontId::proportional(11.0),
                    colors::MUTED,
                );

                let cpu_str = tab
                    .rates
                    .cpu
                    .map(|c| format!("{c:.0}%"))
                    .unwrap_or_else(|| "0%".into());
                ui.painter().text(
                    egui::pos2(cpu_rect.min.x + 8.0, cpu_rect.min.y + 23.0),
                    egui::Align2::LEFT_TOP,
                    &cpu_str,
                    egui::FontId::proportional(16.0),
                    if dark {
                        colors::FOREGROUND
                    } else {
                        colors::LIGHT_FOREGROUND
                    },
                );

                let sub_str = if !tab.rates.cores.is_empty() {
                    format!("{} cores", tab.rates.cores.len())
                } else if let Some(s) = &tab.snapshot
                    && let Some(load) = s.load
                {
                    format!("load {:.2}", load[0])
                } else {
                    "idle".into()
                };
                ui.painter().text(
                    egui::pos2(cpu_rect.min.x + 8.0, cpu_rect.min.y + 42.0),
                    egui::Align2::LEFT_TOP,
                    sub_str,
                    egui::FontId::proportional(10.0),
                    colors::MUTED,
                );

                let spark_rect = Rect::from_min_max(
                    egui::pos2(cpu_rect.max.x - spark_w - 8.0, cpu_rect.min.y + 16.0),
                    egui::pos2(cpu_rect.max.x - 8.0, cpu_rect.max.y - 8.0),
                );
                paint_sparkline(
                    ui.painter(),
                    spark_rect,
                    &tab.cpu_history,
                    colors::FOREGROUND,
                );
            }

            // 2. Memory Card
            let (mem_rect, _) = ui.allocate_exact_size(Vec2::new(card_w, card_h), Sense::hover());
            if ui.is_rect_visible(mem_rect) {
                let bg = if dark {
                    colors::DARK_PANEL_RAISED
                } else {
                    colors::LIGHT_PANEL
                };
                let border = if dark {
                    colors::DARK_BORDER
                } else {
                    colors::LIGHT_BORDER
                };
                ui.painter().rect(
                    mem_rect,
                    4.0_f32,
                    bg,
                    egui::Stroke::new(1.0_f32, border),
                    egui::StrokeKind::Inside,
                );

                let icon_r = Rect::from_min_size(
                    egui::pos2(mem_rect.min.x + 8.0, mem_rect.min.y + 7.0),
                    Vec2::splat(14.0),
                );
                let tex = ui_icon_texture(ui.ctx(), UiIcon::Settings, dark);
                ui.painter().image(
                    tex.id(),
                    icon_r,
                    Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );

                ui.painter().text(
                    egui::pos2(mem_rect.min.x + 26.0, mem_rect.min.y + 6.0),
                    egui::Align2::LEFT_TOP,
                    "Memory",
                    egui::FontId::proportional(11.0),
                    colors::MUTED,
                );

                let (used_str, pct_str) = if let Some(s) = &tab.snapshot {
                    let used = s.memory_used().unwrap_or(0);
                    let total = s.memory.get("MemTotal").copied().unwrap_or(1);
                    let pct = (used as f32 / total as f32 * 100.0) as u32;
                    (
                        format!("{} / {}", bytes(used), bytes(total)),
                        format!("{pct}%"),
                    )
                } else {
                    ("- / -".into(), "0%".into())
                };

                ui.painter().text(
                    egui::pos2(mem_rect.min.x + 8.0, mem_rect.min.y + 23.0),
                    egui::Align2::LEFT_TOP,
                    &pct_str,
                    egui::FontId::proportional(16.0),
                    if dark {
                        colors::FOREGROUND
                    } else {
                        colors::LIGHT_FOREGROUND
                    },
                );

                ui.painter().text(
                    egui::pos2(mem_rect.min.x + 8.0, mem_rect.min.y + 42.0),
                    egui::Align2::LEFT_TOP,
                    &used_str,
                    egui::FontId::proportional(10.0),
                    colors::MUTED,
                );

                let spark_rect = Rect::from_min_max(
                    egui::pos2(mem_rect.max.x - spark_w - 8.0, mem_rect.min.y + 16.0),
                    egui::pos2(mem_rect.max.x - 8.0, mem_rect.max.y - 8.0),
                );
                paint_sparkline(
                    ui.painter(),
                    spark_rect,
                    &tab.mem_history,
                    colors::FOREGROUND,
                );
            }

            // 3. Disk Card
            let (disk_rect, _) = ui.allocate_exact_size(Vec2::new(card_w, card_h), Sense::hover());
            if ui.is_rect_visible(disk_rect) {
                let bg = if dark {
                    colors::DARK_PANEL_RAISED
                } else {
                    colors::LIGHT_PANEL
                };
                let border = if dark {
                    colors::DARK_BORDER
                } else {
                    colors::LIGHT_BORDER
                };
                ui.painter().rect(
                    disk_rect,
                    4.0_f32,
                    bg,
                    egui::Stroke::new(1.0_f32, border),
                    egui::StrokeKind::Inside,
                );

                let icon_r = Rect::from_min_size(
                    egui::pos2(disk_rect.min.x + 8.0, disk_rect.min.y + 7.0),
                    Vec2::splat(14.0),
                );
                let tex = ui_icon_texture(ui.ctx(), UiIcon::Files, dark);
                ui.painter().image(
                    tex.id(),
                    icon_r,
                    Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );

                ui.painter().text(
                    egui::pos2(disk_rect.min.x + 26.0, disk_rect.min.y + 6.0),
                    egui::Align2::LEFT_TOP,
                    "Disk /",
                    egui::FontId::proportional(11.0),
                    colors::MUTED,
                );

                let (disk_used_str, disk_pct_str, disk_pct_val) = if let Some(s) = &tab.snapshot
                    && let Some(fs) = s.filesystems.iter().find(|f| f.mount == "/")
                {
                    (
                        format!("{} / {}", bytes(fs.used), bytes(fs.used + fs.available)),
                        format!("{:.0}%", fs.percent),
                        fs.percent,
                    )
                } else {
                    ("- / -".into(), "0%".into(), 0.0)
                };

                ui.painter().text(
                    egui::pos2(disk_rect.min.x + 8.0, disk_rect.min.y + 23.0),
                    egui::Align2::LEFT_TOP,
                    &disk_pct_str,
                    egui::FontId::proportional(16.0),
                    if dark {
                        colors::FOREGROUND
                    } else {
                        colors::LIGHT_FOREGROUND
                    },
                );

                ui.painter().text(
                    egui::pos2(disk_rect.min.x + 8.0, disk_rect.min.y + 42.0),
                    egui::Align2::LEFT_TOP,
                    &disk_used_str,
                    egui::FontId::proportional(10.0),
                    colors::MUTED,
                );

                let bar_rect = Rect::from_min_max(
                    egui::pos2(disk_rect.max.x - spark_w - 8.0, disk_rect.min.y + 28.0),
                    egui::pos2(disk_rect.max.x - 8.0, disk_rect.min.y + 35.0),
                );
                paint_progress_bar(
                    ui.painter(),
                    bar_rect,
                    disk_pct_val,
                    if dark {
                        colors::DARK_BORDER
                    } else {
                        colors::LIGHT_BORDER
                    },
                    if disk_pct_val > 90.0 {
                        colors::DANGER
                    } else {
                        colors::SUCCESS
                    },
                );
            }

            // 4. Network Card
            let (net_rect, _) = ui.allocate_exact_size(Vec2::new(card_w, card_h), Sense::hover());
            if ui.is_rect_visible(net_rect) {
                let bg = if dark {
                    colors::DARK_PANEL_RAISED
                } else {
                    colors::LIGHT_PANEL
                };
                let border = if dark {
                    colors::DARK_BORDER
                } else {
                    colors::LIGHT_BORDER
                };
                ui.painter().rect(
                    net_rect,
                    4.0_f32,
                    bg,
                    egui::Stroke::new(1.0_f32, border),
                    egui::StrokeKind::Inside,
                );

                let icon_r = Rect::from_min_size(
                    egui::pos2(net_rect.min.x + 8.0, net_rect.min.y + 7.0),
                    Vec2::splat(14.0),
                );
                let tex = ui_icon_texture(ui.ctx(), UiIcon::Transfer, dark);
                ui.painter().image(
                    tex.id(),
                    icon_r,
                    Rect::from_min_max(Pos2::new(0.0, 0.0), Pos2::new(1.0, 1.0)),
                    Color32::WHITE,
                );

                ui.painter().text(
                    egui::pos2(net_rect.min.x + 26.0, net_rect.min.y + 6.0),
                    egui::Align2::LEFT_TOP,
                    "Network",
                    egui::FontId::proportional(11.0),
                    colors::MUTED,
                );

                let (rx, tx) = tab
                    .rates
                    .network
                    .iter()
                    .filter(|(name, _)| name.as_str() != "lo")
                    .fold((0.0, 0.0), |(rx, tx), (_, n)| (rx + n.0, tx + n.1));

                // Vector down arrow (RX)
                paint_arrow_down(
                    ui.painter(),
                    egui::pos2(net_rect.min.x + 8.0, net_rect.min.y + 26.0),
                    7.0_f32,
                    colors::SUCCESS,
                );
                ui.painter().text(
                    egui::pos2(net_rect.min.x + 19.0, net_rect.min.y + 23.0),
                    egui::Align2::LEFT_TOP,
                    format!("{}/s", bytes(rx as u64)),
                    egui::FontId::proportional(11.0),
                    if dark {
                        colors::FOREGROUND
                    } else {
                        colors::LIGHT_FOREGROUND
                    },
                );

                // Vector up arrow (TX)
                paint_arrow_up(
                    ui.painter(),
                    egui::pos2(net_rect.min.x + 8.0, net_rect.min.y + 42.0),
                    7.0_f32,
                    Color32::from_rgb(0x60, 0xA5, 0xFA),
                );
                ui.painter().text(
                    egui::pos2(net_rect.min.x + 19.0, net_rect.min.y + 39.0),
                    egui::Align2::LEFT_TOP,
                    format!("{}/s", bytes(tx as u64)),
                    egui::FontId::proportional(11.0),
                    colors::MUTED,
                );

                let spark_rect = Rect::from_min_max(
                    egui::pos2(net_rect.max.x - spark_w - 8.0, net_rect.min.y + 16.0),
                    egui::pos2(net_rect.max.x - 8.0, net_rect.max.y - 8.0),
                );
                paint_sparkline(
                    ui.painter(),
                    spark_rect,
                    &tab.net_rx_history,
                    colors::FOREGROUND,
                );
            }
        });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_quit
            && self.tabs.iter().any(|t| t.connected)
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.confirmation = Some(Confirmation::Quit);
        }
        self.render(ctx);
    }
}

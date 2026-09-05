use anyhow::Result;
use egui::RichText;
use kervesh_core::{
    Host, Rates, RemoteCapabilities, Settings, Snapshot, Store,
    secrets::{self, Credentials},
};
use kervesh_ssh::{
    Command, Event, FileOperation, RemoteEntry, Session, TransferRequest, TransferState,
};
use kervesh_terminal::Terminal;
use std::{
    collections::VecDeque,
    sync::{Arc, mpsc as std_mpsc},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

pub(crate) struct Tab {
    pub id: u64,
    pub host: Host,
    pub session: Session,
    pub terminal: Terminal,
    pub connected: bool,
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
            allow_quit: false,
        })
    }
    #[cfg(test)]
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
        self.tabs.push(Tab {
            id: self.next_id,
            host: login.host,
            session,
            terminal: Terminal::new(100, 30, self.settings.scrollback),
            connected: false,
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
                        tab.snapshot = Some(snapshot);
                        tab.rates = rates;
                    }
                    Event::Directory { path, entries } => {
                        tab.path = path.clone();
                        tab.path_input = path;
                        tab.entries = entries;
                        tab.selected = None;
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
        self.pump(ctx);
        if self.theme != Some(self.settings.dark) {
            let mut visuals = if self.settings.dark {
                egui::Visuals::dark()
            } else {
                egui::Visuals::light()
            };
            if self.settings.dark {
                visuals.panel_fill = crate::theme::colors::CHARCOAL;
                visuals.window_fill = crate::theme::colors::CHARCOAL;
                visuals.extreme_bg_color = crate::theme::colors::BLACK;
                visuals.faint_bg_color = crate::theme::colors::GRAPHITE;
                visuals.selection.bg_fill = crate::theme::colors::SLATE;
                visuals.selection.stroke.color = crate::theme::colors::FOREGROUND;
                visuals.hyperlink_color = crate::theme::colors::FOREGROUND;
                visuals.widgets.noninteractive.bg_fill = crate::theme::colors::CHARCOAL;
                visuals.widgets.noninteractive.bg_stroke.color = crate::theme::colors::GRAPHITE;
                visuals.widgets.inactive.bg_fill = crate::theme::colors::GRAPHITE;
                visuals.widgets.hovered.bg_fill = crate::theme::colors::SLATE;
                visuals.widgets.active.bg_fill = crate::theme::colors::SLATE;
            } else {
                visuals.panel_fill = crate::theme::colors::LIGHT_PANEL;
                visuals.window_fill = crate::theme::colors::LIGHT_BG;
                visuals.extreme_bg_color = crate::theme::colors::WHITE;
                visuals.selection.bg_fill = crate::theme::colors::LIGHT_BORDER;
                visuals.selection.stroke.color = crate::theme::colors::LIGHT_FOREGROUND;
                visuals.hyperlink_color = crate::theme::colors::LIGHT_FOREGROUND;
            }
            visuals.window_corner_radius = egui::CornerRadius::same(7);
            visuals.menu_corner_radius = egui::CornerRadius::same(6);
            ctx.set_visuals(visuals);
            ctx.style_mut(|style| {
                style.spacing.item_spacing = egui::vec2(8.0, 8.0);
                style.spacing.button_padding = egui::vec2(8.0, 4.0);
            });
            self.theme = Some(self.settings.dark);
        }
        egui::TopBottomPanel::top("title")
            .exact_height(48.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    let mark_color = if self.settings.dark {
                        crate::theme::colors::FOREGROUND
                    } else {
                        crate::theme::colors::LIGHT_FOREGROUND
                    };
                    crate::icons::render_monogram(ui, 20.0, mark_color);
                    ui.label(RichText::new("Kervesh").size(18.0).strong());
                    ui.label(RichText::new("by Kernovae").small().weak());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Settings").clicked() {
                            self.settings_open = true;
                        }
                        if ui
                            .selectable_label(self.settings.sftp_panel, "Files")
                            .clicked()
                        {
                            self.settings.sftp_panel = !self.settings.sftp_panel;
                            let _ = self.store.save_settings(&self.settings);
                        }
                        if ui
                            .selectable_label(self.settings.sidebar, "Hosts")
                            .clicked()
                        {
                            self.settings.sidebar = !self.settings.sidebar;
                            let _ = self.store.save_settings(&self.settings);
                        }
                        if ui
                            .selectable_label(self.transfers_open, "Transfers")
                            .clicked()
                        {
                            self.transfers_open = !self.transfers_open;
                        }
                    });
                });
            });
        if self.settings.sidebar {
            self.host_sidebar(ctx);
        }
        if self.settings.sftp_panel && !self.tabs.is_empty() {
            self.file_sidebar(ctx);
        }
        if self.transfers_open {
            self.transfer_panel(ctx);
        }
        egui::TopBottomPanel::bottom("health")
            .min_height(35.0)
            .show(ctx, |ui| {
                self.health_bar(ui);
            });
        egui::CentralPanel::default()
            .frame(egui::Frame::NONE.inner_margin(8.0))
            .show(ctx, |ui| {
                if self.tabs.is_empty() {
                    ui.vertical_centered(|ui| {
                        ui.add_space((ui.available_height() * 0.22).max(24.0));
                        let mark_color = if self.settings.dark {
                            crate::theme::colors::FOREGROUND
                        } else {
                            crate::theme::colors::LIGHT_FOREGROUND
                        };
                        crate::icons::render_monogram(ui, 56.0, mark_color);
                        ui.add_space(12.0);
                        ui.label(
                            RichText::new("Your hosts. Your keys.\nYour machine.")
                                .size(28.0)
                                .strong(),
                        );
                        ui.add_space(12.0);
                        ui.label("SSH, files and system health in one native workspace.");
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
                                .weak(),
                        );
                    });
                } else {
                    let mut close = None;
                    egui::ScrollArea::horizontal()
                        .id_salt("tabs")
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                for (i, tab) in self.tabs.iter().enumerate() {
                                    let selected = self.active == i;
                                    let resp = ui.selectable_label(
                                        selected,
                                        format!(
                                            "{}  {}",
                                            if tab.connected { "●" } else { "○" },
                                            tab.host.name
                                        ),
                                    );
                                    if resp.clicked() {
                                        self.active = i;
                                    }
                                    if ui
                                        .small_button("×")
                                        .on_hover_text("Close session")
                                        .clicked()
                                    {
                                        close = Some(tab.id);
                                    }
                                }
                                if ui.button("+").on_hover_text("Add host").clicked() {
                                    self.open_new_host();
                                }
                            });
                        });
                    if let Some(id) = close {
                        self.confirmation = Some(Confirmation::CloseTab(id));
                    }
                    ui.separator();
                    self.active = self.active.min(self.tabs.len() - 1);
                    let mut reconnect = None;
                    let tab = &mut self.tabs[self.active];
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(format!(
                                "{}@{}:{}",
                                tab.host.username, tab.host.hostname, tab.host.port
                            ))
                            .monospace()
                            .weak(),
                        );
                        if !tab.connected {
                            ui.label(&tab.status);
                            if ui.small_button("Reconnect").clicked() {
                                reconnect = Some(tab.host.clone());
                            }
                        } else if ui.small_button("Disconnect").clicked() {
                            tab.host.auto_reconnect = false;
                            let _ = tab.session.commands.try_send(Command::Close);
                        }
                    });
                    if let Some(error) = tab.error.clone() {
                        ui.horizontal_wrapped(|ui| {
                            ui.colored_label(crate::theme::colors::WARNING, error);
                            if ui.small_button("Dismiss").clicked() {
                                tab.error = None;
                            }
                        });
                    }
                    let modal = self.host_form.is_some()
                        || self.login.is_some()
                        || !self.trust.is_empty()
                        || self.confirmation.is_some()
                        || self.file_dialog.is_some()
                        || self.settings_open
                        || self.inspector_open
                        || tab.editor.is_some();
                    ui.add_enabled_ui(tab.connected && !modal, |ui| {
                        let action = tab.terminal.ui(ui, self.settings.font_size);
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

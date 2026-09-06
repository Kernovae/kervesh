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
    collections::{HashMap, VecDeque},
    sync::{Arc, mpsc as std_mpsc},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use crate::icons::{
    UiIcon, paint_arrow_down, paint_arrow_up, paint_progress_bar, paint_sparkline, render_monogram,
    ui_icon_label_button, ui_icon_texture,
};
use crate::theme::colors;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SplitMode {
    None,
    Vertical,
    Horizontal,
}

pub(crate) struct Tab {
    pub id: u64,
    pub host: Host,
    pub session: Session,
    pub terminal: Terminal,
    pub split_mode: SplitMode,
    pub split_ratio: f32,
    pub secondary_terminal: Option<Terminal>,
    pub active_pane: usize,
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
    pub forward: Vec<String>,
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
    pub editor: Option<crate::editor::RemoteEditor>,
    pub close_after_editor: bool,
    pub connected_at: Option<String>,
    pub alerts: Vec<kervesh_core::MonitorAlert>,
    pub cpu_history: Vec<f32>,
    pub mem_history: Vec<f32>,
    pub net_rx_history: Vec<f32>,
    pub net_tx_history: Vec<f32>,
    pub recorder: Option<kervesh_core::SessionRecorder>,
    pub cmd_buffer: String,
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
    DeleteHost(Box<Host>),
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

type TunnelStartResult = (
    String,
    u64,
    std::result::Result<kervesh_ssh::ActiveTunnel, String>,
);

fn finish_tunnel_attempt(starting: &mut HashMap<String, u64>, id: &str, attempt: u64) -> bool {
    if starting.get(id).copied() == Some(attempt) {
        starting.remove(id);
        true
    } else {
        false
    }
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
    pub(crate) process_view: crate::process_view::ProcessViewState,
    pub(crate) snippets_ui: crate::snippets_ui::SnippetsUiState,
    pub(crate) tunnels: Vec<kervesh_core::TunnelConfig>,
    pub(crate) tunnels_ui: crate::tunnels::TunnelsUi,
    pub(crate) tunnels_open: bool,
    pub(crate) active_tunnels: HashMap<String, kervesh_ssh::ActiveTunnel>,
    tunnel_start_tx: std_mpsc::Sender<TunnelStartResult>,
    tunnel_start_rx: std_mpsc::Receiver<TunnelStartResult>,
    tunnel_starting: HashMap<String, u64>,
    next_tunnel_attempt: u64,
    pub(crate) workspaces: Vec<kervesh_core::SessionWorkspace>,
    pub(crate) workspaces_ui: crate::workspaces_ui::WorkspacesUi,
    pub(crate) workspaces_open: bool,
    pub(crate) macros: Vec<kervesh_core::AutomationMacro>,
    pub(crate) automation_ui: crate::automation_ui::AutomationUi,
    pub(crate) automation_open: bool,
    pub(crate) search_ui: crate::search_ui::SearchUiState,
    pub(crate) sync_ui: crate::sync_ui::SyncUiState,
    pub(crate) devops_ui: crate::devops_ui::DevOpsUiState,
    pub(crate) audit_ui: crate::audit_ui::AuditUi,
    pub(crate) triggers_ui: crate::triggers_ui::TriggersUi,
    pub(crate) vault_ui: crate::vault_ui::VaultUi,
    pub(crate) theme_ui: crate::theme_ui::ThemeUi,
    pub(crate) trigger_engine: kervesh_core::TriggerEngine,
    pub(crate) trigger_rules: Vec<kervesh_core::TriggerRule>,
    pub(crate) selected_tag_filter: Option<String>,
}

impl App {
    pub fn new(store: Store, runtime: tokio::runtime::Runtime) -> Result<Self> {
        let settings = store.settings()?;
        let hosts = store.hosts()?;
        let tunnels = store.tunnels().unwrap_or_default();
        let workspaces = store.workspaces().unwrap_or_default();
        let macros = store.macros().unwrap_or_default();
        let trigger_rules = store.triggers().unwrap_or_default();
        let trigger_engine = kervesh_core::TriggerEngine::new(&trigger_rules);
        let (secret_tx, secret_rx) = std_mpsc::channel();
        let (tunnel_start_tx, tunnel_start_rx) = std_mpsc::channel();
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
            process_view: crate::process_view::ProcessViewState::default(),
            snippets_ui: crate::snippets_ui::SnippetsUiState::default(),
            tunnels,
            tunnels_ui: crate::tunnels::TunnelsUi::default(),
            tunnels_open: false,
            active_tunnels: HashMap::new(),
            tunnel_start_tx,
            tunnel_start_rx,
            tunnel_starting: HashMap::new(),
            next_tunnel_attempt: 0,
            workspaces,
            workspaces_ui: crate::workspaces_ui::WorkspacesUi::default(),
            workspaces_open: false,
            macros,
            automation_ui: crate::automation_ui::AutomationUi::default(),
            automation_open: false,
            search_ui: crate::search_ui::SearchUiState::default(),
            sync_ui: crate::sync_ui::SyncUiState::default(),
            devops_ui: crate::devops_ui::DevOpsUiState::default(),
            audit_ui: crate::audit_ui::AuditUi::default(),
            triggers_ui: crate::triggers_ui::TriggersUi::default(),
            vault_ui: crate::vault_ui::VaultUi::default(),
            theme_ui: crate::theme_ui::ThemeUi::default(),
            trigger_engine,
            trigger_rules,
            selected_tag_filter: None,
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
        self.refresh_tunnels();
        self.refresh_workspaces();
        self.refresh_macros();
    }

    pub(crate) fn refresh_tunnels(&mut self) {
        match self.store.tunnels() {
            Ok(tunnels) => self.tunnels = tunnels,
            Err(e) => self.notice = Some(e.to_string()),
        }
    }

    pub(crate) fn refresh_workspaces(&mut self) {
        match self.store.workspaces() {
            Ok(ws) => self.workspaces = ws,
            Err(e) => self.notice = Some(e.to_string()),
        }
    }

    pub(crate) fn refresh_macros(&mut self) {
        match self.store.macros() {
            Ok(macros) => self.macros = macros,
            Err(e) => self.notice = Some(e.to_string()),
        }
    }

    pub(crate) fn start_tunnel(&mut self, config: kervesh_core::TunnelConfig, ctx: &egui::Context) {
        if self.active_tunnels.contains_key(&config.id)
            || self.tunnel_starting.contains_key(&config.id)
        {
            return;
        }
        self.next_tunnel_attempt = self.next_tunnel_attempt.wrapping_add(1);
        let attempt = self.next_tunnel_attempt;
        self.tunnel_starting.insert(config.id.clone(), attempt);
        if config.kind != kervesh_core::TunnelKind::Remote
            && crate::tunnels::is_port_in_use(&config.bind_addr, config.bind_port)
        {
            self.tunnel_starting.remove(&config.id);
            self.tunnels_ui.error_message = Some(format!(
                "Port {} is already in use on {}",
                config.bind_port, config.bind_addr
            ));
            return;
        }

        let host = match self.hosts.iter().find(|h| h.id == config.host_id).cloned() {
            Some(h) => h,
            None => {
                self.tunnel_starting.remove(&config.id);
                self.tunnels_ui.error_message = Some("Host profile not found".into());
                return;
            }
        };

        let store = self.store.clone();
        let (tx, _rx) = tokio::sync::mpsc::channel(32);
        let wake = ctx.clone();
        let result_wake = ctx.clone();
        let sink = kervesh_ssh::EventSink::new(tx, Arc::new(move || wake.request_repaint()));
        let tid = config.id.clone();
        let cfg = config.clone();
        let result_tx = self.tunnel_start_tx.clone();
        self.runtime.spawn(async move {
            let host_id = host.id.clone();
            let secret = match tokio::task::spawn_blocking(move || {
                secrets::load(&host_id).ok().flatten().unwrap_or_default()
            })
            .await
            {
                Ok(secret) => secret,
                Err(error) => {
                    let _ = result_tx.send((
                        tid,
                        attempt,
                        Err(format!("Credential lookup failed: {error}")),
                    ));
                    result_wake.request_repaint();
                    return;
                }
            };
            let credentials = Credentials {
                secret,
                remember: false,
            };
            let result =
                kervesh_ssh::ActiveTunnel::start_for_host(&host, &credentials, store, sink, cfg)
                    .await
                    .map_err(|e| format!("{e:#}"));
            let _ = result_tx.send((tid, attempt, result));
            result_wake.request_repaint();
        });
    }

    pub(crate) fn stop_tunnel(&mut self, id: &str) {
        self.tunnel_starting.remove(id);
        if let Some(tunnel) = self.active_tunnels.remove(id) {
            tunnel.stop();
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
            split_mode: SplitMode::None,
            split_ratio: 0.5,
            secondary_terminal: None,
            active_pane: 0,
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
            forward: Vec::new(),
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
            close_after_editor: false,
            connected_at: None,
            alerts: Vec::new(),
            cpu_history: Vec::new(),
            mem_history: Vec::new(),
            net_rx_history: Vec::new(),
            net_tx_history: Vec::new(),
            recorder: None,
            cmd_buffer: String::new(),
        });
        self.active = self.tabs.len() - 1;
    }

    pub(crate) fn send(&mut self, id: u64, command: Command) {
        let write = match &command {
            Command::File(FileOperation::Write(path, _, operation_id)) => {
                Some((path.clone(), *operation_id))
            }
            _ => None,
        };
        if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == id)
            && tab.session.commands.try_send(command).is_err()
        {
            if let Some((path, operation_id)) = write
                && let Some(editor) = &mut tab.editor
            {
                editor.fail_save(
                    &path,
                    operation_id,
                    "Session unavailable or command queue full".into(),
                );
            }
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
        while let Ok((id, attempt, result)) = self.tunnel_start_rx.try_recv() {
            if finish_tunnel_attempt(&mut self.tunnel_starting, &id, attempt) {
                match result {
                    Ok(active) => {
                        self.active_tunnels.insert(id, active);
                    }
                    Err(error) => {
                        self.tunnels_ui.error_message =
                            Some(format!("Failed starting tunnel: {error}"));
                    }
                }
            } else if let Ok(active) = result {
                // A stale attempt must never replace a newer Start request.
                active.stop();
            }
        }
        let mut reload = false;
        let mut auto_start_tunnels = Vec::new();
        let mut close_tabs = Vec::new();
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
                        for tunnel in &self.tunnels {
                            if tunnel.auto_start
                                && tunnel.host_id == tab.host.id
                                && !self.active_tunnels.contains_key(&tunnel.id)
                            {
                                auto_start_tunnels.push(tunnel.clone());
                            }
                        }
                        for mac in &self.macros {
                            if mac.run_on_connect
                                && (mac.host_id.as_deref() == Some(&tab.host.id)
                                    || mac.host_id.is_none())
                            {
                                let _ = tab
                                    .session
                                    .commands
                                    .try_send(Command::RunMacro(mac.clone()));
                            }
                        }
                    }
                    Event::Output(bytes) => {
                        if let Some(rec) = &mut tab.recorder {
                            let _ = rec.write_output(&bytes);
                        }
                        let text = String::from_utf8_lossy(&bytes);
                        let trigger_actions =
                            self.trigger_engine.evaluate(&text, Some(&tab.host.id));
                        for act in trigger_actions {
                            match act {
                                kervesh_core::TriggerAction::Notification(msg) => {
                                    self.notice = Some(msg);
                                }
                                kervesh_core::TriggerAction::SendInput(inp) => {
                                    let _ = tab
                                        .session
                                        .commands
                                        .try_send(Command::Input(inp.into_bytes()));
                                }
                                kervesh_core::TriggerAction::Highlight(_) => {}
                                kervesh_core::TriggerAction::PlayBeep => {
                                    self.runtime.spawn_blocking(crate::bell::play);
                                }
                            }
                        }
                        tab.terminal.feed(&bytes);
                        if let Some(sec) = &mut tab.secondary_terminal {
                            sec.feed(&bytes);
                        }
                        let replies = tab.terminal.replies();
                        if !replies.is_empty() {
                            let _ = tab.session.commands.try_send(Command::Input(replies));
                        }
                    }
                    Event::Disconnected(reason) => {
                        if let Some(editor) = &mut tab.editor
                            && let Some((path, operation_id)) = editor
                                .pending_save()
                                .map(|(path, operation_id)| (path.to_owned(), operation_id))
                        {
                            editor.fail_save(&path, operation_id, reason.clone());
                        }
                        tab.close_after_editor = false;
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
                        tab.error = Some(error);
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
                        let thresholds = tab
                            .host
                            .thresholds
                            .unwrap_or(self.settings.default_thresholds);
                        tab.alerts = snapshot.check_alerts(&rates, &thresholds);
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
                        if tab.editor.as_ref().is_some_and(|editor| editor.dirty) {
                            tab.error = Some("Editor has unsaved changes; save or discard before opening another file".into());
                        } else {
                            tab.editor = Some(crate::editor::RemoteEditor::new(path, content));
                        }
                        tab.busy = false;
                    }
                    Event::FileWriteComplete { path, operation_id } => {
                        let close_editor = if let Some(editor) = &mut tab.editor {
                            editor.complete_save(&path, operation_id)
                                && editor.take_close_after_save()
                        } else {
                            false
                        };
                        let close_tab = close_editor && tab.close_after_editor;
                        if close_editor {
                            tab.editor = None;
                        }
                        if close_tab {
                            tab.close_after_editor = false;
                            close_tabs.push(tab.id);
                        } else {
                            let _ = tab
                                .session
                                .commands
                                .try_send(Command::File(FileOperation::List(tab.path.clone())));
                        }
                    }
                    Event::FileWriteError {
                        path,
                        operation_id,
                        error,
                    } => {
                        if let Some(editor) = &mut tab.editor {
                            if !editor.fail_save(&path, operation_id, error.clone()) {
                                tab.error = Some(format!("SFTP write failed: {error}"));
                            } else {
                                tab.close_after_editor = false;
                            }
                        } else {
                            tab.error = Some(format!("SFTP write failed: {error}"));
                        }
                    }
                    Event::Processes(processes) => {
                        self.process_view.processes = processes;
                        self.process_view.loading = false;
                    }
                    Event::ProcessSignalled {
                        pid,
                        signal,
                        success,
                        error,
                    } => {
                        if success {
                            self.notice =
                                Some(format!("Signal {} sent to PID {}", signal.as_str(), pid));
                            let _ = tab.session.commands.try_send(Command::ProcessList);
                        } else if let Some(err) = error {
                            self.notice = Some(format!("Failed to signal PID {}: {}", pid, err));
                        }
                    }
                    Event::MacroStatus { done, error, .. } => {
                        if let Some(err) = error {
                            self.notice = Some(format!("Macro sequence error: {err}"));
                        } else if done {
                            self.notice = Some("Automation macro completed successfully".into());
                        }
                    }
                    Event::SearchResults(results) => {
                        self.search_ui.set_results(results);
                    }
                    Event::SyncPlanReady(plan) => {
                        self.sync_ui.set_plan(plan);
                    }
                    Event::DockerContainers(c) => {
                        self.devops_ui.containers = c;
                    }
                    Event::DockerImages(img) => {
                        self.devops_ui.images = img;
                    }
                    Event::DockerLogs { id, logs } => {
                        self.devops_ui.docker_logs_id = Some(id);
                        self.devops_ui.docker_logs = Some(logs);
                    }
                    Event::SystemdUnits(u) => {
                        self.devops_ui.units = u;
                    }
                    Event::SystemdLogs { unit, logs } => {
                        self.devops_ui.systemd_logs_unit = Some(unit);
                        self.devops_ui.systemd_logs = Some(logs);
                    }
                    Event::NetDiagResult(res) => {
                        self.devops_ui.diag_running = false;
                        self.devops_ui.diag_results.push(res);
                    }
                    Event::OperationComplete => {
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
        for id in close_tabs {
            if let Some(index) = self.tabs.iter().position(|tab| tab.id == id)
                && self.tabs[index].editor.is_none()
            {
                for transfer in &self.tabs[index].transfers {
                    transfer.request.cancel.cancel();
                }
                self.tabs.remove(index);
            }
        }
        self.active = self.active.min(self.tabs.len().saturating_sub(1));
        for tunnel in auto_start_tunnels {
            self.start_tunnel(tunnel, ctx);
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

        // Global Keybindings Dispatcher
        let is_modal_open = self.host_form.is_some()
            || self.login.is_some()
            || !self.trust.is_empty()
            || self.confirmation.is_some()
            || self.file_dialog.is_some()
            || self.settings_open
            || self.inspector_open
            || self.process_view.open
            || self.snippets_ui.manager_open
            || self.snippets_ui.run_dialog.is_some()
            || self
                .tabs
                .get(self.active)
                .is_some_and(|t| t.editor.is_some());

        if !is_modal_open {
            let key_match = |shortcut: &str| -> bool {
                let parts: Vec<&str> = shortcut.split('+').collect();
                let mut ctrl = false;
                let mut alt = false;
                let mut shift = false;
                let mut key = None;
                for p in parts {
                    let clean = p.trim().to_lowercase();
                    if clean == "ctrl" || clean == "cmd" || clean == "command" {
                        ctrl = true;
                    } else if clean == "alt" || clean == "opt" || clean == "option" {
                        alt = true;
                    } else if clean == "shift" {
                        shift = true;
                    } else if clean == "d" {
                        key = Some(egui::Key::D);
                    } else if clean == "e" {
                        key = Some(egui::Key::E);
                    } else if clean == "w" {
                        key = Some(egui::Key::W);
                    } else if clean == "p" {
                        key = Some(egui::Key::P);
                    } else if clean == "k" {
                        key = Some(egui::Key::K);
                    } else if clean == "b" {
                        key = Some(egui::Key::B);
                    } else if clean == "f" {
                        key = Some(egui::Key::F);
                    } else if clean == "t" {
                        key = Some(egui::Key::T);
                    } else if clean == "pagedown" {
                        key = Some(egui::Key::PageDown);
                    } else if clean == "pageup" {
                        key = Some(egui::Key::PageUp);
                    } else if clean == "right" {
                        key = Some(egui::Key::ArrowRight);
                    } else if clean == "left" {
                        key = Some(egui::Key::ArrowLeft);
                    }
                }
                if let Some(k) = key {
                    ctx.input(|i| {
                        let m = i.modifiers;
                        (m.command == ctrl)
                            && (m.alt == alt)
                            && (m.shift == shift)
                            && i.key_pressed(k)
                    })
                } else {
                    false
                }
            };

            if key_match(
                self.settings
                    .keybindings
                    .get_shortcut(kervesh_core::KeyAction::ProcessViewer),
            ) {
                self.process_view.open = !self.process_view.open;
                if self.process_view.open
                    && let Some(tab) = self.tabs.get(self.active)
                {
                    self.process_view.loading = true;
                    let _ = tab.session.commands.try_send(Command::ProcessList);
                }
            }
            if key_match(
                self.settings
                    .keybindings
                    .get_shortcut(kervesh_core::KeyAction::SnippetsLibrary),
            ) {
                self.snippets_ui.manager_open = !self.snippets_ui.manager_open;
            }
            if key_match(
                self.settings
                    .keybindings
                    .get_shortcut(kervesh_core::KeyAction::MultiExec),
            ) {
                self.snippets_ui.broadcast_mode = !self.snippets_ui.broadcast_mode;
            }
            if key_match(
                self.settings
                    .keybindings
                    .get_shortcut(kervesh_core::KeyAction::SplitVertical),
            ) && let Some(tab) = self.tabs.get_mut(self.active)
            {
                tab.split_mode = SplitMode::Vertical;
                if tab.secondary_terminal.is_none() {
                    tab.secondary_terminal = Some(Terminal::with_profile(
                        100,
                        30,
                        tab.terminal.profile().clone(),
                    ));
                }
            }
            if key_match(
                self.settings
                    .keybindings
                    .get_shortcut(kervesh_core::KeyAction::SplitHorizontal),
            ) && let Some(tab) = self.tabs.get_mut(self.active)
            {
                tab.split_mode = SplitMode::Horizontal;
                if tab.secondary_terminal.is_none() {
                    tab.secondary_terminal = Some(Terminal::with_profile(
                        100,
                        30,
                        tab.terminal.profile().clone(),
                    ));
                }
            }
            if key_match(
                self.settings
                    .keybindings
                    .get_shortcut(kervesh_core::KeyAction::ClosePane),
            ) && let Some(tab) = self.tabs.get_mut(self.active)
            {
                tab.split_mode = SplitMode::None;
                tab.secondary_terminal = None;
                tab.active_pane = 0;
            }
            if key_match(
                self.settings
                    .keybindings
                    .get_shortcut(kervesh_core::KeyAction::NewSession),
            ) {
                self.open_new_host();
            }
            if key_match(
                self.settings
                    .keybindings
                    .get_shortcut(kervesh_core::KeyAction::NextTab),
            ) && !self.tabs.is_empty()
            {
                self.active = (self.active + 1) % self.tabs.len();
            }
            if key_match(
                self.settings
                    .keybindings
                    .get_shortcut(kervesh_core::KeyAction::PrevTab),
            ) && !self.tabs.is_empty()
            {
                self.active = (self.active + self.tabs.len() - 1) % self.tabs.len();
            }
        }

        // 1. Top Titlebar / Global Toolbar
        egui::TopBottomPanel::top("title")
            .exact_height(44.0)
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    render_monogram(ui, 20.0, dark);
                    ui.add_space(2.0);
                    ui.label(RichText::new("Kervesh").size(17.0).strong());
                    ui.label(RichText::new("by Kernovae").size(11.0).weak());

                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
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
                    let workspace_status = self
                        .tabs
                        .get(self.active)
                        .map(|tab| tab.status.as_str())
                        .unwrap_or("Unknown");
                    ui.label(RichText::new(workspace_status).small().weak());
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        render_monogram(ui, 14.0, dark);
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
                        render_monogram(ui, 64.0, dark);
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
                                        if dark {
                                            colors::FOREGROUND
                                        } else {
                                            colors::LIGHT_FOREGROUND
                                        }
                                    } else if dark {
                                        colors::MUTED
                                    } else {
                                        colors::LIGHT_MUTED
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
                        if let Some(index) = self.tabs.iter().position(|tab| tab.id == id)
                            && self.tabs[index]
                                .editor
                                .as_ref()
                                .is_some_and(|editor| editor.dirty)
                        {
                            self.active = index;
                            self.tabs[index].close_after_editor = true;
                            if let Some(editor) = &mut self.tabs[index].editor {
                                editor.close_prompt = true;
                            }
                        } else {
                            self.confirmation = Some(Confirmation::CloseTab(id));
                        }
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
                        ui.painter().rect_filled(
                            badge_rect,
                            3.0_f32,
                            if dark {
                                colors::DARK_PANEL_RAISED
                            } else {
                                colors::LIGHT_PANEL_RAISED
                            },
                        );
                        ui.painter().text(
                            badge_rect.center(),
                            egui::Align2::CENTER_CENTER,
                            "SSH",
                            egui::FontId::proportional(11.0),
                            colors::MUTED,
                        );

                        if !tab.alerts.is_empty() {
                            let alert_text = format!(
                                "⚠ {} Alert{}",
                                tab.alerts.len(),
                                if tab.alerts.len() > 1 { "s" } else { "" }
                            );
                            ui.colored_label(colors::WARNING, alert_text)
                                .on_hover_ui(|ui| {
                                    for a in &tab.alerts {
                                        ui.label(format!("{}: {}", a.metric, a.message));
                                    }
                                });
                        }

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
                            if ui.small_button("Save profile").clicked() {
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
                            if ui.small_button("🎨 Theme").on_hover_text("Open visual theme engine & ANSI palette editor").clicked() {
                                self.theme_ui.open_for_profile(tab.terminal.profile());
                            }

                            ui.separator();
                            if ui.small_button("⚡ Processes").on_hover_text("Open live remote process viewer").clicked() {
                                self.process_view.open = true;
                                self.process_view.loading = true;
                                let _ = tab.session.commands.try_send(Command::ProcessList);
                            }
                            if ui.small_button("📑 Snippets").on_hover_text("Open command snippets library").clicked() {
                                self.snippets_ui.manager_open = true;
                            }
                            let bcast_color = if self.snippets_ui.broadcast_mode { colors::WARNING } else { if dark { colors::FOREGROUND } else { colors::LIGHT_FOREGROUND } };
                            if ui.small_button(RichText::new("📡 Broadcast").color(bcast_color)).on_hover_text("Toggle Multi-Exec broadcast across all active sessions").clicked() {
                                self.snippets_ui.broadcast_mode = !self.snippets_ui.broadcast_mode;
                            }
                            let tunnels_btn_text = if !self.active_tunnels.is_empty() {
                                format!("🔀 Tunnels ({})", self.active_tunnels.len())
                            } else {
                                "🔀 Tunnels".to_string()
                            };
                            let tunnels_color = if !self.active_tunnels.is_empty() {
                                Color32::from_rgb(74, 222, 128)
                            } else if dark {
                                colors::FOREGROUND
                            } else {
                                colors::LIGHT_FOREGROUND
                            };
                            if ui.small_button(RichText::new(tunnels_btn_text).color(tunnels_color)).on_hover_text("Open SSH tunnel and proxy manager").clicked() {
                                self.tunnels_open = true;
                            }
                            if ui.small_button("🏢 Workspaces").on_hover_text("Open session workspaces & clusters manager").clicked() {
                                self.workspaces_open = true;
                            }
                            if ui.small_button("🤖 Macros").on_hover_text("Open automation sequences & login macros").clicked() {
                                self.automation_open = true;
                            }
                            if ui.small_button("🧰 DevOps").on_hover_text("Open Docker, Systemd, and Network Diagnostics toolbox").clicked() {
                                self.devops_ui.open = true;
                                if self.devops_ui.containers.is_empty() {
                                    let _ = tab.session.commands.try_send(Command::DockerList);
                                }
                            }
                            if ui.small_button("📜 History").on_hover_text("Open unified command history & audit trail").clicked() {
                                self.audit_ui.open = true;
                                self.audit_ui.refresh(&self.store);
                            }
                            if ui.small_button("⚡ Triggers").on_hover_text("Open terminal output trigger-action rules").clicked() {
                                self.triggers_ui.open = true;
                            }
                            if ui.small_button("🔐 Vault & Keys").on_hover_text("Open encrypted master vault and SSH key generator").clicked() {
                                self.vault_ui.open = true;
                                self.vault_ui.refresh_keys(&self.store);
                                self.vault_ui.refresh_vault_state(&self.store);
                            }
                            let rec_btn_text = if let Some(rec) = &tab.recorder {
                                format!("⏹ REC ({:.0}s)", rec.duration_secs())
                            } else {
                                "⏺ Record".to_string()
                            };
                            let rec_btn_color = if tab.recorder.is_some() {
                                Color32::from_rgb(235, 87, 87)
                            } else if dark {
                                colors::FOREGROUND
                            } else {
                                colors::LIGHT_FOREGROUND
                            };
                            if ui.small_button(RichText::new(rec_btn_text).color(rec_btn_color)).on_hover_text("Start / stop session recording (Asciicast v2)").clicked() {
                                if let Some(mut rec) = tab.recorder.take() {
                                    if let Ok(p) = rec.stop() {
                                        self.notice = Some(format!("Recording saved to {:?}", p));
                                    }
                                } else {
                                    match kervesh_core::SessionRecorder::start(&format!("{}", tab.id), &tab.host.name, kervesh_core::RecordingFormat::AsciicastV2, None, 100, 30) {
                                        Ok(rec) => {
                                            tab.recorder = Some(rec);
                                            self.notice = Some("Session recording started (Asciicast v2)".into());
                                        }
                                        Err(e) => {
                                            tab.error = Some(format!("Recording failed: {}", e));
                                        }
                                    }
                                }
                            }

                            ui.separator();
                            if tab.split_mode == SplitMode::None {
                                if ui.small_button("⬔ Split V").on_hover_text("Split pane vertically (side-by-side)").clicked() {
                                    tab.split_mode = SplitMode::Vertical;
                                    if tab.secondary_terminal.is_none() {
                                        tab.secondary_terminal = Some(Terminal::with_profile(100, 30, tab.terminal.profile().clone()));
                                    }
                                }
                                if ui.small_button("⬒ Split H").on_hover_text("Split pane horizontally (stacked)").clicked() {
                                    tab.split_mode = SplitMode::Horizontal;
                                    if tab.secondary_terminal.is_none() {
                                        tab.secondary_terminal = Some(Terminal::with_profile(100, 30, tab.terminal.profile().clone()));
                                    }
                                }
                            } else if ui.small_button("✕ Unsplit").on_hover_text("Close split pane").clicked() {
                                tab.split_mode = SplitMode::None;
                                tab.secondary_terminal = None;
                                tab.active_pane = 0;
                            }
                        }

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            let meta_str = if let Some(c) = &tab.capabilities {
                                let os = if c.os.is_empty() { "Unknown" } else { &c.os };
                                let kernel = if c.kernel.is_empty() {
                                    "Unknown"
                                } else {
                                    &c.kernel
                                };
                                format!("{}  |  {}  |  {}", tab.host.hostname, os, kernel)
                            } else {
                                format!("{}  |  Detecting…", tab.host.hostname)
                            };
                            ui.label(RichText::new(meta_str).small().weak());
                        });
                    });

                    // Multi-Exec Broadcast Bar
                    let mut broadcast_bytes: Option<Vec<u8>> = None;
                    if self.snippets_ui.broadcast_mode {
                        ui.group(|ui| {
                            ui.horizontal(|ui| {
                                ui.colored_label(colors::WARNING, "📡 BROADCAST MODE ACTIVE");
                                let response = ui.add(
                                    egui::TextEdit::singleline(&mut self.snippets_ui.broadcast_input)
                                        .hint_text("Type command and press Enter to broadcast to all sessions…")
                                        .desired_width(320.0),
                                );
                                let send = ui.button("Send to All").clicked() || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));
                                if send && !self.snippets_ui.broadcast_input.is_empty() {
                                    let mut cmd = self.snippets_ui.broadcast_input.clone();
                                    if !cmd.ends_with('\n') {
                                        cmd.push('\n');
                                    }
                                    broadcast_bytes = Some(cmd.into_bytes());
                                    self.snippets_ui.broadcast_input.clear();
                                }
                                if ui.button("✕ Exit").clicked() {
                                    self.snippets_ui.broadcast_mode = false;
                                }
                            });
                        });
                    }

                    if let Some(error) = tab.error.clone() {
                        ui.horizontal_wrapped(|ui| {
                            ui.colored_label(colors::WARNING, error);
                            if ui.small_button("Dismiss").clicked() {
                                tab.error = None;
                            }
                        });
                    }

                    ui.separator();

                    // Terminal Widget & Split Panes
                    let modal = self.host_form.is_some()
                        || self.login.is_some()
                        || !self.trust.is_empty()
                        || self.confirmation.is_some()
                        || self.file_dialog.is_some()
                        || self.settings_open
                        || self.inspector_open
                        || tab.editor.is_some();

                    ui.add_enabled_ui(tab.connected && !modal, |ui| {
                        let split_mode = tab.split_mode;
                        let split_ratio = tab.split_ratio;

                        match split_mode {
                            SplitMode::None => {
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
                                    let _ = tab.session.commands.try_send(Command::File(FileOperation::List(directory)));
                                }
                                if action.audio_bell {
                                    self.runtime.spawn_blocking(crate::bell::play);
                                }
                                if let Some((cols, rows)) = action.resize {
                                    let _ = tab.session.commands.try_send(Command::Resize(cols, rows));
                                }
                                if !action.input.is_empty() {
                                    for &b in &action.input {
                                        if b == b'\r' || b == b'\n' {
                                            // Raw PTY input cannot distinguish shell commands from
                                            // passwords, prompts, or application control data.
                                            // Never persist it as an audit command.
                                            tab.cmd_buffer.clear();
                                        } else if b == 0x08 || b == 0x7f {
                                            tab.cmd_buffer.pop();
                                        } else if (32..=126).contains(&b) {
                                            tab.cmd_buffer.push(b as char);
                                        }
                                    }
                                    let _ = tab.session.commands.try_send(Command::Input(action.input));
                                }
                            }
                            SplitMode::Vertical => {
                                let available = ui.available_size();
                                let w1 = (available.x * split_ratio - 4.0).max(100.0);
                                let w2 = (available.x * (1.0 - split_ratio) - 4.0).max(100.0);
                                ui.horizontal(|ui| {
                                    ui.allocate_ui(egui::vec2(w1, available.y), |ui| {
                                        let action = ui.push_id((tab.id, 0), |ui| tab.terminal.show(ui, tab.sftp_available)).inner;
                                        if !action.input.is_empty() {
                                            for &b in &action.input {
                                                if b == b'\r' || b == b'\n' {
                                                    tab.cmd_buffer.clear();
                                                } else if b == 0x08 || b == 0x7f {
                                                    tab.cmd_buffer.pop();
                                                } else if (32..=126).contains(&b) {
                                                    tab.cmd_buffer.push(b as char);
                                                }
                                            }
                                            let _ = tab.session.commands.try_send(Command::Input(action.input));
                                        }
                                        if let Some((cols, rows)) = action.resize {
                                            let _ = tab.session.commands.try_send(Command::Resize(cols, rows));
                                        }
                                    });
                                    ui.separator();
                                    ui.allocate_ui(egui::vec2(w2, available.y), |ui| {
                                        if let Some(sec) = &mut tab.secondary_terminal {
                                            ui.label(RichText::new("Mirror · read-only").small().weak());
                                            ui.add_enabled_ui(false, |ui| {
                                                let _ = ui.push_id((tab.id, 1), |ui| sec.show(ui, tab.sftp_available));
                                            });
                                        }
                                    });
                                });
                            }
                            SplitMode::Horizontal => {
                                let available = ui.available_size();
                                let h1 = (available.y * split_ratio - 4.0).max(80.0);
                                let h2 = (available.y * (1.0 - split_ratio) - 4.0).max(80.0);
                                ui.vertical(|ui| {
                                    ui.allocate_ui(egui::vec2(available.x, h1), |ui| {
                                        let action = ui.push_id((tab.id, 0), |ui| tab.terminal.show(ui, tab.sftp_available)).inner;
                                        if !action.input.is_empty() {
                                            for &b in &action.input {
                                                if b == b'\r' || b == b'\n' {
                                                    tab.cmd_buffer.clear();
                                                } else if b == 0x08 || b == 0x7f {
                                                    tab.cmd_buffer.pop();
                                                } else if (32..=126).contains(&b) {
                                                    tab.cmd_buffer.push(b as char);
                                                }
                                            }
                                            let _ = tab.session.commands.try_send(Command::Input(action.input));
                                        }
                                        if let Some((cols, rows)) = action.resize {
                                            let _ = tab.session.commands.try_send(Command::Resize(cols, rows));
                                        }
                                    });
                                    ui.separator();
                                    ui.allocate_ui(egui::vec2(available.x, h2), |ui| {
                                        if let Some(sec) = &mut tab.secondary_terminal {
                                            ui.label(RichText::new("Mirror · read-only").small().weak());
                                            ui.add_enabled_ui(false, |ui| {
                                                let _ = ui.push_id((tab.id, 1), |ui| sec.show(ui, tab.sftp_available));
                                            });
                                        }
                                    });
                                });
                            }
                        }
                    });

                    if let Some(host) = reconnect {
                        self.begin_connect(host, ctx);
                    }

                    if let Some(bytes) = broadcast_bytes {
                        for t in &mut self.tabs {
                            if t.connected {
                                let _ = t.session.commands.try_send(Command::Input(bytes.clone()));
                            }
                        }
                    }
                }
            });

        // Process Viewer Window
        let pv_action = if let Some(tab) = self.tabs.get(self.active) {
            self.process_view.show(ctx, &tab.host.name, dark)
        } else {
            self.process_view.show(ctx, "No Active Session", dark)
        };
        match pv_action {
            crate::process_view::ProcessViewAction::Refresh => {
                if let Some(tab) = self.tabs.get(self.active) {
                    self.process_view.loading = true;
                    let _ = tab.session.commands.try_send(Command::ProcessList);
                }
            }
            crate::process_view::ProcessViewAction::SendSignal(pid, sig) => {
                if let Some(tab) = self.tabs.get(self.active) {
                    let _ = tab
                        .session
                        .commands
                        .try_send(Command::SignalProcess(pid, sig));
                }
            }
            crate::process_view::ProcessViewAction::None => {}
        }

        // Snippets Modals
        let snippet_action1 = self.snippets_ui.show_manager(ctx, &self.store, dark);
        let snippet_action2 = self.snippets_ui.show_runner_modal(ctx);
        let snippet_action = snippet_action1.or(snippet_action2);

        if let Some(action) = snippet_action {
            match action {
                crate::snippets_ui::SnippetAction::InsertIntoActive(cmd) => {
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        let mut text = cmd;
                        if !text.ends_with('\n') {
                            text.push('\n');
                        }
                        let _ = tab
                            .session
                            .commands
                            .try_send(Command::Input(text.into_bytes()));
                    }
                }
                crate::snippets_ui::SnippetAction::BroadcastToAll(cmd) => {
                    let mut text = cmd;
                    if !text.ends_with('\n') {
                        text.push('\n');
                    }
                    let bytes = text.into_bytes();
                    for t in &mut self.tabs {
                        if t.connected {
                            let _ = t.session.commands.try_send(Command::Input(bytes.clone()));
                        }
                    }
                }
            }
        }

        // Tunnels Modal
        let mut tunnel_stats_map = HashMap::new();
        for (id, active) in &self.active_tunnels {
            tunnel_stats_map.insert(id.clone(), active.stats());
        }
        let tunnel_action = self.tunnels_ui.ui(
            ctx,
            &self.tunnels,
            &self.hosts,
            &tunnel_stats_map,
            &mut self.tunnels_open,
        );
        if let Some(action) = tunnel_action {
            match action {
                crate::tunnels::TunnelAction::Start(config) => {
                    self.start_tunnel(config, ctx);
                }
                crate::tunnels::TunnelAction::Stop(id) => {
                    self.stop_tunnel(&id);
                }
                crate::tunnels::TunnelAction::Save(config) => {
                    if let Err(e) = self.store.save_tunnel(&config) {
                        self.tunnels_ui.error_message = Some(e.to_string());
                    } else {
                        self.refresh_tunnels();
                    }
                }
                crate::tunnels::TunnelAction::Delete(id) => {
                    self.stop_tunnel(&id);
                    if let Err(e) = self.store.delete_tunnel(&id) {
                        self.tunnels_ui.error_message = Some(e.to_string());
                    } else {
                        self.refresh_tunnels();
                    }
                }
            }
        }

        // Workspaces Modal
        let ws_action = self.workspaces_ui.ui(
            ctx,
            &self.workspaces,
            &self.hosts,
            &mut self.workspaces_open,
        );
        if let Some(action) = ws_action {
            match action {
                crate::workspaces_ui::WorkspaceAction::ConnectAll(host_ids) => {
                    for hid in host_ids {
                        if let Some(host) = self.hosts.iter().find(|h| h.id == hid).cloned() {
                            self.begin_connect(host, ctx);
                        }
                    }
                }
                crate::workspaces_ui::WorkspaceAction::Save(ws) => {
                    if let Err(e) = self.store.save_workspace(&ws) {
                        self.workspaces_ui.error_message = Some(e.to_string());
                    } else {
                        self.refresh_workspaces();
                    }
                }
                crate::workspaces_ui::WorkspaceAction::Delete(id) => {
                    if let Err(e) = self.store.delete_workspace(&id) {
                        self.workspaces_ui.error_message = Some(e.to_string());
                    } else {
                        self.refresh_workspaces();
                    }
                }
            }
        }

        // Automation Macros Modal
        let macro_action =
            self.automation_ui
                .ui(ctx, &self.macros, &self.hosts, &mut self.automation_open);
        if let Some(action) = macro_action {
            match action {
                crate::automation_ui::AutomationAction::RunOnActive(mac) => {
                    if let Some(tab) = self.tabs.get(self.active) {
                        if tab.connected {
                            let _ = tab.session.commands.try_send(Command::RunMacro(mac));
                        } else {
                            self.automation_ui.error_message =
                                Some("Active session is not connected".into());
                        }
                    } else {
                        self.automation_ui.error_message = Some("No active session tab".into());
                    }
                }
                crate::automation_ui::AutomationAction::Save(mac) => {
                    if let Err(e) = self.store.save_macro(&mac) {
                        self.automation_ui.error_message = Some(e.to_string());
                    } else {
                        self.refresh_macros();
                    }
                }
                crate::automation_ui::AutomationAction::Delete(id) => {
                    if let Err(e) = self.store.delete_macro(&id) {
                        self.automation_ui.error_message = Some(e.to_string());
                    } else {
                        self.refresh_macros();
                    }
                }
            }
        }

        // Remote File Search Modal
        let search_action = self.search_ui.show(ctx);
        if let Some(action) = search_action {
            match action {
                crate::search_ui::SearchUiAction::ExecuteSearch(query) => {
                    if let Some(tab) = self.tabs.get(self.active) {
                        if tab.connected {
                            let _ = tab.session.commands.try_send(Command::SearchFiles(query));
                        } else {
                            self.search_ui
                                .set_error("Active session is not connected".into());
                        }
                    } else {
                        self.search_ui.set_error("No active session tab".into());
                    }
                }
                crate::search_ui::SearchUiAction::OpenFile { path, .. } => {
                    if let Some(tab) = self.tabs.get(self.active) {
                        let _ = tab
                            .session
                            .commands
                            .try_send(Command::File(FileOperation::Read(path)));
                    }
                }
                crate::search_ui::SearchUiAction::Close => {}
            }
        }

        // Directory Sync Modal
        let sync_action = self.sync_ui.show(ctx);
        if let Some(action) = sync_action {
            match action {
                crate::sync_ui::SyncUiAction::ComputePlan {
                    local_dir,
                    remote_dir,
                    direction,
                    policy,
                } => {
                    if let Some(tab) = self.tabs.get(self.active) {
                        if tab.connected {
                            let _ = tab.session.commands.try_send(Command::ComputeSyncPlan {
                                local_dir: local_dir.into(),
                                remote_dir,
                                direction,
                                policy,
                            });
                        } else {
                            self.sync_ui
                                .set_error("Active session is not connected".into());
                        }
                    } else {
                        self.sync_ui.set_error("No active session tab".into());
                    }
                }
                crate::sync_ui::SyncUiAction::ExecuteSync(plan) => {
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        if tab.connected {
                            let transfer_id = self.next_id;
                            self.next_id = self.next_id.wrapping_add(1);
                            let cancel = kervesh_ssh::CancellationToken::new();
                            let _ = tab.session.commands.try_send(Command::ExecuteSync {
                                plan,
                                transfer_id,
                                cancel,
                            });
                            self.sync_ui
                                .set_complete("Sync job queued and running in background".into());
                        } else {
                            self.sync_ui
                                .set_error("Active session is not connected".into());
                        }
                    } else {
                        self.sync_ui.set_error("No active session tab".into());
                    }
                }
                crate::sync_ui::SyncUiAction::Close => {}
            }
        }

        // Sysadmin & DevOps Modal
        let devops_action = self.devops_ui.show(ctx);
        if let Some(action) = devops_action {
            match action {
                crate::devops_ui::DevOpsUiAction::RefreshDocker => {
                    if let Some(tab) = self.tabs.get(self.active) {
                        if tab.connected {
                            let _ = tab.session.commands.try_send(Command::DockerList);
                        } else {
                            self.devops_ui.error = Some("Active session is not connected".into());
                        }
                    }
                }
                crate::devops_ui::DevOpsUiAction::DockerAction(id, act) => {
                    if let Some(tab) = self.tabs.get(self.active) {
                        if tab.connected {
                            let _ = tab
                                .session
                                .commands
                                .try_send(Command::DockerAction(id, act));
                        } else {
                            self.devops_ui.error = Some("Active session is not connected".into());
                        }
                    }
                }
                crate::devops_ui::DevOpsUiAction::DockerLogs(id) => {
                    if let Some(tab) = self.tabs.get(self.active) {
                        if tab.connected {
                            let _ = tab.session.commands.try_send(Command::DockerLogs(id));
                        } else {
                            self.devops_ui.error = Some("Active session is not connected".into());
                        }
                    }
                }
                crate::devops_ui::DevOpsUiAction::RefreshSystemd => {
                    if let Some(tab) = self.tabs.get(self.active) {
                        if tab.connected {
                            let _ = tab.session.commands.try_send(Command::SystemdList);
                        } else {
                            self.devops_ui.error = Some("Active session is not connected".into());
                        }
                    }
                }
                crate::devops_ui::DevOpsUiAction::SystemdAction(unit, act) => {
                    if let Some(tab) = self.tabs.get(self.active) {
                        if tab.connected {
                            let _ = tab
                                .session
                                .commands
                                .try_send(Command::SystemdAction(unit, act));
                        } else {
                            self.devops_ui.error = Some("Active session is not connected".into());
                        }
                    }
                }
                crate::devops_ui::DevOpsUiAction::SystemdLogs(unit) => {
                    if let Some(tab) = self.tabs.get(self.active) {
                        if tab.connected {
                            let _ = tab.session.commands.try_send(Command::SystemdLogs(unit));
                        } else {
                            self.devops_ui.error = Some("Active session is not connected".into());
                        }
                    }
                }
                crate::devops_ui::DevOpsUiAction::RunNetDiag {
                    tool,
                    target,
                    port_or_type,
                } => {
                    if let Some(tab) = self.tabs.get(self.active) {
                        if tab.connected {
                            let _ = tab.session.commands.try_send(Command::NetDiag {
                                tool,
                                target,
                                port_or_type,
                            });
                        } else {
                            self.devops_ui.error = Some("Active session is not connected".into());
                            self.devops_ui.diag_running = false;
                        }
                    } else {
                        self.devops_ui.error = Some("No active session tab".into());
                        self.devops_ui.diag_running = false;
                    }
                }
                crate::devops_ui::DevOpsUiAction::Close => {}
            }
        }

        // Audit & History Modal
        let mut audit_action = None;
        self.audit_ui
            .render(ctx, &self.store, &self.hosts, &mut audit_action);
        if let Some(act) = audit_action {
            match act {
                crate::audit_ui::AuditUiAction::RunCommand(cmd) => {
                    let mut cmd_bytes = cmd.into_bytes();
                    cmd_bytes.push(b'\n');
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        let _ = tab.session.commands.try_send(Command::Input(cmd_bytes));
                    }
                }
                crate::audit_ui::AuditUiAction::CopyCommand(cmd) => {
                    ctx.copy_text(cmd);
                }
                crate::audit_ui::AuditUiAction::ClearHistory => {}
            }
        }

        // Triggers UI Modal
        let mut trigger_action = None;
        self.triggers_ui
            .render(ctx, &self.store, &self.hosts, &mut trigger_action);
        if let Some(act) = trigger_action {
            match act {
                crate::triggers_ui::TriggerUiAction::TriggerSaved
                | crate::triggers_ui::TriggerUiAction::TriggerDeleted => {
                    let rules = self.store.triggers().unwrap_or_default();
                    self.trigger_engine = kervesh_core::TriggerEngine::new(&rules);
                    self.trigger_rules = rules;
                }
            }
        }

        // Encrypted Vault & Key Inventory Modal
        let mut vault_action = None;
        self.vault_ui.render(
            ctx,
            &self.runtime,
            &self.store,
            &self.hosts,
            &mut vault_action,
        );
        if let Some(act) = vault_action {
            match act {
                crate::vault_ui::VaultUiAction::DeployKeyToHost {
                    host_id,
                    public_key,
                } => {
                    let cmd_str = kervesh_core::generate_ssh_copy_id_command(&public_key);
                    let mut cmd_bytes = cmd_str.into_bytes();
                    cmd_bytes.push(b'\n');
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.host.id == host_id) {
                        let _ = tab.session.commands.try_send(Command::Input(cmd_bytes));
                        self.notice = Some("Key deployment command sent to host terminal".into());
                    } else {
                        self.notice = Some("Host not connected in active session".into());
                    }
                }
                crate::vault_ui::VaultUiAction::CopyText(txt) => {
                    ctx.copy_text(txt);
                }
            }
        }

        // Theme Engine & ANSI Palette Modal
        let mut theme_action = None;
        self.theme_ui
            .render(ctx, &mut self.settings, &self.store, &mut theme_action);
        if let Some(act) = theme_action {
            match act {
                crate::theme_ui::ThemeUiAction::ProfileUpdated => {
                    for tab in &mut self.tabs {
                        if tab.terminal.profile().id == self.theme_ui.active_profile_id {
                            tab.terminal.set_profile(
                                self.settings
                                    .terminal_profile(Some(&self.theme_ui.active_profile_id))
                                    .clone(),
                            );
                        }
                    }
                }
                crate::theme_ui::ThemeUiAction::ExportPalette(json) => {
                    ctx.copy_text(json);
                    self.notice = Some("Palette JSON copied to clipboard".into());
                }
            }
        }

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

#[cfg(test)]
mod tests {
    use super::finish_tunnel_attempt;
    use std::collections::HashMap;

    #[test]
    fn stale_tunnel_attempt_cannot_finish_new_start() {
        let mut starting = HashMap::from([(String::from("tunnel"), 2_u64)]);

        assert!(!finish_tunnel_attempt(&mut starting, "tunnel", 1));
        assert_eq!(starting.get("tunnel"), Some(&2));
        assert!(finish_tunnel_attempt(&mut starting, "tunnel", 2));
        assert!(starting.is_empty());
    }
}

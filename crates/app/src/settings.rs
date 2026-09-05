use crate::app::{App, Confirmation};
use egui::RichText;
use kervesh_core::{Settings, bytes};

impl App {
    pub(crate) fn settings_dialog(&mut self, ctx: &egui::Context) {
        if !self.settings_open {
            return;
        }
        let mut open = true;
        let mut save = false;
        let mut export = false;
        let mut import = false;
        let mut import_ssh = false;
        let mut forget = None;
        egui::Window::new("Settings").open(&mut open).default_width(520.0).vscroll(true).show(ctx,|ui|{
            ui.heading("Appearance & behavior");ui.checkbox(&mut self.settings.dark,"Dark theme");
            ui.add(egui::Slider::new(&mut self.settings.font_size,8.0..=32.0).text("Terminal font size"));
            ui.add(egui::Slider::new(&mut self.settings.scrollback,0..=100000).logarithmic(true).text("Scrollback lines"));
            ui.add(egui::Slider::new(&mut self.settings.monitor_secs,1..=300).text("Monitor interval (seconds)"));
            ui.checkbox(&mut self.settings.show_hidden,"Show hidden remote files");
            ui.label(RichText::new("Scrollback and monitor interval apply to new sessions.").small().weak());
            ui.horizontal(|ui|{save=ui.button("Save settings").clicked();if ui.button("Restore defaults").clicked(){self.settings=Settings::default();}});
            ui.separator();
            ui.heading("Portable connections");
            ui.label("Versioned JSON export includes hosts and preferences. Passwords, passphrases, trusted fingerprints and recent history are excluded.");
            ui.horizontal(|ui| {
                export = ui.button("Export JSON…").clicked();
                import = ui.button("Import JSON…").clicked();
                if ui.button("Import ~/.ssh/config").on_hover_text("Import hosts from default OpenSSH configuration").clicked() {
                    match self.store.import_default_ssh_config() {
                        Ok(count) => {
                            self.refresh_hosts();
                            self.notice = Some(format!("Imported {count} hosts from ~/.ssh/config."));
                        }
                        Err(e) => {
                            self.notice = Some(format!("Could not import ~/.ssh/config: {e}"));
                        }
                    }
                }
                if ui.button("Import SSH config…").on_hover_text("Import hosts from an OpenSSH config file").clicked() {
                    import_ssh = true;
                }
            });
            ui.separator();
            ui.heading("Trusted host keys");
            match self.store.known_hosts() {
                Ok(hosts) => {
                    if hosts.is_empty() {
                        ui.label("No trusted hosts yet.");
                    }
                    for (host, port, fingerprint) in hosts {
                        ui.horizontal(|ui| {
                            ui.monospace(format!("{host}:{port}"));
                            if ui.small_button("Remove…").clicked() {
                                forget = Some((host.clone(), port));
                            }
                        });
                        ui.label(RichText::new(fingerprint).monospace().small());
                    }
                }
                Err(e) => {
                    ui.colored_label(crate::theme::colors::DANGER, e.to_string());
                }
            }
            ui.separator();
            ui.heading("Keyboard");
            ui.label(
                "Terminal: Ctrl+C sends interrupt · Ctrl+Shift+C copies selection · Ctrl+Shift+V pastes · Shift+PageUp/Down scrolls history · Shift+drag selects while remote mouse mode is active.",
            );
            ui.label(RichText::new("Kervesh 0.1 · Native Rust workspace · No telemetry").small().weak());
        });
        self.settings_open = open;
        if save {
            match self.store.save_settings(&self.settings) {
                Ok(()) => self.settings_open = false,
                Err(e) => self.notice = Some(e.to_string()),
            }
        }
        if let Some((host, port)) = forget {
            self.confirmation = Some(Confirmation::ForgetTrust(host, port));
        }
        if export
            && let Some(path) = rfd::FileDialog::new()
                .set_file_name("kervesh-connections.json")
                .add_filter("JSON", &["json"])
                .save_file()
        {
            match self.store.export().and_then(|data| {
                std::fs::write(path, data)?;
                Ok(())
            }) {
                Ok(()) => self.notice = Some("Export saved without credentials.".into()),
                Err(e) => self.notice = Some(e.to_string()),
            }
        }
        if import
            && let Some(path) = rfd::FileDialog::new()
                .add_filter("JSON", &["json"])
                .pick_file()
        {
            let result = (|| -> anyhow::Result<usize> {
                anyhow::ensure!(
                    std::fs::metadata(&path)?.len() <= 10 * 1024 * 1024,
                    "Import exceeds 10 MB"
                );
                self.store.import(&std::fs::read_to_string(path)?)
            })();
            match result {
                Ok(count) => {
                    self.refresh_hosts();
                    if let Ok(settings) = self.store.settings() {
                        self.settings = settings;
                    }
                    self.notice = Some(format!(
                        "Imported {count} connections. Credentials must be supplied separately."
                    ));
                }
                Err(e) => {
                    self.notice = Some(format!("Import rejected; existing data unchanged: {e}"))
                }
            }
        }
        if import_ssh && let Some(path) = rfd::FileDialog::new().pick_file() {
            let result = (|| -> anyhow::Result<usize> {
                anyhow::ensure!(
                    std::fs::metadata(&path)?.len() <= 10 * 1024 * 1024,
                    "File exceeds 10 MB"
                );
                let content = std::fs::read_to_string(path)?;
                self.store.import_ssh_config(&content)
            })();
            match result {
                Ok(count) => {
                    self.refresh_hosts();
                    self.notice = Some(format!("Imported {count} hosts from SSH config."));
                }
                Err(e) => {
                    self.notice = Some(format!("Import failed: {e}"));
                }
            }
        }
    }
    pub(crate) fn inspector(&mut self, ctx: &egui::Context) {
        if !self.inspector_open {
            return;
        }
        let mut open = true;
        egui::Window::new("System inspector").open(&mut open).default_width(650.0).vscroll(true).show(ctx,|ui|{
            let Some(tab)=self.tabs.get(self.active)else{ui.label("Connect to a host first.");return;};
            ui.heading(&tab.host.name);
            if let Some(c)=&tab.capabilities{egui::Grid::new("system_meta").num_columns(2).show(ui,|ui|{for (label,value) in [("Hostname",&c.hostname),("OS",&c.os),("Kernel",&c.kernel),("Architecture",&c.architecture)]{ui.label(RichText::new(label).weak());ui.monospace(value);ui.end_row();}});}
            let Some(s)=&tab.snapshot else{ui.label("System metrics unavailable. Linux procfs is required for the v0.1 collector.");return;};
            ui.separator();ui.heading("CPU & memory");ui.label(&s.cpu_model);
            if let Some(load)=s.load{ui.monospace(format!("Load: {:.2}  {:.2}  {:.2}",load[0],load[1],load[2]));}
            if let Some(uptime)=s.uptime{ui.label(format!("Uptime: {:.0}d {:.0}h · Processes: {}",(uptime/86400.0).floor(),(uptime%86400.0/3600.0).floor(),s.processes.map(|v|v.to_string()).unwrap_or_else(||"—".into())));}
            for (name,usage) in &tab.rates.cores{
                ui.horizontal(|ui|{
                    ui.monospace(name);
                    let mut bar = egui::ProgressBar::new(*usage as f32/100.0).desired_width(200.0).text(format!("{usage:.0}%"));
                    if *usage >= 90.0 {
                        bar = bar.fill(crate::theme::colors::WARNING);
                    }
                    ui.add(bar);
                });
            }
            let mem_used = s.memory_used();
            let mem_total = s.memory.get("MemTotal").copied().unwrap_or(0);
            let mem_alert = mem_total > 0 && mem_used.is_some_and(|u| u as f64 / mem_total as f64 >= 0.9);
            let mem_color = if mem_alert { crate::theme::colors::WARNING } else { ui.visuals().text_color() };
            ui.colored_label(mem_color, format!("Memory: {} used / {} total · Swap: {} used / {} total",mem_used.map(bytes).unwrap_or_else(||"—".into()),s.memory.get("MemTotal").copied().map(bytes).unwrap_or_else(||"—".into()),s.swap_used().map(bytes).unwrap_or_else(||"—".into()),s.memory.get("SwapTotal").copied().map(bytes).unwrap_or_else(||"—".into())));
            ui.label(format!("Available: {} · Cached: {} · Buffers: {}",bytes(*s.memory.get("MemAvailable").unwrap_or(&0)),bytes(*s.memory.get("Cached").unwrap_or(&0)),bytes(*s.memory.get("Buffers").unwrap_or(&0))));
            ui.separator();ui.heading("Filesystems");
            egui::Grid::new("mounts").striped(true).show(ui,|ui|{
                for label in ["Mount","Device","Used","Free","Usage"]{ui.strong(label);}
                ui.end_row();
                for fs in &s.filesystems{
                    let fs_color = if fs.percent >= 90.0 { crate::theme::colors::WARNING } else { ui.visuals().text_color() };
                    ui.monospace(&fs.mount);
                    ui.label(&fs.device);
                    ui.label(bytes(fs.used));
                    ui.label(bytes(fs.available));
                    ui.colored_label(fs_color, format!("{:.0}%",fs.percent));
                    ui.end_row();
                }
            });
            ui.separator();ui.heading("Network");egui::Grid::new("interfaces").striped(true).show(ui,|ui|{ui.strong("Interface");ui.strong("Receive / s");ui.strong("Transmit / s");ui.end_row();for (name,(rx,tx)) in &tab.rates.network{ui.monospace(name);ui.label(bytes(*rx as u64));ui.label(bytes(*tx as u64));ui.end_row();}});
            ui.collapsing("Inodes & file descriptors",|ui|{ui.monospace(&s.inodes);ui.monospace(format!("Allocated / unused / max file handles: {}",s.file_descriptors));});
        });
        self.inspector_open = open;
    }
}

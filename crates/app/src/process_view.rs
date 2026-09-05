use crate::theme::colors;
use egui::RichText;
use kervesh_core::{ProcessInfo, Signal};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProcessSortColumn {
    Pid,
    User,
    Cpu,
    Mem,
    Command,
}

pub enum ProcessViewAction {
    None,
    Refresh,
    SendSignal(u32, Signal),
}

pub struct ProcessViewState {
    pub open: bool,
    pub processes: Vec<ProcessInfo>,
    pub filter: String,
    pub sort_col: ProcessSortColumn,
    pub sort_desc: bool,
    pub selected_pid: Option<u32>,
    pub signal_dialog: Option<(u32, Signal)>,
    pub auto_refresh: bool,
    pub last_refresh: Option<Instant>,
    pub loading: bool,
}

impl Default for ProcessViewState {
    fn default() -> Self {
        Self {
            open: false,
            processes: Vec::new(),
            filter: String::new(),
            sort_col: ProcessSortColumn::Cpu,
            sort_desc: true,
            selected_pid: None,
            signal_dialog: None,
            auto_refresh: true,
            last_refresh: None,
            loading: false,
        }
    }
}

impl ProcessViewState {
    pub fn toggle_sort(&mut self, col: ProcessSortColumn, default_desc: bool) {
        if self.sort_col == col {
            self.sort_desc = !self.sort_desc;
        } else {
            self.sort_col = col;
            self.sort_desc = default_desc;
        }
    }

    pub fn sorted_and_filtered(&self) -> Vec<ProcessInfo> {
        let mut list: Vec<ProcessInfo> = self
            .processes
            .iter()
            .filter(|p| {
                if self.filter.is_empty() {
                    return true;
                }
                let q = self.filter.to_lowercase();
                p.command.to_lowercase().contains(&q)
                    || p.user.to_lowercase().contains(&q)
                    || p.pid.to_string().contains(&q)
            })
            .cloned()
            .collect();

        list.sort_by(|a, b| {
            let ordering = match self.sort_col {
                ProcessSortColumn::Pid => a.pid.cmp(&b.pid),
                ProcessSortColumn::User => a.user.cmp(&b.user),
                ProcessSortColumn::Cpu => a
                    .cpu
                    .partial_cmp(&b.cpu)
                    .unwrap_or(std::cmp::Ordering::Equal),
                ProcessSortColumn::Mem => a
                    .mem
                    .partial_cmp(&b.mem)
                    .unwrap_or(std::cmp::Ordering::Equal),
                ProcessSortColumn::Command => a.command.cmp(&b.command),
            };
            if self.sort_desc {
                ordering.reverse()
            } else {
                ordering
            }
        });

        list
    }

    pub fn show(&mut self, ctx: &egui::Context, host_name: &str, dark: bool) -> ProcessViewAction {
        if !self.open {
            return ProcessViewAction::None;
        }

        let mut action = ProcessViewAction::None;
        let mut open = self.open;

        // Auto-refresh every 3 seconds if enabled
        if self.auto_refresh {
            let should_refresh = match self.last_refresh {
                Some(last) => last.elapsed().as_secs() >= 3,
                None => true,
            };
            if should_refresh && !self.loading {
                self.last_refresh = Some(Instant::now());
                action = ProcessViewAction::Refresh;
            }
        }

        egui::Window::new(format!("Process Viewer — {}", host_name))
            .open(&mut open)
            .default_width(850.0)
            .default_height(550.0)
            .resizable(true)
            .show(ctx, |ui| {
                // Top control bar
                ui.horizontal(|ui| {
                    ui.label("Filter:");
                    ui.add(
                        egui::TextEdit::singleline(&mut self.filter)
                            .hint_text("Search PID, user, command…")
                            .desired_width(200.0),
                    );
                    if ui.button("Clear").clicked() {
                        self.filter.clear();
                    }

                    ui.separator();
                    if ui.button("🔄 Refresh").clicked() {
                        self.last_refresh = Some(Instant::now());
                        action = ProcessViewAction::Refresh;
                    }
                    ui.checkbox(&mut self.auto_refresh, "Auto-refresh (3s)");

                    if self.loading {
                        ui.spinner();
                        ui.label("Loading processes…");
                    } else {
                        ui.label(
                            RichText::new(format!("{} processes", self.processes.len()))
                                .weak()
                                .small(),
                        );
                    }

                    if let Some(pid) = self.selected_pid {
                        ui.separator();
                        ui.label(RichText::new(format!("Selected: PID {pid}")).strong());

                        if ui.button("Terminate (SIGTERM)").clicked() {
                            self.signal_dialog = Some((pid, Signal::Term));
                        }
                        if ui
                            .button(RichText::new("Force Kill (SIGKILL)").color(colors::DANGER))
                            .clicked()
                        {
                            self.signal_dialog = Some((pid, Signal::Kill));
                        }
                    }
                });

                ui.separator();

                // Table Header with sortable columns
                let text_color = if dark {
                    colors::FOREGROUND
                } else {
                    colors::LIGHT_FOREGROUND
                };

                let current_col = self.sort_col;
                let current_desc = self.sort_desc;
                let arrow = |col: ProcessSortColumn| {
                    if current_col == col {
                        if current_desc { " ▼" } else { " ▲" }
                    } else {
                        ""
                    }
                };

                let mut toggle_target = None;

                ui.horizontal(|ui| {
                    if ui
                        .button(format!("PID{}", arrow(ProcessSortColumn::Pid)))
                        .clicked()
                    {
                        toggle_target = Some((ProcessSortColumn::Pid, false));
                    }
                    if ui
                        .button(format!("User{}", arrow(ProcessSortColumn::User)))
                        .clicked()
                    {
                        toggle_target = Some((ProcessSortColumn::User, false));
                    }
                    if ui
                        .button(format!("%CPU{}", arrow(ProcessSortColumn::Cpu)))
                        .clicked()
                    {
                        toggle_target = Some((ProcessSortColumn::Cpu, true));
                    }
                    if ui
                        .button(format!("%MEM{}", arrow(ProcessSortColumn::Mem)))
                        .clicked()
                    {
                        toggle_target = Some((ProcessSortColumn::Mem, true));
                    }
                    if ui
                        .button(format!("Command{}", arrow(ProcessSortColumn::Command)))
                        .clicked()
                    {
                        toggle_target = Some((ProcessSortColumn::Command, false));
                    }
                });

                if let Some((col, def_desc)) = toggle_target {
                    self.toggle_sort(col, def_desc);
                }

                ui.separator();

                // Process rows
                let processes = self.sorted_and_filtered();
                let mut new_selected_pid = self.selected_pid;

                egui::ScrollArea::both()
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        egui::Grid::new("process_grid")
                            .striped(true)
                            .num_columns(6)
                            .spacing([12.0, 4.0])
                            .show(ui, |ui| {
                                ui.label(RichText::new("PID").strong());
                                ui.label(RichText::new("USER").strong());
                                ui.label(RichText::new("%CPU").strong());
                                ui.label(RichText::new("%MEM").strong());
                                ui.label(RichText::new("TIME").strong());
                                ui.label(RichText::new("COMMAND").strong());
                                ui.end_row();

                                for p in &processes {
                                    let is_selected = new_selected_pid == Some(p.pid);
                                    let pid_btn =
                                        ui.selectable_label(is_selected, p.pid.to_string());
                                    if pid_btn.clicked() {
                                        new_selected_pid =
                                            if is_selected { None } else { Some(p.pid) };
                                    }

                                    ui.label(&p.user);
                                    let cpu_color = if p.cpu > 50.0 {
                                        colors::DANGER
                                    } else if p.cpu > 10.0 {
                                        colors::WARNING
                                    } else {
                                        text_color
                                    };
                                    ui.colored_label(cpu_color, format!("{:.1}%", p.cpu));
                                    ui.label(format!("{:.1}%", p.mem));
                                    ui.label(&p.time);
                                    ui.monospace(&p.command);
                                    ui.end_row();
                                }
                            });
                    });

                self.selected_pid = new_selected_pid;
            });

        // Signal confirmation dialog
        if let Some((pid, sig)) = self.signal_dialog {
            let mut confirm_open = true;
            let mut execute = false;
            let mut cancel = false;

            egui::Window::new("Confirm Process Signal")
                .open(&mut confirm_open)
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label(
                        RichText::new(format!("Send {} to PID {}?", sig.as_str(), pid)).strong(),
                    );
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("Cancel").clicked() {
                            cancel = true;
                        }
                        if ui
                            .button(
                                RichText::new("Confirm & Send").color(if sig == Signal::Kill {
                                    colors::DANGER
                                } else {
                                    colors::FOREGROUND
                                }),
                            )
                            .clicked()
                        {
                            execute = true;
                        }
                    });
                });

            if execute {
                action = ProcessViewAction::SendSignal(pid, sig);
                self.signal_dialog = None;
            } else if cancel || !confirm_open {
                self.signal_dialog = None;
            }
        }

        self.open = open;
        action
    }
}

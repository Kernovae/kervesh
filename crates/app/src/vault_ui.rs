use eframe::egui::{self, Align, Color32, Layout, RichText, ScrollArea, Stroke, TextEdit};
use kervesh_core::{
    EncryptedVault, GeneratedKeypair, Host, KeyAlgorithm, LocalSshKeyInfo, Store, VaultCategory,
    VaultEntry, discover_local_ssh_keys,
};
use std::sync::mpsc::{self, Receiver, Sender};
use tokio::runtime::Runtime;

enum VaultTaskResult {
    Unlock {
        generation: u64,
        result: Result<EncryptedVault, String>,
    },
    Create {
        generation: u64,
        result: Result<EncryptedVault, String>,
    },
    Save {
        generation: u64,
        result: Result<EncryptedVault, String>,
    },
    Delete {
        generation: u64,
        result: Result<EncryptedVault, String>,
    },
    Generate {
        generation: u64,
        result: Result<GeneratedKeypair, String>,
    },
}

impl VaultTaskResult {
    fn generation(&self) -> u64 {
        match self {
            Self::Unlock { generation, .. }
            | Self::Create { generation, .. }
            | Self::Save { generation, .. }
            | Self::Delete { generation, .. }
            | Self::Generate { generation, .. } => *generation,
        }
    }
}

pub enum VaultUiAction {
    DeployKeyToHost { host_id: String, public_key: String },
    CopyText(String),
}

pub struct VaultUi {
    pub open: bool,
    pub selected_tab: usize, // 0: Vault, 1: Key Generator & Inventory

    // Vault State
    pub master_password_input: String,
    pub new_master_password: String,
    pub unlocked_vault: Option<EncryptedVault>,
    pub vault_error: Option<String>,
    vault_exists: Option<bool>,
    pub vault_search: String,
    pub selected_category: Option<VaultCategory>,
    pub editing_entry: Option<VaultEntry>,
    pub show_secrets: bool,

    // Key Generator State
    pub key_algo: KeyAlgorithm,
    pub key_comment: String,
    pub key_passphrase: String,
    pub gen_error: Option<String>,
    pub generated_keys: Vec<GeneratedKeypair>,
    pub local_keys: Vec<LocalSshKeyInfo>,
    pub deploy_host_id: Option<String>,
    pub deploy_target_key: Option<String>,
    task_tx: Sender<VaultTaskResult>,
    task_rx: Receiver<VaultTaskResult>,
    generation: u64,
    task_pending: bool,
}

impl Default for VaultUi {
    fn default() -> Self {
        Self::new()
    }
}

impl VaultUi {
    pub fn new() -> Self {
        let (task_tx, task_rx) = mpsc::channel();
        Self {
            open: false,
            selected_tab: 0,
            master_password_input: String::new(),
            new_master_password: String::new(),
            unlocked_vault: None,
            vault_error: None,
            vault_exists: None,
            vault_search: String::new(),
            selected_category: None,
            editing_entry: None,
            show_secrets: false,
            key_algo: KeyAlgorithm::Ed25519,
            key_comment: "kervesh-key".to_string(),
            key_passphrase: String::new(),
            gen_error: None,
            generated_keys: Vec::new(),
            local_keys: Vec::new(),
            deploy_host_id: None,
            deploy_target_key: None,
            task_tx,
            task_rx,
            generation: 0,
            task_pending: false,
        }
    }

    pub fn refresh_keys(&mut self, store: &Store) {
        self.generated_keys = store.generated_keys().unwrap_or_default();
        self.local_keys = discover_local_ssh_keys();
    }

    pub fn refresh_vault_state(&mut self, store: &Store) {
        self.vault_exists = Some(store.load_vault_blob().unwrap_or(None).is_some());
    }

    fn begin_operation(&mut self) -> Option<u64> {
        if self.task_pending {
            return None;
        }
        self.generation = self.generation.wrapping_add(1);
        self.task_pending = true;
        Some(self.generation)
    }

    fn launch_operation(
        &mut self,
        runtime: &Runtime,
        ctx: &egui::Context,
        task: impl FnOnce(u64) -> VaultTaskResult + Send + 'static,
    ) -> bool {
        let Some(generation) = self.begin_operation() else {
            return false;
        };
        let sender = self.task_tx.clone();
        let wake = ctx.clone();
        runtime.spawn_blocking(move || {
            let result = task(generation);
            let _ = sender.send(result);
            wake.request_repaint();
        });
        true
    }

    fn invalidate_operations(&mut self) {
        self.generation = self.generation.wrapping_add(1);
        self.task_pending = false;
        while self.task_rx.try_recv().is_ok() {}
    }

    fn poll_operations(&mut self) {
        while let Ok(task) = self.task_rx.try_recv() {
            if !self.open || !self.task_pending || task.generation() != self.generation {
                continue;
            }
            self.task_pending = false;
            match task {
                VaultTaskResult::Unlock { result, .. } => match result {
                    Ok(vault) => {
                        self.unlocked_vault = Some(vault);
                        self.vault_error = None;
                    }
                    Err(error) => self.vault_error = Some(error),
                },
                VaultTaskResult::Create { result, .. } | VaultTaskResult::Delete { result, .. } => {
                    match result {
                        Ok(vault) => {
                            self.unlocked_vault = Some(vault);
                            self.vault_exists = Some(true);
                            self.vault_error = None;
                        }
                        Err(error) => self.vault_error = Some(error),
                    }
                }
                VaultTaskResult::Save { result, .. } => match result {
                    Ok(vault) => {
                        self.unlocked_vault = Some(vault);
                        self.vault_exists = Some(true);
                        self.vault_error = None;
                        self.editing_entry = None;
                    }
                    Err(error) => self.vault_error = Some(error),
                },
                VaultTaskResult::Generate { result, .. } => match result {
                    Ok(key) => {
                        self.generated_keys.push(key);
                        self.gen_error = None;
                    }
                    Err(error) => self.gen_error = Some(error),
                },
            }
        }
    }

    fn start_unlock(&mut self, runtime: &Runtime, ctx: &egui::Context, store: &Store) {
        let store = store.clone();
        let password = zeroize::Zeroizing::new(self.master_password_input.clone());
        self.launch_operation(runtime, ctx, move |generation| {
            let result = store
                .load_vault_blob()
                .map_err(|error| format!("Could not read vault: {error:#}"))
                .and_then(|blob| blob.ok_or_else(|| "Vault is not initialized".to_string()))
                .and_then(|blob| {
                    EncryptedVault::unlock(&blob, password.as_str())
                        .map_err(|error| format!("{error:#}"))
                });
            VaultTaskResult::Unlock { generation, result }
        });
    }

    fn start_create(&mut self, runtime: &Runtime, ctx: &egui::Context, store: &Store) {
        let store = store.clone();
        let password = zeroize::Zeroizing::new(self.master_password_input.clone());
        self.launch_operation(runtime, ctx, move |generation| {
            let vault = EncryptedVault::empty();
            let result = vault
                .encrypt_to_blob(password.as_str())
                .map_err(|error| format!("{error:#}"))
                .and_then(|blob| {
                    store
                        .save_vault_blob(&blob)
                        .map_err(|error| format!("Could not save vault: {error:#}"))
                })
                .map(|()| vault);
            VaultTaskResult::Create { generation, result }
        });
    }

    fn start_save_vault(
        &mut self,
        runtime: &Runtime,
        ctx: &egui::Context,
        store: &Store,
        vault: EncryptedVault,
    ) {
        let store = store.clone();
        let password = zeroize::Zeroizing::new(self.master_password_input.clone());
        self.launch_operation(runtime, ctx, move |generation| {
            let result = vault
                .encrypt_to_blob(password.as_str())
                .map_err(|error| format!("{error:#}"))
                .and_then(|blob| {
                    store
                        .save_vault_blob(&blob)
                        .map_err(|error| format!("Could not save vault: {error:#}"))
                })
                .map(|()| vault);
            VaultTaskResult::Save { generation, result }
        });
    }

    fn start_delete_entry(
        &mut self,
        runtime: &Runtime,
        ctx: &egui::Context,
        store: &Store,
        vault: EncryptedVault,
    ) {
        let store = store.clone();
        let password = zeroize::Zeroizing::new(self.master_password_input.clone());
        self.launch_operation(runtime, ctx, move |generation| {
            let result = vault
                .encrypt_to_blob(password.as_str())
                .map_err(|error| format!("{error:#}"))
                .and_then(|blob| {
                    store
                        .save_vault_blob(&blob)
                        .map_err(|error| format!("Could not save vault: {error:#}"))
                })
                .map(|()| vault);
            VaultTaskResult::Delete { generation, result }
        });
    }

    fn start_generate_key(&mut self, runtime: &Runtime, ctx: &egui::Context, store: &Store) {
        let store = store.clone();
        let algorithm = self.key_algo;
        let comment = self.key_comment.clone();
        let passphrase = (!self.key_passphrase.is_empty()).then(|| self.key_passphrase.clone());
        self.launch_operation(runtime, ctx, move |generation| {
            let result = GeneratedKeypair::generate(algorithm, &comment, passphrase.as_deref())
                .map_err(|error| format!("{error:#}"))
                .and_then(|key| {
                    store
                        .save_generated_key(&key)
                        .map(|()| key)
                        .map_err(|error| format!("Could not save generated key: {error:#}"))
                });
            VaultTaskResult::Generate { generation, result }
        });
    }

    pub fn render(
        &mut self,
        ctx: &egui::Context,
        runtime: &Runtime,
        store: &Store,
        hosts: &[Host],
        action: &mut Option<VaultUiAction>,
    ) {
        if !self.open {
            self.invalidate_operations();
            return;
        }
        self.poll_operations();

        let mut is_open = self.open;
        egui::Window::new("🔐 Security, Encrypted Vault & Key Inventory")
            .open(&mut is_open)
            .collapsible(false)
            .resizable(true)
            .default_width(750.0)
            .default_height(540.0)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if ui
                        .selectable_label(self.selected_tab == 0, "🔐 Encrypted Secret Vault")
                        .clicked()
                    {
                        self.selected_tab = 0;
                    }
                    if ui
                        .selectable_label(self.selected_tab == 1, "🛡 SSH Key Generator & Inventory")
                        .clicked()
                    {
                        self.selected_tab = 1;
                        self.refresh_keys(store);
                    }

                    if self.unlocked_vault.is_some() && self.selected_tab == 0 {
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .button(
                                    RichText::new("🔒 Lock Vault")
                                        .color(Color32::from_rgb(235, 87, 87)),
                                )
                                .clicked()
                            {
                                self.invalidate_operations();
                                self.unlocked_vault = None;
                                self.editing_entry = None;
                                self.master_password_input.clear();
                            }
                        });
                    }
                });

                ui.separator();

                match self.selected_tab {
                    0 => self.render_vault_tab(ui, ctx, runtime, store, action),
                    1 => self.render_keys_tab(ui, ctx, runtime, store, hosts, action),
                    _ => {}
                }
            });

        self.open = is_open;
        if !self.open {
            self.invalidate_operations();
            self.unlocked_vault = None;
            self.editing_entry = None;
            self.master_password_input.clear();
        }
    }

    fn render_vault_tab(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        runtime: &Runtime,
        store: &Store,
        action: &mut Option<VaultUiAction>,
    ) {
        if self.unlocked_vault.is_some() {
            self.render_unlocked_vault(ui, ctx, runtime, store, action);
        } else {
            self.render_locked_vault(ui, ctx, runtime, store);
        }
    }

    fn render_locked_vault(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        runtime: &Runtime,
        store: &Store,
    ) {
        let has_vault = *self
            .vault_exists
            .get_or_insert_with(|| store.load_vault_blob().unwrap_or(None).is_some());

        ui.vertical_centered(|ui| {
            ui.add_space(40.0);
            ui.heading(if has_vault {
                "🔐 Zero-Knowledge Encrypted Vault is Locked"
            } else {
                "✨ Initialize Your Encrypted Master Vault"
            });
            ui.add_space(8.0);
            ui.label(
                RichText::new(
                    "Your secrets are encrypted with AES-256-GCM authenticated AEAD and PBKDF2-HMAC-SHA256 (100k iterations).",
                )
                .weak()
                .size(11.5),
            );
            ui.add_space(16.0);

            let resp = ui.add(
                TextEdit::singleline(&mut self.master_password_input)
                    .password(true)
                    .hint_text("Enter Master Password…")
                    .desired_width(260.0),
            );

            if let Some(err) = &self.vault_error {
                ui.add_space(4.0);
                ui.label(RichText::new(err).color(Color32::from_rgb(235, 87, 87)));
            }

            ui.add_space(12.0);
            let unlock_clicked = if has_vault {
                ui.add_enabled(
                    !self.task_pending,
                    egui::Button::new(RichText::new(" 🔓 Unlock Vault ").strong()),
                )
                .clicked()
            } else {
                false
            };
            let create_clicked = if !has_vault {
                ui.add_enabled(
                    !self.task_pending,
                    egui::Button::new(RichText::new(" 🛡 Create & Encrypt New Vault ").strong()),
                )
                .clicked()
            } else {
                false
            };
            if self.task_pending {
                ui.spinner();
                ui.label("Working in background…");
            }
            if has_vault
                && !self.task_pending
                && (unlock_clicked
                    || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))))
                && !self.master_password_input.is_empty()
            {
                self.start_unlock(runtime, ctx, store);
            } else if !has_vault
                && create_clicked
                && !self.master_password_input.is_empty()
            {
                self.start_create(runtime, ctx, store);
            }
        });
    }

    fn render_unlocked_vault(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        runtime: &Runtime,
        store: &Store,
        action: &mut Option<VaultUiAction>,
    ) {
        if let Some(mut entry) = self.editing_entry.take() {
            let mut save_entry = false;
            let mut cancel_entry = false;

            ui.horizontal(|ui| {
                ui.heading("Edit Secret Entry");
            });
            ui.separator();

            egui::Grid::new("vault_edit_grid")
                .num_columns(2)
                .spacing([12.0, 8.0])
                .show(ui, |ui| {
                    ui.label(RichText::new("Title:").strong());
                    ui.text_edit_singleline(&mut entry.title);
                    ui.end_row();

                    ui.label(RichText::new("Category:").strong());
                    egui::ComboBox::from_id_salt("vault_entry_cat")
                        .selected_text(entry.category.display_name())
                        .show_ui(ui, |ui| {
                            ui.selectable_value(
                                &mut entry.category,
                                VaultCategory::Password,
                                "🔑 Password",
                            );
                            ui.selectable_value(
                                &mut entry.category,
                                VaultCategory::SshPrivateKey,
                                "🛡 SSH Private Key",
                            );
                            ui.selectable_value(
                                &mut entry.category,
                                VaultCategory::ApiToken,
                                "⚡ API / Access Token",
                            );
                            ui.selectable_value(
                                &mut entry.category,
                                VaultCategory::Note,
                                "📝 Secure Note",
                            );
                        });
                    ui.end_row();

                    ui.label(RichText::new("Username / Key ID:").strong());
                    ui.text_edit_singleline(&mut entry.username);
                    ui.end_row();

                    ui.label(RichText::new("Secret / Password:").strong());
                    ui.text_edit_singleline(&mut entry.secret);
                    ui.end_row();

                    ui.label(RichText::new("Notes:").strong());
                    ui.add(
                        TextEdit::multiline(&mut entry.notes)
                            .desired_rows(3)
                            .desired_width(320.0),
                    );
                    ui.end_row();
                });

            ui.add_space(10.0);
            ui.horizontal(|ui| {
                if ui
                    .add_enabled(
                        !self.task_pending,
                        egui::Button::new(RichText::new(" Save Secret ").strong()),
                    )
                    .clicked()
                {
                    save_entry = true;
                }
                if ui
                    .add_enabled(!self.task_pending, egui::Button::new("Cancel"))
                    .clicked()
                {
                    cancel_entry = true;
                }
            });

            if save_entry {
                if let Some(mut vault) = self.unlocked_vault.clone() {
                    if vault.get_entry(&entry.id).is_some() {
                        vault.update_entry(entry.clone());
                    } else {
                        vault.add_entry(entry.clone());
                    }
                    self.editing_entry = Some(entry);
                    self.start_save_vault(runtime, ctx, store, vault);
                }
            } else if !cancel_entry {
                self.editing_entry = Some(entry);
            }
            return;
        }

        ui.horizontal(|ui| {
            ui.label(RichText::new("Filter:").strong());
            ui.add(
                TextEdit::singleline(&mut self.vault_search)
                    .hint_text("search title, username, notes…")
                    .desired_width(200.0),
            );

            ui.checkbox(&mut self.show_secrets, "👁 Show Secrets");

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui
                    .add_enabled(
                        !self.task_pending,
                        egui::Button::new(RichText::new(" + Add Secret ").strong()),
                    )
                    .clicked()
                {
                    self.editing_entry = Some(VaultEntry::new(
                        "New Credential",
                        VaultCategory::Password,
                        "",
                        "",
                        "",
                    ));
                }
            });
        });

        ui.add_space(4.0);

        let Some(vault) = &mut self.unlocked_vault else {
            return;
        };
        let entries = if self.vault_search.is_empty() {
            vault.entries().to_vec()
        } else {
            vault
                .search(&self.vault_search)
                .into_iter()
                .cloned()
                .collect()
        };

        if entries.is_empty() {
            ui.vertical_centered(|ui| {
                ui.add_space(40.0);
                ui.label(
                    RichText::new("Vault is empty. Click '+ Add Secret' to store credentials.")
                        .weak()
                        .italics(),
                );
            });
            return;
        }

        let mut entry_to_delete = None;
        let mut entry_to_edit = None;

        ScrollArea::vertical().max_height(400.0).show(ui, |ui| {
            for e in entries {
                let frame = egui::Frame::group(ui.style())
                    .fill(ui.visuals().window_fill().gamma_multiply(0.4))
                    .stroke(Stroke::new(
                        1.0_f32,
                        ui.visuals().weak_text_color().gamma_multiply(0.15),
                    ))
                    .inner_margin(6.0);

                frame.show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(e.category.icon()).size(14.0));
                        ui.label(RichText::new(&e.title).strong().size(12.5));
                        if !e.username.is_empty() {
                            ui.label(RichText::new(format!("({})", e.username)).weak().size(11.5));
                        }

                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if ui
                                .add_enabled(
                                    !self.task_pending,
                                    egui::Button::new(
                                        RichText::new("🗑").color(Color32::from_rgb(235, 87, 87)),
                                    ),
                                )
                                .clicked()
                            {
                                entry_to_delete = Some(e.id.clone());
                            }
                            if ui
                                .add_enabled(!self.task_pending, egui::Button::new("✏ Edit"))
                                .clicked()
                            {
                                entry_to_edit = Some(e.clone());
                            }
                            if ui.button("📋 Copy Secret").clicked() {
                                *action = Some(VaultUiAction::CopyText(e.secret.clone()));
                            }
                        });
                    });

                    ui.horizontal(|ui| {
                        if self.show_secrets {
                            ui.label(
                                RichText::new(&e.secret)
                                    .monospace()
                                    .color(Color32::from_rgb(74, 222, 128)),
                            );
                        } else {
                            ui.label(RichText::new("••••••••••••••••").monospace().weak());
                        }

                        if !e.notes.is_empty() {
                            ui.label(
                                RichText::new(format!("— {}", e.notes))
                                    .weak()
                                    .italics()
                                    .size(11.0),
                            );
                        }
                    });
                });
                ui.add_space(3.0);
            }
        });

        if let Some(id) = entry_to_delete
            && let Some(mut vault) = self.unlocked_vault.clone()
        {
            vault.delete_entry(&id);
            self.start_delete_entry(runtime, ctx, store, vault);
        }
        if let Some(e) = entry_to_edit {
            self.editing_entry = Some(e);
        }
    }

    fn render_keys_tab(
        &mut self,
        ui: &mut egui::Ui,
        ctx: &egui::Context,
        runtime: &Runtime,
        store: &Store,
        hosts: &[Host],
        action: &mut Option<VaultUiAction>,
    ) {
        ui.horizontal(|ui| {
            ui.label(RichText::new("Generate & Deploy Cryptographic SSH Keypairs").strong());
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button(" 🔄 Rescan ~/.ssh ").clicked() {
                    self.refresh_keys(store);
                }
            });
        });

        ui.separator();

        // Key Generator Group
        egui::Frame::group(ui.style())
            .fill(ui.visuals().window_fill().gamma_multiply(0.4))
            .inner_margin(8.0)
            .show(ui, |ui| {
                ui.label(RichText::new("⚡ Generate New SSH Keypair").strong());
                ui.add_space(4.0);

                egui::Grid::new("keygen_grid")
                    .num_columns(2)
                    .spacing([12.0, 6.0])
                    .show(ui, |ui| {
                        ui.label(RichText::new("Algorithm:").strong());
                        egui::ComboBox::from_id_salt("keygen_algo")
                            .selected_text(self.key_algo.display_name())
                            .show_ui(ui, |ui| {
                                ui.selectable_value(
                                    &mut self.key_algo,
                                    KeyAlgorithm::Ed25519,
                                    "Ed25519 (Recommended)",
                                );
                                ui.selectable_value(
                                    &mut self.key_algo,
                                    KeyAlgorithm::Rsa4096,
                                    "RSA 4096-bit",
                                );
                                ui.selectable_value(
                                    &mut self.key_algo,
                                    KeyAlgorithm::Rsa2048,
                                    "RSA 2048-bit",
                                );
                            });
                        ui.end_row();

                        ui.label(RichText::new("Comment / Tag:").strong());
                        ui.text_edit_singleline(&mut self.key_comment);
                        ui.end_row();

                        ui.label(RichText::new("Passphrase (Optional):").strong());
                        ui.add(TextEdit::singleline(&mut self.key_passphrase).password(true));
                        ui.end_row();
                    });

                ui.add_space(6.0);
                if ui
                    .add_enabled(
                        !self.task_pending,
                        egui::Button::new(RichText::new(" 🔑 Generate Keypair ").strong()),
                    )
                    .clicked()
                {
                    self.start_generate_key(runtime, ctx, store);
                }
                if self.task_pending {
                    ui.spinner();
                    ui.label("Generating in background…");
                }
                if let Some(err) = &self.gen_error {
                    ui.label(RichText::new(err).color(Color32::from_rgb(235, 87, 87)));
                }
            });

        ui.add_space(8.0);
        ui.heading("Key Inventory");

        ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
            if self.generated_keys.is_empty() && self.local_keys.is_empty() {
                ui.label(RichText::new("No keys in inventory.").weak().italics());
                return;
            }

            if !self.generated_keys.is_empty() {
                ui.label(
                    RichText::new("Kervesh Generated Keys:")
                        .strong()
                        .color(Color32::from_rgb(90, 160, 245)),
                );
                for k in &self.generated_keys {
                    let frame = egui::Frame::group(ui.style())
                        .fill(ui.visuals().window_fill().gamma_multiply(0.4))
                        .stroke(Stroke::new(
                            1.0_f32,
                            ui.visuals().weak_text_color().gamma_multiply(0.15),
                        ))
                        .inner_margin(6.0);

                    frame.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(k.algorithm.display_name()).strong());
                            ui.label(RichText::new(format!("\"{}\"", k.comment)).weak());
                            ui.label(
                                RichText::new(&k.fingerprint_sha256)
                                    .monospace()
                                    .size(11.0)
                                    .color(Color32::from_rgb(74, 222, 128)),
                            );

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if ui
                                    .button(
                                        RichText::new("🗑").color(Color32::from_rgb(235, 87, 87)),
                                    )
                                    .clicked()
                                {
                                    let _ = store.delete_generated_key(&k.id);
                                }
                                if ui.button("📋 Copy Private").clicked() {
                                    *action = Some(VaultUiAction::CopyText(
                                        k.private_key_openssh.clone(),
                                    ));
                                }
                                if ui.button("📋 Copy Public").clicked() {
                                    *action =
                                        Some(VaultUiAction::CopyText(k.public_key_openssh.clone()));
                                }
                            });
                        });

                        ui.add_space(2.0);
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("Deploy to host:").weak().size(11.0));
                            for h in hosts {
                                if ui
                                    .small_button(&h.name)
                                    .on_hover_text("Append public key to remote authorized_keys")
                                    .clicked()
                                {
                                    *action = Some(VaultUiAction::DeployKeyToHost {
                                        host_id: h.id.clone(),
                                        public_key: k.public_key_openssh.clone(),
                                    });
                                }
                            }
                        });
                    });
                    ui.add_space(3.0);
                }
            }

            if !self.local_keys.is_empty() {
                ui.add_space(6.0);
                ui.label(
                    RichText::new("Local ~/.ssh Discovered Keys:")
                        .strong()
                        .color(Color32::from_rgb(180, 100, 240)),
                );
                for l in &self.local_keys {
                    let frame = egui::Frame::group(ui.style())
                        .fill(ui.visuals().window_fill().gamma_multiply(0.4))
                        .stroke(Stroke::new(
                            1.0_f32,
                            ui.visuals().weak_text_color().gamma_multiply(0.15),
                        ))
                        .inner_margin(6.0);

                    frame.show(ui, |ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&l.filename).strong());
                            ui.label(RichText::new(format!("({})", l.key_type)).weak().size(11.0));
                            if let Some(fp) = &l.fingerprint {
                                ui.label(
                                    RichText::new(fp)
                                        .monospace()
                                        .size(11.0)
                                        .color(Color32::from_rgb(74, 222, 128)),
                                );
                            }

                            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                if let Some(pub_preview) = &l.public_key_preview
                                    && ui.button("📋 Copy Public Key").clicked()
                                {
                                    *action = Some(VaultUiAction::CopyText(pub_preview.clone()));
                                }
                            });
                        });
                    });
                    ui.add_space(3.0);
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::{VaultTaskResult, VaultUi};
    use kervesh_core::EncryptedVault;

    #[test]
    fn stale_result_cannot_reopen_vault_after_invalidation() {
        let mut ui = VaultUi::new();
        ui.open = true;
        let old_generation = ui.begin_operation().expect("operation should start");

        ui.invalidate_operations();
        ui.open = true;
        assert!(!ui.task_pending);

        ui.task_tx
            .send(VaultTaskResult::Unlock {
                generation: old_generation,
                result: Ok(EncryptedVault::empty()),
            })
            .expect("result channel should be open");
        ui.poll_operations();

        assert!(ui.unlocked_vault.is_none());
        assert!(!ui.task_pending);
    }

    #[test]
    fn current_generation_result_is_applied() {
        let mut ui = VaultUi::new();
        ui.open = true;
        let generation = ui.begin_operation().expect("operation should start");

        ui.task_tx
            .send(VaultTaskResult::Unlock {
                generation,
                result: Ok(EncryptedVault::empty()),
            })
            .expect("result channel should be open");
        ui.poll_operations();

        assert!(ui.unlocked_vault.is_some());
        assert!(!ui.task_pending);
    }
}

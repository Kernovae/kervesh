use eframe::egui::{self, Align, Color32, Layout, RichText, ScrollArea, Stroke, TextEdit};
use kervesh_core::{
    EncryptedVault, GeneratedKeypair, Host, KeyAlgorithm, LocalSshKeyInfo, Store, VaultCategory,
    VaultEntry, discover_local_ssh_keys,
};

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
}

impl Default for VaultUi {
    fn default() -> Self {
        Self::new()
    }
}

impl VaultUi {
    pub fn new() -> Self {
        Self {
            open: false,
            selected_tab: 0,
            master_password_input: String::new(),
            new_master_password: String::new(),
            unlocked_vault: None,
            vault_error: None,
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
        }
    }

    pub fn refresh_keys(&mut self, store: &Store) {
        self.generated_keys = store.generated_keys().unwrap_or_default();
        self.local_keys = discover_local_ssh_keys();
    }

    pub fn render(
        &mut self,
        ctx: &egui::Context,
        store: &Store,
        hosts: &[Host],
        action: &mut Option<VaultUiAction>,
    ) {
        if !self.open {
            return;
        }

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
                                self.unlocked_vault = None;
                                self.master_password_input.clear();
                            }
                        });
                    }
                });

                ui.separator();

                match self.selected_tab {
                    0 => self.render_vault_tab(ui, store, action),
                    1 => self.render_keys_tab(ui, store, hosts, action),
                    _ => {}
                }
            });

        self.open = is_open;
    }

    fn render_vault_tab(
        &mut self,
        ui: &mut egui::Ui,
        store: &Store,
        action: &mut Option<VaultUiAction>,
    ) {
        if self.unlocked_vault.is_some() {
            self.render_unlocked_vault(ui, store, action);
        } else {
            self.render_locked_vault(ui, store);
        }
    }

    fn render_locked_vault(&mut self, ui: &mut egui::Ui, store: &Store) {
        let has_vault = store.load_vault_blob().unwrap_or(None).is_some();

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
            if has_vault
                && (ui.button(RichText::new(" 🔓 Unlock Vault ").strong()).clicked()
                    || (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))))
                && !self.master_password_input.is_empty()
                && let Ok(Some(blob)) = store.load_vault_blob()
            {
                match EncryptedVault::unlock(&blob, &self.master_password_input) {
                    Ok(v) => {
                        self.unlocked_vault = Some(v);
                        self.vault_error = None;
                    }
                    Err(e) => {
                        self.vault_error = Some(e.to_string());
                    }
                }
            } else if !has_vault
                && ui
                    .button(RichText::new(" 🛡 Create & Encrypt New Vault ").strong())
                    .clicked()
                && !self.master_password_input.is_empty()
            {
                let vault = EncryptedVault::empty();
                match vault.encrypt_to_blob(&self.master_password_input) {
                    Ok(blob) => {
                        let _ = store.save_vault_blob(&blob);
                        self.unlocked_vault = Some(vault);
                        self.vault_error = None;
                    }
                    Err(e) => {
                        self.vault_error = Some(e.to_string());
                    }
                }
            }
        });
    }

    fn render_unlocked_vault(
        &mut self,
        ui: &mut egui::Ui,
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
                if ui.button(RichText::new(" Save Secret ").strong()).clicked() {
                    save_entry = true;
                }
                if ui.button("Cancel").clicked() {
                    cancel_entry = true;
                }
            });

            if save_entry {
                if let Some(v) = &mut self.unlocked_vault {
                    if v.get_entry(&entry.id).is_some() {
                        v.update_entry(entry);
                    } else {
                        v.add_entry(entry);
                    }
                    if let Ok(blob) = v.encrypt_to_blob(&self.master_password_input) {
                        let _ = store.save_vault_blob(&blob);
                    }
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
                    .button(RichText::new(" + Add Secret ").strong())
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
                                .button(RichText::new("🗑").color(Color32::from_rgb(235, 87, 87)))
                                .clicked()
                            {
                                entry_to_delete = Some(e.id.clone());
                            }
                            if ui.button("✏ Edit").clicked() {
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
            && let Some(v) = &mut self.unlocked_vault
        {
            v.delete_entry(&id);
            if let Ok(blob) = v.encrypt_to_blob(&self.master_password_input) {
                let _ = store.save_vault_blob(&blob);
            }
        }
        if let Some(e) = entry_to_edit {
            self.editing_entry = Some(e);
        }
    }

    fn render_keys_tab(
        &mut self,
        ui: &mut egui::Ui,
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
                    .button(RichText::new(" 🔑 Generate Keypair ").strong())
                    .clicked()
                {
                    let pass = if self.key_passphrase.is_empty() {
                        None
                    } else {
                        Some(self.key_passphrase.as_str())
                    };

                    match GeneratedKeypair::generate(self.key_algo, &self.key_comment, pass) {
                        Ok(kp) => {
                            let _ = store.save_generated_key(&kp);
                            self.refresh_keys(store);
                            self.gen_error = None;
                        }
                        Err(e) => {
                            self.gen_error = Some(e.to_string());
                        }
                    }
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

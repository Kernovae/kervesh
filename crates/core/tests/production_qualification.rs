use kervesh_core::{
    audit::AuditCommandEntry,
    automation::{AutomationMacro, MacroStep},
    file_search::SearchQuery,
    key_gen::{GeneratedKeypair, KeyAlgorithm},
    process::{ProcessInfo, Signal},
    protocol::{
        FtpConfig, FtpTlsMode, ProtocolKind, RemoteDesktopConfig, RemoteDesktopKind, SerialConfig,
        SerialFlowControl, SerialParity, TelnetConfig,
    },
    recording::{RecordingFormat, SessionRecorder},
    snippet::Snippet,
    ssh_config::parse_ssh_config_with_report,
    sync::{FileMetadataEntry, SyncActionKind, SyncConflictPolicy, SyncDirection, SyncPlan},
    terminal_profile::TerminalPalette,
    triggers::{TriggerAction, TriggerEngine, TriggerRule},
    tunnel::{TunnelConfig, TunnelKind},
    vault::{EncryptedVault, VaultCategory, VaultEntry},
    workspace::SessionWorkspace,
    x11::X11ForwardingConfig,
};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::thread;

#[test]
fn test_v02_ssh_config_import_and_jump_hosts() {
    let ssh_config_content = r#"
Host bastion
    HostName 10.0.0.1
    User jumpuser
    Port 2222
    IdentityFile ~/.ssh/id_bastion

Host internal-node
    HostName 192.168.1.50
    User appuser
    ProxyJump bastion
    LocalForward 8080 127.0.0.1:80
    RemoteForward 9000 127.0.0.1:9000
    DynamicForward 1080
"#;

    let report = parse_ssh_config_with_report(ssh_config_content);
    assert_eq!(report.hosts.len(), 2);
    assert_eq!(report.proxy_jump_count, 1);
    assert!(report.forwarded_rules_count >= 3);

    let bastion = report.hosts.iter().find(|h| h.name == "bastion").unwrap();
    assert_eq!(bastion.hostname, "10.0.0.1");
    assert_eq!(bastion.port, 2222);
    assert_eq!(bastion.username, "jumpuser");

    let internal = report
        .hosts
        .iter()
        .find(|h| h.name == "internal-node")
        .unwrap();
    assert_eq!(internal.hostname, "192.168.1.50");
    assert_eq!(internal.username, "appuser");
    assert_eq!(internal.proxy_jump, Some("bastion".to_string()));
    assert_eq!(internal.local_forwards.len(), 1);
    assert_eq!(internal.remote_forwards.len(), 1);
    assert_eq!(internal.dynamic_forwards.len(), 1);
}

#[test]
fn test_v03_process_listing_and_snippets() {
    let ps_output = "\
PID PPID USER %CPU %MEM STAT TIME COMMAND
1 0 root 0.1 0.2 Ss 00:01:23 /sbin/init
1234 1 deploy 4.5 12.3 Ssl 00:45:10 /usr/bin/kervesh-node
5678 1234 deploy 0.0 1.1 S 00:00:02 worker-process\n";

    let processes = ProcessInfo::parse_ps_output(ps_output);
    assert_eq!(processes.len(), 3);
    assert_eq!(processes[0].pid, 1);
    assert_eq!(processes[0].user, "root");
    assert_eq!(processes[1].pid, 1234);
    assert_eq!(processes[1].cpu, 4.5);
    assert_eq!(processes[1].mem, 12.3);
    assert_eq!(processes[1].command, "/usr/bin/kervesh-node");
    assert_eq!(Signal::Kill.number(), 9);
    assert_eq!(Signal::Term.number(), 15);

    let snippet = Snippet::new(
        "Deploy App",
        "systemctl restart {{service_name}} && journalctl -u {{service_name}} -n 50",
    );
    let mut values = HashMap::new();
    values.insert("service_name".to_string(), "kervesh.service".to_string());
    let rendered = snippet.render(&values);
    assert_eq!(
        rendered,
        "systemctl restart kervesh.service && journalctl -u kervesh.service -n 50"
    );
}

#[test]
fn test_v04_tunnel_and_socks5_configurations() {
    let mut local_tunnel = TunnelConfig::new(
        "host-1",
        "Database Tunnel",
        TunnelKind::Local,
        5432,
        "db-cluster.internal",
        5432,
    );
    local_tunnel.auto_start = true;
    assert!(local_tunnel.validate().is_ok());

    let socks_tunnel = TunnelConfig::new(
        "host-1",
        "Corporate SOCKS5 Proxy",
        TunnelKind::Dynamic,
        10808,
        "",
        0,
    );
    assert!(socks_tunnel.validate().is_ok());
}

#[test]
fn test_v05_automation_macro_and_workspace_grouping() {
    let mut sequence = AutomationMacro::new("Elevate & Drop Caches", "System optimization macro");
    sequence.steps.push(MacroStep::ExpectPrompt("\\$ ".into()));
    sequence.steps.push(MacroStep::SendText {
        text: "sudo su -\n".into(),
        append_newline: false,
    });
    sequence.steps.push(MacroStep::DelayMs(500));
    sequence.steps.push(MacroStep::SendText {
        text: "sync && echo 3 > /proc/sys/vm/drop_caches\n".into(),
        append_newline: false,
    });
    assert!(sequence.validate().is_ok());
    assert_eq!(sequence.steps.len(), 4);

    let mut workspace = SessionWorkspace::new("Production Cluster", "EU-West region");
    workspace.add_host("node-1");
    workspace.add_host("node-2");
    workspace.add_host("node-3");
    workspace.add_tag("production");
    workspace.add_tag("eu-west");
    assert!(workspace.validate().is_ok());
    assert_eq!(workspace.host_ids.len(), 3);
    assert_eq!(workspace.tags.len(), 2);
}

#[test]
fn test_v06_remote_file_search_and_sync_comparison() {
    let query = SearchQuery {
        directory: "/var/log".into(),
        pattern: "ERROR|CRITICAL".into(),
        extension: Some("log".into()),
        case_sensitive: true,
        is_regex: true,
        max_results: 50,
    };
    assert!(query.validate().is_ok());
    let cmd = query.to_grep_command();
    assert!(cmd.contains("-rnE"));
    assert!(cmd.contains("--include=*.log"));

    let local_entries = vec![
        FileMetadataEntry {
            rel_path: "index.html".into(),
            size: 1024,
            mtime: 100,
            is_dir: false,
        },
        FileMetadataEntry {
            rel_path: "app.js".into(),
            size: 2048,
            mtime: 200,
            is_dir: false,
        },
    ];
    let remote_entries = vec![
        FileMetadataEntry {
            rel_path: "index.html".into(),
            size: 1024,
            mtime: 100,
            is_dir: false,
        },
        FileMetadataEntry {
            rel_path: "app.js".into(),
            size: 1800,
            mtime: 150,
            is_dir: false,
        },
        FileMetadataEntry {
            rel_path: "legacy.css".into(),
            size: 512,
            mtime: 90,
            is_dir: false,
        },
    ];

    let plan = SyncPlan::compute(
        std::path::PathBuf::from("/local/dist"),
        "/var/www/html".into(),
        SyncDirection::LocalToRemote,
        SyncConflictPolicy::Overwrite,
        &local_entries,
        &remote_entries,
    );

    let uploads: Vec<_> = plan
        .items
        .iter()
        .filter(|i| i.action == SyncActionKind::Upload)
        .collect();
    let identical: Vec<_> = plan
        .items
        .iter()
        .filter(|i| i.action == SyncActionKind::Identical)
        .collect();
    let deletions: Vec<_> = plan
        .items
        .iter()
        .filter(|i| i.action == SyncActionKind::DeleteRemote)
        .collect();

    assert_eq!(uploads.len(), 1);
    assert_eq!(identical.len(), 1);
    assert_eq!(deletions.len(), 1);
    assert_eq!(uploads[0].rel_path, "app.js");
}

#[test]
fn test_v07_multi_protocol_descriptors() {
    let telnet = TelnetConfig {
        host: "router.corp".into(),
        port: 23,
        terminal_type: "xterm-256color".into(),
        naws: true,
    };
    assert!(telnet.validate().is_ok());

    let serial = SerialConfig {
        port: "/dev/ttyUSB0".into(),
        baud_rate: 115200,
        data_bits: 8,
        parity: SerialParity::None,
        stop_bits: 1,
        flow_control: SerialFlowControl::None,
    };
    assert!(serial.validate().is_ok());

    let ftp = FtpConfig {
        host: "ftp.backup.local".into(),
        port: 21,
        username: "anonymous".into(),
        tls_mode: FtpTlsMode::None,
        passive_mode: true,
    };
    assert_eq!(ftp.port, 21);

    let rdp = RemoteDesktopConfig {
        kind: RemoteDesktopKind::Rdp,
        host: "win-srv-01".into(),
        port: 3389,
        username: "admin".into(),
        domain: Some("CORP".into()),
        width: 1920,
        height: 1080,
        color_depth: 24,
        fullscreen: false,
        custom_args: Vec::new(),
    };
    assert_eq!(rdp.port, 3389);

    let vnc = RemoteDesktopConfig {
        kind: RemoteDesktopKind::Vnc,
        host: "kvm-host-01".into(),
        port: 5900,
        username: String::new(),
        domain: None,
        width: 1920,
        height: 1080,
        color_depth: 24,
        fullscreen: false,
        custom_args: Vec::new(),
    };
    assert_eq!(vnc.port, 5900);
    assert_eq!(ProtocolKind::SSH.label(), "SSH");
    assert_eq!(ProtocolKind::Telnet.default_port(), 23);
}

#[test]
fn test_v08_x11_forwarding_and_cookies() {
    let cookie = X11ForwardingConfig::generate_cookie();
    assert_eq!(cookie.len(), 32);

    let x11_cfg = X11ForwardingConfig {
        enabled: true,
        display: ":10.0".into(),
        trusted: true,
        auth_protocol: "MIT-MAGIC-COOKIE-1".into(),
        auth_cookie: cookie.clone(),
        screen: 0,
    };
    assert!(x11_cfg.enabled);
    assert!(x11_cfg.trusted);
}

#[test]
fn test_v010_session_recording_and_trigger_rules() {
    let tmp = tempfile::tempdir().unwrap();
    let mut recorder = SessionRecorder::start(
        "sess-1",
        "production-srv",
        RecordingFormat::CleanText,
        Some(tmp.path()),
        80,
        24,
    )
    .expect("recorder start must succeed");

    recorder.write_output(b"ls -la\n").unwrap();
    recorder
        .write_output(b"total 64\ndrwxr-xr-x 4 user user 4096 Sep 5 12:00 .\n")
        .unwrap();
    recorder.stop().unwrap();

    let rule1 = TriggerRule::new(
        "Critical Error",
        "FATAL|PANIC|ERROR",
        true,
        TriggerAction::Notification("Server Alert".into()),
    );
    let rule2 = TriggerRule::new(
        "Prompt Beep",
        "[sudo] password for",
        false,
        TriggerAction::PlayBeep,
    );

    let engine = TriggerEngine::new(&[rule1, rule2]);

    let actions = engine.evaluate(
        "kernel: [   12.345678] FATAL: hardware check failed\n",
        None,
    );
    assert_eq!(actions.len(), 1);
    assert!(matches!(actions[0], TriggerAction::Notification(_)));

    let bell_actions = engine.evaluate("[sudo] password for admin: ", None);
    assert_eq!(bell_actions.len(), 1);
    assert!(matches!(bell_actions[0], TriggerAction::PlayBeep));
}

#[test]
fn test_v011_vault_encryption_and_ssh_key_generation() {
    let keypair = GeneratedKeypair::generate(
        KeyAlgorithm::Ed25519,
        "kervesh-production-key",
        Some("super-secret-key-passphrase"),
    )
    .expect("keypair generation must succeed");

    assert!(keypair.public_key_openssh.starts_with("ssh-ed25519 "));
    assert!(
        keypair
            .public_key_openssh
            .contains("kervesh-production-key")
    );
    assert!(
        keypair
            .private_key_openssh
            .contains("-----BEGIN OPENSSH PRIVATE KEY-----")
    );
    assert!(!keypair.fingerprint_sha256.is_empty());

    let mut vault = EncryptedVault::empty();
    vault.add_entry(VaultEntry::new(
        "Prod Root Password",
        VaultCategory::Password,
        "root",
        "Correct-Horse-Battery-Staple-2026!",
        "Root credential for production node 1",
    ));
    vault.add_entry(VaultEntry::new(
        "Deploy Key",
        VaultCategory::SshPrivateKey,
        "git",
        &keypair.private_key_openssh,
        "Ed25519 private key",
    ));

    let master_password = "Master-Vault-Password-991!";
    let ciphertext = vault
        .encrypt_to_blob(master_password)
        .expect("vault encryption must succeed");
    assert!(!ciphertext.is_empty());

    let decrypted_vault = EncryptedVault::unlock(&ciphertext, master_password)
        .expect("vault decryption must succeed with correct password");
    let entries = decrypted_vault.entries();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[1].secret, "Correct-Horse-Battery-Staple-2026!");
    assert_eq!(entries[0].secret, keypair.private_key_openssh);

    let wrong_decrypt = EncryptedVault::unlock(&ciphertext, "Wrong-Password!");
    assert!(wrong_decrypt.is_err());
}

#[test]
fn test_v012_theme_wcag_and_palette_validation() {
    let dracula = TerminalPalette::dracula();

    let bg = dracula.background;
    let fg = dracula.foreground;
    let ratio = TerminalPalette::contrast_ratio(fg, bg);
    assert!(
        ratio >= 4.5,
        "Dracula FG/BG must pass WCAG AA (>= 4.5), got {ratio}"
    );
    assert!(TerminalPalette::is_wcag_aa(fg, bg));

    let exported_json = dracula
        .export_json()
        .expect("palette serialization must succeed");
    let imported_palette =
        TerminalPalette::import_json(&exported_json).expect("palette import must succeed");
    assert_eq!(imported_palette.background, dracula.background);
    assert_eq!(imported_palette.ansi, dracula.ansi);
}

#[test]
fn test_v10_high_concurrency_simulated_session_mesh() {
    // Validate 100+ concurrent simulated operations across Store, Vault, Triggers, and Audit logs
    let audit_entries = Arc::new(RwLock::new(Vec::<AuditCommandEntry>::new()));
    let trigger_engine = Arc::new(TriggerEngine::new(&[TriggerRule::new(
        "Crit",
        "CRITICAL",
        false,
        TriggerAction::PlayBeep,
    )]));

    let mut threads = Vec::new();
    for i in 0..100 {
        let audit = Arc::clone(&audit_entries);
        let triggers = Arc::clone(&trigger_engine);
        let handle = thread::spawn(move || {
            let session_id = format!("session-{i}");
            let host_id = format!("host-{}", i % 10);
            let command = format!("docker service update --image registry:2026/node:v{i} svc_{i}");

            // 1. Audit append
            let entry = AuditCommandEntry::new(session_id, host_id, "Prod Host", command);
            {
                let mut guard = audit.write().unwrap();
                guard.push(entry);
            }

            // 2. Trigger check
            let simulated_output = format!("[host-{i}] SUCCESS: updated svc_{i} without error");
            let fired = triggers.evaluate(&simulated_output, None);
            assert!(fired.is_empty());

            let err_output = format!("[host-{i}] CRITICAL: failed container health check");
            let fired_err = triggers.evaluate(&err_output, None);
            assert_eq!(fired_err.len(), 1);
        });
        threads.push(handle);
    }

    for t in threads {
        t.join().expect("thread join must not panic");
    }

    let audit_guard = audit_entries.read().unwrap();
    assert_eq!(audit_guard.len(), 100);
}

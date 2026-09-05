use kervesh_core::{Host, Snapshot, Store, Trust};

fn host() -> Host {
    Host {
        name: "Lab".into(),
        hostname: "::1".into(),
        username: "operator".into(),
        ..Host::default()
    }
}

#[test]
fn host_validation_rejects_missing_fields_and_invalid_port() {
    assert!(Host::default().validate().is_err());
    let mut h = host();
    assert!(h.validate().is_ok());
    h.port = 0;
    assert!(h.validate().is_err());
    h.port = 22;
    h.hostname = "bad\nhost".into();
    assert!(h.validate().is_err());
}

#[test]
fn store_roundtrip_duplicate_and_delete_survive_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("state.db");
    let store = Store::open(&path).unwrap();
    let h = host();
    store.save_host(&h).unwrap();
    let mut copy = h.duplicate();
    assert_ne!(h.id, copy.id);
    copy.favorite = true;
    store.save_host(&copy).unwrap();
    store.delete_host(&h.id).unwrap();
    drop(store);
    let hosts = Store::open(&path).unwrap().hosts().unwrap();
    assert_eq!(hosts.len(), 1);
    assert_eq!(hosts[0].id, copy.id);
    assert!(hosts[0].favorite);
}

#[test]
fn export_excludes_trust_and_import_is_atomic_with_fresh_ids() {
    let store = Store::open_memory().unwrap();
    let h = host();
    store.save_host(&h).unwrap();
    store
        .trust(&h.hostname, h.port, "SHA256:private-trust")
        .unwrap();
    let export = store.export().unwrap();
    assert!(!export.contains("private-trust"));
    assert!(!export.contains("password"));
    let other = Store::open_memory().unwrap();
    other.import(&export).unwrap();
    assert_ne!(other.hosts().unwrap()[0].id, h.id);
    let mut invalid: serde_json::Value = serde_json::from_str(&export).unwrap();
    invalid["hosts"][0]["port"] = 0.into();
    let invalid = invalid.to_string();
    assert!(other.import(&invalid).is_err());
    assert_eq!(other.hosts().unwrap().len(), 1);
    assert!(other.import("{\"version\":999,\"hosts\":[]}").is_err());
}

#[test]
fn host_key_changes_fail_closed_and_ports_have_separate_trust() {
    let store = Store::open_memory().unwrap();
    assert_eq!(
        store.check_trust("server", 22, "first").unwrap(),
        Trust::Unknown
    );
    store.trust("server", 22, "first").unwrap();
    assert_eq!(
        store.check_trust("server", 22, "first").unwrap(),
        Trust::Trusted
    );
    assert_eq!(
        store.check_trust("server", 23, "first").unwrap(),
        Trust::Unknown
    );
    assert!(matches!(
        store.check_trust("server", 22, "second").unwrap(),
        Trust::Changed(_)
    ));
    assert!(store.trust("server", 22, "second").is_err());
}

#[test]
fn procfs_metrics_use_deltas_and_handle_counter_resets() {
    let a = Snapshot::parse("@@stat\ncpu 10 0 10 80 0 0 0 0 0 0\n@@mem\nMemTotal: 1000 kB\nMemAvailable: 400 kB\nSwapTotal: 100 kB\nSwapFree: 75 kB\n@@net\neth0: 100 0 0 0 0 0 0 0 200 0 0 0 0 0 0 0\n@@load\n1.0 2.0 3.0 1/100 500\n@@uptime\n3600.0 0\n").unwrap();
    let b = Snapshot::parse(
        "@@stat\ncpu 20 0 20 160 0 0 0 0 0 0\n@@net\neth0: 300 0 0 0 0 0 0 0 600 0 0 0 0 0 0 0\n",
    )
    .unwrap();
    assert_eq!(a.memory_used(), Some(600 * 1024));
    assert_eq!(a.swap_used(), Some(25 * 1024));
    let rates = b.rates(&a, 2.0);
    assert_eq!(rates.cpu, Some(20.0));
    assert_eq!(rates.network["eth0"], (100.0, 200.0));
    assert_eq!(a.rates(&b, 2.0).network["eth0"], (0.0, 0.0));
    assert!(Snapshot::parse("garbage").is_err());
}

#[test]
fn imports_with_secret_fields_or_invalid_preferences_leave_database_unchanged() {
    let store = Store::open_memory().unwrap();
    let h = host();
    store.save_host(&h).unwrap();
    let mut data: serde_json::Value = serde_json::from_str(&store.export().unwrap()).unwrap();
    data["hosts"][0]["password"] = "must-never-persist".into();
    assert!(store.import(&data.to_string()).is_err());
    assert_eq!(store.hosts().unwrap().len(), 1);
    let mut data: serde_json::Value = serde_json::from_str(&store.export().unwrap()).unwrap();
    data["settings"]["monitor_secs"] = 0.into();
    assert!(store.import(&data.to_string()).is_err());
    assert_eq!(store.settings().unwrap().monitor_secs, 2);
}

#[test]
fn malformed_and_guest_cpu_counters_do_not_inflate_utilization() {
    let a = Snapshot::parse("@@stat\ncpu 100 0 50 850 0 0 0 0 90 0\n").unwrap();
    let b = Snapshot::parse("@@stat\ncpu 110 0 60 930 0 0 0 0 100 0\n").unwrap();
    assert_eq!(b.rates(&a, 1.0).cpu, Some(20.0));
    assert_eq!(b.rates(&a, 0.0).cpu, None);
}

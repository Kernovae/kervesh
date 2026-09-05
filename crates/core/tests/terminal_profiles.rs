use kervesh_core::{Settings, Store};
#[test]
fn legacy_terminal_preferences_migrate_without_losing_values() {
    let settings: Settings =
        serde_json::from_str(r#"{"font_size":19.0,"scrollback":4321,"dark":false}"#).unwrap();
    let value = serde_json::to_value(settings).unwrap();
    assert_eq!(value["terminal_profiles"][0]["font_size"], 19.0);
    assert_eq!(value["terminal_profiles"][0]["scrollback"], 4321);
    assert_eq!(
        value["terminal_profiles"][0]["clipboard_profile"],
        "Desktop"
    );
}
#[test]
fn profile_settings_roundtrip_and_reject_invalid_values() {
    let store = Store::open_memory().unwrap();
    let mut value = serde_json::to_value(Settings::default()).unwrap();
    value["terminal_profiles"][0]["line_height"] = serde_json::json!(1.4);
    let settings: Settings = serde_json::from_value(value.clone()).unwrap();
    store.save_settings(&settings).unwrap();
    assert!((store.settings().unwrap().terminal_profiles[0].line_height - 1.4).abs() < 0.0001);
    value["terminal_profiles"][0]["line_height"] = serde_json::json!(0.0);
    let invalid: Settings = serde_json::from_value(value).unwrap();
    assert!(store.save_settings(&invalid).is_err());
}

#[test]
fn host_profile_binding_and_custom_palette_survive_sqlite_and_export() {
    let store = Store::open_memory().unwrap();
    let host = kervesh_core::Host {
        name: "fixture".into(),
        hostname: "example.test".into(),
        username: "test".into(),
        terminal_profile: Some("development".into()),
        ..Default::default()
    };
    store.save_host(&host).unwrap();
    assert_eq!(
        store.hosts().unwrap()[0].terminal_profile.as_deref(),
        Some("development")
    );
    let mut settings = Settings::default();
    settings.terminal_profiles[0].palette.kind = kervesh_core::PaletteKind::Custom;
    settings.terminal_profiles[0].palette.ansi[1] = [123, 45, 67];
    store.save_settings(&settings).unwrap();
    let exported = store.export().unwrap();
    let destination = Store::open_memory().unwrap();
    destination.import(&exported).unwrap();
    assert_eq!(
        destination.settings().unwrap().terminal_profiles[0]
            .palette
            .ansi[1],
        [123, 45, 67]
    );
    assert_ne!(destination.hosts().unwrap()[0].id, host.id);
    assert_eq!(
        destination.hosts().unwrap()[0].terminal_profile.as_deref(),
        Some("development")
    );
}
#[test]
fn invalid_default_and_duplicate_profiles_are_rejected_atomically() {
    let store = Store::open_memory().unwrap();
    let mut settings = Settings {
        default_terminal_profile: "missing".into(),
        ..Default::default()
    };
    assert!(store.save_settings(&settings).is_err());
    settings.default_terminal_profile = "default".into();
    settings.terminal_profiles[1].id = "default".into();
    assert!(store.save_settings(&settings).is_err());
    assert_eq!(
        store.settings().unwrap().default_terminal_profile,
        "default"
    );
}

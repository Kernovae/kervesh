#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum OsTheme {
    Dark,
    Light,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum AppThemePreference {
    Dark,
    Light,
    System,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ResolvedTheme {
    pub os: OsTheme,
    pub app_dark: bool,
}

impl ResolvedTheme {
    pub fn resolve(pref: AppThemePreference, os: OsTheme) -> Self {
        let app_dark = match pref {
            AppThemePreference::Dark => true,
            AppThemePreference::Light => false,
            AppThemePreference::System => os == OsTheme::Dark,
        };
        Self { os, app_dark }
    }

    pub fn app_asset_path(&self) -> &'static str {
        match (self.os, self.app_dark) {
            (OsTheme::Dark, true) => "adaptive/os-dark/app-dark/kervesh-app.svg",
            (OsTheme::Dark, false) => "adaptive/os-dark/app-light/kervesh-app.svg",
            (OsTheme::Light, true) => "adaptive/os-light/app-dark/kervesh-app.svg",
            (OsTheme::Light, false) => "adaptive/os-light/app-light/kervesh-app.svg",
        }
    }

    pub fn titlebar_asset_path(&self) -> &'static str {
        if self.app_dark {
            "adaptive/titlebar/kervesh-titlebar-for-dark-app.svg"
        } else {
            "adaptive/titlebar/kervesh-titlebar-for-light-app.svg"
        }
    }

    pub fn tray_asset_path(&self) -> &'static str {
        match self.os {
            OsTheme::Dark => "adaptive/tray/kervesh-tray-for-dark-os.svg",
            OsTheme::Light => "adaptive/tray/kervesh-tray-for-light-os.svg",
        }
    }
}

// Brand color palette constants
pub mod colors {
    use egui::Color32;

    pub const BLACK: Color32 = Color32::from_rgb(0x0B, 0x0B, 0x0C);
    pub const CHARCOAL: Color32 = Color32::from_rgb(0x11, 0x11, 0x11);
    pub const GRAPHITE: Color32 = Color32::from_rgb(0x2A, 0x2A, 0x2A);
    pub const SLATE: Color32 = Color32::from_rgb(0x3A, 0x3A, 0x3A);
    pub const MUTED: Color32 = Color32::from_rgb(0x8A, 0x8A, 0x8A);
    pub const FOREGROUND: Color32 = Color32::from_rgb(0xED, 0xED, 0xED);
    pub const WHITE: Color32 = Color32::from_rgb(0xFF, 0xFF, 0xFF);

    pub const DANGER: Color32 = Color32::from_rgb(0xC8, 0x5B, 0x62);
    pub const WARNING: Color32 = Color32::from_rgb(0xC4, 0x9A, 0x5A);
    pub const SUCCESS: Color32 = Color32::from_rgb(0x58, 0xA6, 0x7B);
    pub const DISCONNECTED: Color32 = Color32::from_rgb(0x77, 0x77, 0x77);

    // Light mode surfaces
    pub const LIGHT_BG: Color32 = Color32::from_rgb(0xF6, 0xF7, 0xF8);
    pub const LIGHT_PANEL: Color32 = Color32::from_rgb(0xEE, 0xF0, 0xF2);
    pub const LIGHT_BORDER: Color32 = Color32::from_rgb(0xD5, 0xD8, 0xDC);
    pub const LIGHT_FOREGROUND: Color32 = Color32::from_rgb(0x11, 0x11, 0x11);
    pub const LIGHT_MUTED: Color32 = Color32::from_rgb(0x6A, 0x73, 0x7D);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_four_combinations() {
        let combinations = [
            (
                AppThemePreference::Dark,
                OsTheme::Dark,
                "adaptive/os-dark/app-dark/kervesh-app.svg",
                "adaptive/titlebar/kervesh-titlebar-for-dark-app.svg",
                "adaptive/tray/kervesh-tray-for-dark-os.svg",
                true,
            ),
            (
                AppThemePreference::Light,
                OsTheme::Dark,
                "adaptive/os-dark/app-light/kervesh-app.svg",
                "adaptive/titlebar/kervesh-titlebar-for-light-app.svg",
                "adaptive/tray/kervesh-tray-for-dark-os.svg",
                false,
            ),
            (
                AppThemePreference::Dark,
                OsTheme::Light,
                "adaptive/os-light/app-dark/kervesh-app.svg",
                "adaptive/titlebar/kervesh-titlebar-for-dark-app.svg",
                "adaptive/tray/kervesh-tray-for-light-os.svg",
                true,
            ),
            (
                AppThemePreference::Light,
                OsTheme::Light,
                "adaptive/os-light/app-light/kervesh-app.svg",
                "adaptive/titlebar/kervesh-titlebar-for-light-app.svg",
                "adaptive/tray/kervesh-tray-for-light-os.svg",
                false,
            ),
        ];

        for (pref, os, expected_app, expected_title, expected_tray, expected_dark) in combinations {
            let resolved = ResolvedTheme::resolve(pref, os);
            assert_eq!(resolved.app_dark, expected_dark);
            assert_eq!(resolved.app_asset_path(), expected_app);
            assert_eq!(resolved.titlebar_asset_path(), expected_title);
            assert_eq!(resolved.tray_asset_path(), expected_tray);
        }
    }

    #[test]
    fn test_explicit_preference_not_overridden_by_os() {
        let explicit_dark = ResolvedTheme::resolve(AppThemePreference::Dark, OsTheme::Light);
        assert!(explicit_dark.app_dark);

        let explicit_light = ResolvedTheme::resolve(AppThemePreference::Light, OsTheme::Dark);
        assert!(!explicit_light.app_dark);
    }

    #[test]
    fn test_system_preference_follows_os() {
        let system_dark = ResolvedTheme::resolve(AppThemePreference::System, OsTheme::Dark);
        assert!(system_dark.app_dark);

        let system_light = ResolvedTheme::resolve(AppThemePreference::System, OsTheme::Light);
        assert!(!system_light.app_dark);
    }
}

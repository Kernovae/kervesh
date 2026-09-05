use ab_glyph::Font;
use egui::{Context, FontData, FontDefinitions, FontFamily, FontId};
use std::sync::Arc;

#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub struct TerminalFontConfig {
    pub primary: String,
    pub fallbacks: Vec<String>,
}
impl From<&crate::TerminalProfile> for TerminalFontConfig {
    fn from(profile: &crate::TerminalProfile) -> Self {
        Self {
            primary: profile.font_family.clone(),
            fallbacks: profile.font_fallbacks.clone(),
        }
    }
}
impl TerminalFontConfig {
    fn family(&self, bold: bool) -> FontFamily {
        // Length-delimited configuration avoids ambiguous names and survives profile renaming.
        FontFamily::Name(format!("terminal:{:?}:{:?}:{bold}", self.primary, self.fallbacks).into())
    }
    pub fn font_id(&self, size: f32, bold: bool) -> FontId {
        FontId::new(size, self.family(bold))
    }
}
#[derive(Clone, Debug)]
pub struct TerminalFontDiagnostics {
    pub source: String,
    pub message: String,
}
#[derive(Default)]
pub struct TerminalFontManager {
    configs: Vec<TerminalFontConfig>,
    diagnostics: Vec<TerminalFontDiagnostics>,
    initialized: bool,
}
impl TerminalFontManager {
    pub fn diagnostics(&self) -> &[TerminalFontDiagnostics] {
        &self.diagnostics
    }
    /// Call before a frame. Rebuild only when configured sources change.
    pub fn register(&mut self, ctx: &Context, configs: &[TerminalFontConfig]) {
        if self.initialized && self.configs == configs {
            return;
        }
        self.initialized = true;
        self.configs = configs.to_vec();
        self.diagnostics.clear();
        let mut definitions = FontDefinitions::default();
        definitions.font_data.insert(
            "Hack-Bold".into(),
            Arc::new(FontData::from_static(include_bytes!(
                "../../../assets/fonts/Hack-Bold.ttf"
            ))),
        );
        for config in configs {
            let mut chain = Vec::new();
            for source in std::iter::once(&config.primary).chain(&config.fallbacks) {
                if source == "Hack" || source.is_empty() {
                    chain.push("Hack".into());
                    continue;
                }
                let key = format!("terminal-file:{source}");
                if !definitions.font_data.contains_key(&key) {
                    let result = (|| -> Result<Vec<u8>, String> {
                        let path = std::path::Path::new(source);
                        if !path.is_absolute() {
                            return Err("Choose an absolute local TTF/OTF path, or Hack".into());
                        }
                        let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
                        if !meta.is_file() || meta.len() > 32 * 1024 * 1024 {
                            return Err("Font must be a regular file under 32 MiB".into());
                        }
                        let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
                        let face =
                            ab_glyph::FontRef::try_from_slice(&bytes).map_err(|e| e.to_string())?;
                        if !face
                            .units_per_em()
                            .is_some_and(|em| (16.0..=16384.0).contains(&em))
                        {
                            return Err("Unsupported font units per em".into());
                        }
                        if source == &config.primary {
                            let m = face.glyph_id('M');
                            let i = face.glyph_id('i');
                            if m.0 == 0
                                || i.0 == 0
                                || (face.h_advance_unscaled(m) - face.h_advance_unscaled(i)).abs()
                                    > 1.0
                            {
                                return Err(
                                    "Primary font must be monospace and contain ASCII".into()
                                );
                            }
                        }
                        Ok(bytes)
                    })();
                    match result {
                        Ok(bytes) => {
                            definitions
                                .font_data
                                .insert(key.clone(), Arc::new(FontData::from_owned(bytes)));
                        }
                        Err(message) => {
                            self.diagnostics.push(TerminalFontDiagnostics {
                                source: source.clone(),
                                message,
                            });
                            continue;
                        }
                    }
                }
                chain.push(key);
            }
            chain.push("Hack".into());
            chain.extend(
                definitions.families[&FontFamily::Monospace]
                    .iter()
                    .filter(|s| *s != "Hack")
                    .cloned(),
            );
            let bold: Vec<_> = chain
                .iter()
                .map(|s| {
                    if s == "Hack" {
                        "Hack-Bold".into()
                    } else {
                        s.clone()
                    }
                })
                .collect();
            definitions.families.insert(config.family(false), chain);
            definitions.families.insert(config.family(true), bold);
        }
        ctx.set_fonts(definitions);
    }
}

pub fn cell_metrics(ctx: &Context, font: &FontId, line_height: f32) -> egui::Vec2 {
    let ppp = ctx.pixels_per_point();
    ctx.fonts_mut(|fonts| {
        egui::vec2(
            (fonts.glyph_width(font, char::from(77)) * ppp).ceil() / ppp,
            (fonts.row_height(font) * line_height * ppp).ceil() / ppp,
        )
    })
}

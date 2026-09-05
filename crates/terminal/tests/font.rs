use kervesh_terminal::{TerminalFontConfig, TerminalFontManager};
#[test]
fn configured_missing_font_keeps_complete_box_block_coverage() {
    let ctx = egui::Context::default();
    let config = TerminalFontConfig {
        primary: "/missing/font.ttf".into(),
        fallbacks: vec![],
    };
    let mut manager = TerminalFontManager::default();
    manager.register(&ctx, std::slice::from_ref(&config));
    let _ = ctx.run(Default::default(), |ctx| {
        let font = config.font_id(14.0, false);
        ctx.fonts_mut(|fonts| {
            let missing: Vec<_> = (0x2500..=0x259f)
                .filter_map(char::from_u32)
                .chain("ASCII éñ←→↑↓∑√≠≤≥\u{301}".chars())
                .filter(|c| !fonts.has_glyph(&font, *c))
                .collect();
            assert!(missing.is_empty(), "missing glyphs: {missing:?}");
        });
    });
    assert!(!manager.diagnostics().is_empty());
}

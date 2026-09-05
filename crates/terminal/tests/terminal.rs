use egui::{Key, Modifiers};
use kervesh_terminal::{Terminal, encode_key, encode_paste};
#[test]
fn ansi_cursor_and_alternate_screen_restore_primary_content() {
    let mut t = Terminal::new(80, 24, 1000);
    t.feed(b"hello\rX");
    assert!(t.text().starts_with("Xello"));
    t.feed(b"\x1b[?1049hother");
    assert!(t.text().contains("other"));
    t.feed(b"\x1b[?1049l");
    assert!(t.text().starts_with("Xello"));
}
#[test]
fn utf8_can_arrive_in_split_packets_and_resize_preserves_content() {
    let mut t = Terminal::new(80, 24, 1000);
    t.feed(&[0xc3]);
    t.feed(&[0xa9]);
    assert!(t.text().contains('é'));
    t.resize(100, 30);
    assert!(t.text().contains('é'));
}
#[test]
fn paste_neutralizes_embedded_bracketed_paste_terminators() {
    assert_eq!(encode_paste("ls\n", true), b"\x1b[200~ls\n\x1b[201~");
    assert_eq!(
        encode_paste("a\x1b[201~b", true),
        b"\x1b[200~a[201~b\x1b[201~"
    );
    assert_eq!(encode_paste("hello", false), b"hello");
}
#[test]
fn terminal_key_sequences_include_control_and_application_cursor() {
    assert_eq!(encode_key(Key::C, Modifiers::CTRL, false), Some(vec![3]));
    assert_eq!(
        encode_key(Key::ArrowUp, Modifiers::NONE, true),
        Some(b"\x1bOA".to_vec())
    );
    assert_eq!(
        encode_key(Key::ArrowUp, Modifiers::NONE, false),
        Some(b"\x1b[A".to_vec())
    );
    assert_eq!(
        encode_key(Key::Enter, Modifiers::NONE, false),
        Some(vec![13])
    );
}

#[test]
fn desktop_clipboard_events_preserve_terminal_control_shortcuts() {
    use kervesh_terminal::clipboard_control;
    assert_eq!(
        clipboard_control(&egui::Event::Copy, Modifiers::CTRL),
        Some(3)
    );
    assert_eq!(
        clipboard_control(&egui::Event::Cut, Modifiers::CTRL),
        Some(24)
    );
    assert_eq!(
        clipboard_control(
            &egui::Event::Paste("secret clipboard".into()),
            Modifiers::CTRL
        ),
        Some(22)
    );
    assert_eq!(
        clipboard_control(
            &egui::Event::Copy,
            Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::NONE
            }
        ),
        None
    );
}

#[test]
fn terminal_feeds_box_drawing_and_block_elements_without_corruption() {
    let mut t = Terminal::new(80, 24, 1000);
    let box_str =
        "┌──────┬──────┐\n│ left │ right│\n├──────┼──────┤\n│ test │ ok   │\n└──────┴──────┘\n";
    t.feed(box_str.as_bytes());
    let text = t.text();
    assert!(text.contains('┌') && text.contains('┬') && text.contains('┐'));
    assert!(text.contains('│') && text.contains('├') && text.contains('┤'));
    assert!(text.contains('└') && text.contains('┴') && text.contains('┘'));

    let mut t2 = Terminal::new(80, 24, 1000);
    let blocks_str = "█▓▒░ ─ │ ┌ ┐ └ ┘ ├ ┤ ┬ ┴ ┼ ← → ↑ ↓\n";
    t2.feed(blocks_str.as_bytes());
    let text2 = t2.text();
    assert!(
        text2.contains('█') && text2.contains('▓') && text2.contains('▒') && text2.contains('░')
    );
    assert!(
        text2.contains('←') && text2.contains('→') && text2.contains('↑') && text2.contains('↓')
    );
}

#[test]
fn custom_terminal_glyph_coverage() {
    use kervesh_terminal::is_custom_glyph;
    // Box drawing
    assert!(is_custom_glyph('┌'));
    assert!(is_custom_glyph('─'));
    assert!(is_custom_glyph('│'));
    assert!(is_custom_glyph('┼'));
    assert!(is_custom_glyph('╭'));
    assert!(is_custom_glyph('═'));
    assert!(is_custom_glyph('║'));
    assert!(is_custom_glyph('╔'));
    // Block elements
    assert!(is_custom_glyph('█'));
    assert!(is_custom_glyph('▓'));
    assert!(is_custom_glyph('▒'));
    assert!(is_custom_glyph('░'));
    assert!(is_custom_glyph('▀'));
    assert!(is_custom_glyph('▄'));
    assert!(is_custom_glyph('▌'));
    assert!(is_custom_glyph('▐'));
    // Arrows
    assert!(is_custom_glyph('←'));
    assert!(is_custom_glyph('→'));
    assert!(is_custom_glyph('↑'));
    assert!(is_custom_glyph('↓'));
    // Regular characters should not be custom glyphs
    assert!(!is_custom_glyph('A'));
    assert!(!is_custom_glyph('1'));
    assert!(!is_custom_glyph('$'));
}

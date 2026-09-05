use egui::{Event, Key, Modifiers};
use kervesh_terminal::{ClipboardIntent, ClipboardProfile, clipboard_intent, encode_paste};
#[test]
fn desktop_copy_is_smart_and_paste_is_desktop() {
    assert_eq!(
        clipboard_intent(
            &Event::Copy,
            Modifiers::CTRL,
            ClipboardProfile::Desktop,
            true,
            true
        ),
        Some(ClipboardIntent::Copy)
    );
    assert_eq!(
        clipboard_intent(
            &Event::Copy,
            Modifiers::CTRL,
            ClipboardProfile::Desktop,
            false,
            true
        ),
        Some(ClipboardIntent::Control(3))
    );
    assert_eq!(
        clipboard_intent(
            &Event::Paste("x".into()),
            Modifiers::CTRL,
            ClipboardProfile::Desktop,
            false,
            true
        ),
        Some(ClipboardIntent::Paste)
    );
}
#[test]
fn traditional_and_shift_shortcuts_and_literal_controls() {
    assert_eq!(
        clipboard_intent(
            &Event::Copy,
            Modifiers::CTRL,
            ClipboardProfile::Traditional,
            true,
            true
        ),
        Some(ClipboardIntent::Control(3))
    );
    assert_eq!(
        clipboard_intent(
            &Event::Paste("x".into()),
            Modifiers::CTRL,
            ClipboardProfile::Traditional,
            true,
            true
        ),
        Some(ClipboardIntent::Control(22))
    );
    let shift = Modifiers {
        ctrl: true,
        shift: true,
        ..Modifiers::NONE
    };
    assert_eq!(
        clipboard_intent(
            &Event::Copy,
            shift,
            ClipboardProfile::Traditional,
            true,
            true
        ),
        Some(ClipboardIntent::Copy)
    );
    let literal = Modifiers {
        ctrl: true,
        alt: true,
        ..Modifiers::NONE
    };
    let event = Event::Key {
        key: Key::F,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: literal,
    };
    assert_eq!(
        clipboard_intent(&event, literal, ClipboardProfile::Desktop, false, true),
        Some(ClipboardIntent::Control(6))
    );
}
#[test]
fn insert_and_key_shortcuts() {
    let shift_insert = Event::Key {
        key: Key::Insert,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::SHIFT,
    };
    assert_eq!(
        clipboard_intent(
            &shift_insert,
            Modifiers::SHIFT,
            ClipboardProfile::Desktop,
            false,
            false
        ),
        Some(ClipboardIntent::RequestPaste)
    );
    assert_eq!(
        clipboard_intent(
            &shift_insert,
            Modifiers::SHIFT,
            ClipboardProfile::Traditional,
            false,
            false
        ),
        Some(ClipboardIntent::RequestPaste)
    );

    let ctrl_insert = Event::Key {
        key: Key::Insert,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::CTRL,
    };
    assert_eq!(
        clipboard_intent(
            &ctrl_insert,
            Modifiers::CTRL,
            ClipboardProfile::Desktop,
            true,
            false
        ),
        Some(ClipboardIntent::Copy)
    );

    let ctrl_shift_c = Event::Key {
        key: Key::C,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers {
            ctrl: true,
            shift: true,
            ..Modifiers::NONE
        },
    };
    assert_eq!(
        clipboard_intent(
            &ctrl_shift_c,
            Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::NONE
            },
            ClipboardProfile::Traditional,
            true,
            false
        ),
        Some(ClipboardIntent::Copy)
    );

    let ctrl_shift_v = Event::Key {
        key: Key::V,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers {
            ctrl: true,
            shift: true,
            ..Modifiers::NONE
        },
    };
    assert_eq!(
        clipboard_intent(
            &ctrl_shift_v,
            Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::NONE
            },
            ClipboardProfile::Traditional,
            false,
            false
        ),
        Some(ClipboardIntent::RequestPaste)
    );
}

#[test]
fn paste_preserves_contents_and_normalizes_both_protocols() {
    assert_eq!(
        encode_paste("a\r\nb\rc\n", true),
        b"\x1b[200~a\nb\nc\n\x1b[201~"
    );
    assert_eq!(encode_paste("a\r\nb\rc\n", false), b"a\rb\rc\r");
}

#[test]
fn custom_glyph_recognizes_braille_powerline_box_and_blocks() {
    use kervesh_terminal::is_custom_glyph;
    // Braille patterns (anime art / charts)
    assert!(is_custom_glyph('⠀')); // U+2800 blank
    assert!(is_custom_glyph('⣿')); // U+28FF full
    assert!(is_custom_glyph('⢀')); // U+2880
    assert!(is_custom_glyph('⣠')); // U+28E0

    // Box drawing & blocks
    assert!(is_custom_glyph('┌'));
    assert!(is_custom_glyph('█'));
    assert!(is_custom_glyph('▀'));
    assert!(is_custom_glyph('▄'));
    assert!(is_custom_glyph('░'));

    // Powerline
    assert!(is_custom_glyph('\u{E0B0}'));
    assert!(is_custom_glyph('\u{E0B2}'));

    // Standard ASCII
    assert!(!is_custom_glyph('a'));
    assert!(!is_custom_glyph('1'));
    assert!(!is_custom_glyph('$'));
}

#[test]
fn clipboard_get_and_set_functions_do_not_panic() {
    kervesh_terminal::set_clipboard_text("test_clipboard_payload");
    kervesh_terminal::set_primary_text("test_primary_payload");
    let _ = kervesh_terminal::get_clipboard_text();
    let _ = kervesh_terminal::get_primary_text();
}

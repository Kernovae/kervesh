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
fn paste_preserves_contents_and_normalizes_both_protocols() {
    assert_eq!(
        encode_paste("a\r\nb\rc\n", true),
        b"\x1b[200~a\nb\nc\n\x1b[201~"
    );
    assert_eq!(encode_paste("a\r\nb\rc\n", false), b"a\rb\rc\r");
}

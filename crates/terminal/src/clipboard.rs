use crate::ClipboardProfile;
use egui::{Event, Key, Modifiers};

#[derive(Debug, PartialEq, Eq)]
pub enum ClipboardIntent {
    Copy,
    Paste,
    RequestPaste,
    Control(u8),
}

pub fn get_clipboard_text() -> Option<String> {
    arboard::Clipboard::new()
        .ok()
        .and_then(|mut cb| cb.get_text().ok())
        .filter(|s| !s.is_empty())
}

pub fn set_clipboard_text(text: &str) {
    if let Ok(mut cb) = arboard::Clipboard::new() {
        let _ = cb.set_text(text);
    }
}

/// Resolve both native clipboard events and raw keys without reading clipboard contents.
pub fn clipboard_intent(
    event: &Event,
    current: Modifiers,
    profile: ClipboardProfile,
    selected: bool,
    literal: bool,
) -> Option<ClipboardIntent> {
    let modifiers = if let Event::Key { modifiers, .. } = event {
        *modifiers
    } else {
        current
    };
    if literal && modifiers.ctrl && modifiers.alt {
        let byte = match event {
            Event::Copy => Some(3),
            Event::Cut => Some(24),
            Event::Paste(_) => Some(22),
            Event::Key {
                key, pressed: true, ..
            } => crate::encode_key(*key, Modifiers::CTRL, false)
                .filter(|b| b.len() == 1)
                .map(|b| b[0]),
            _ => None,
        };
        if let Some(byte) = byte {
            return Some(ClipboardIntent::Control(byte));
        }
    }

    match event {
        Event::Copy => {
            if modifiers.ctrl
                && !modifiers.shift
                && (profile == ClipboardProfile::Traditional || !selected)
            {
                Some(ClipboardIntent::Control(3))
            } else {
                Some(ClipboardIntent::Copy)
            }
        }
        Event::Cut if modifiers.ctrl && !modifiers.shift => Some(ClipboardIntent::Control(24)),
        Event::Paste(_) => {
            if modifiers.ctrl && !modifiers.shift && profile == ClipboardProfile::Traditional {
                Some(ClipboardIntent::Control(22))
            } else {
                Some(ClipboardIntent::Paste)
            }
        }
        Event::Key {
            key: Key::Insert,
            pressed: true,
            ..
        } => {
            if modifiers.shift {
                Some(ClipboardIntent::RequestPaste)
            } else if modifiers.ctrl {
                Some(ClipboardIntent::Copy)
            } else {
                None
            }
        }
        Event::Key {
            key: Key::C,
            pressed: true,
            ..
        } if modifiers.ctrl => {
            if !modifiers.shift && (profile == ClipboardProfile::Traditional || !selected) {
                Some(ClipboardIntent::Control(3))
            } else {
                Some(ClipboardIntent::Copy)
            }
        }
        Event::Key {
            key: Key::V,
            pressed: true,
            ..
        } if modifiers.ctrl => {
            if !modifiers.shift && profile == ClipboardProfile::Traditional {
                Some(ClipboardIntent::Control(22))
            } else {
                Some(ClipboardIntent::RequestPaste)
            }
        }
        Event::Key {
            key: Key::X,
            pressed: true,
            ..
        } if modifiers.ctrl && !modifiers.shift => Some(ClipboardIntent::Control(24)),
        _ => None,
    }
}

pub fn paste_lines(text: &str) -> usize {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    normalized.split('\n').count()
}

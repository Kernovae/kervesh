use crate::ClipboardProfile;
use egui::{Event, Key, Modifiers};

#[derive(Debug, PartialEq, Eq)]
pub enum ClipboardIntent {
    Copy,
    Paste,
    RequestPaste,
    Control(u8),
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
    let copy = match event {
        Event::Copy => true,
        Event::Key {
            key: Key::C,
            pressed: true,
            ..
        } if modifiers.ctrl => true,
        _ => false,
    };
    if copy {
        return Some(
            if modifiers.ctrl
                && !modifiers.shift
                && (profile == ClipboardProfile::Traditional || !selected)
            {
                ClipboardIntent::Control(3)
            } else {
                ClipboardIntent::Copy
            },
        );
    }
    match event {
        Event::Cut if modifiers.ctrl => Some(ClipboardIntent::Control(24)),
        Event::Paste(_)
            if modifiers.ctrl && !modifiers.shift && profile == ClipboardProfile::Traditional =>
        {
            Some(ClipboardIntent::Control(22))
        }
        Event::Paste(_) => Some(ClipboardIntent::Paste),
        Event::Key {
            key: Key::V,
            pressed: true,
            ..
        } if modifiers.ctrl => Some(
            if !modifiers.shift && profile == ClipboardProfile::Traditional {
                ClipboardIntent::Control(22)
            } else {
                ClipboardIntent::RequestPaste
            },
        ),
        _ => None,
    }
}

pub fn paste_lines(text: &str) -> usize {
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    normalized.split('\n').count()
}

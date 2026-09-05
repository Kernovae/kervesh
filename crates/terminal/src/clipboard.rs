use crate::ClipboardProfile;
use egui::{Event, Key, Modifiers};

#[derive(Debug, PartialEq, Eq)]
pub enum ClipboardIntent {
    Copy,
    Paste,
    RequestPaste,
    Control(u8),
}

use std::sync::{Mutex, OnceLock};

#[cfg(target_os = "linux")]
use arboard::{GetExtLinux, LinuxClipboardKind, SetExtLinux};

static CLIPBOARD: OnceLock<Mutex<Option<arboard::Clipboard>>> = OnceLock::new();

fn with_clipboard<R>(f: impl FnOnce(&mut arboard::Clipboard) -> R) -> Option<R> {
    let mutex = CLIPBOARD.get_or_init(|| Mutex::new(arboard::Clipboard::new().ok()));
    let mut guard = mutex.lock().ok()?;
    if guard.is_none() {
        *guard = arboard::Clipboard::new().ok();
    }
    guard.as_mut().map(f)
}

pub fn get_clipboard_text() -> Option<String> {
    with_clipboard(|cb| {
        #[cfg(target_os = "linux")]
        {
            cb.get()
                .clipboard(LinuxClipboardKind::Clipboard)
                .text()
                .or_else(|_| cb.get_text())
                .ok()
        }
        #[cfg(not(target_os = "linux"))]
        {
            cb.get_text().ok()
        }
    })
    .flatten()
    .filter(|s| !s.is_empty())
}

pub fn get_primary_text() -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        with_clipboard(|cb| cb.get().clipboard(LinuxClipboardKind::Primary).text().ok())
            .flatten()
            .filter(|s| !s.is_empty())
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

pub fn set_clipboard_text(text: &str) {
    if text.is_empty() {
        return;
    }
    let _ = with_clipboard(|cb| {
        #[cfg(target_os = "linux")]
        {
            let _ = cb
                .set()
                .clipboard(LinuxClipboardKind::Clipboard)
                .text(text.to_owned());
            let _ = cb
                .set()
                .clipboard(LinuxClipboardKind::Primary)
                .text(text.to_owned());
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = cb.set_text(text.to_owned());
        }
    });
}

pub fn set_primary_text(text: &str) {
    if text.is_empty() {
        return;
    }
    #[cfg(target_os = "linux")]
    {
        let _ = with_clipboard(|cb| {
            let _ = cb
                .set()
                .clipboard(LinuxClipboardKind::Primary)
                .text(text.to_owned());
        });
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

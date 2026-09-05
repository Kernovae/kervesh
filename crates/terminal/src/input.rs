use egui::{Key, Modifiers};

pub fn encode_paste(text: &str, bracketed: bool) -> Vec<u8> {
    if bracketed {
        format!("\x1b[200~{}\x1b[201~", text.replace('\x1b', "")).into_bytes()
    } else {
        text.replace("\r\n", "\r").replace('\n', "\r").into_bytes()
    }
}
pub fn encode_key(key: Key, modifiers: Modifiers, application: bool) -> Option<Vec<u8>> {
    if modifiers.ctrl {
        let control = match key {
            Key::A => 1,
            Key::B => 2,
            Key::C => 3,
            Key::D => 4,
            Key::E => 5,
            Key::F => 6,
            Key::G => 7,
            Key::H => 8,
            Key::I => 9,
            Key::J => 10,
            Key::K => 11,
            Key::L => 12,
            Key::M => 13,
            Key::N => 14,
            Key::O => 15,
            Key::P => 16,
            Key::Q => 17,
            Key::R => 18,
            Key::S => 19,
            Key::T => 20,
            Key::U => 21,
            Key::V => 22,
            Key::W => 23,
            Key::X => 24,
            Key::Y => 25,
            Key::Z => 26,
            Key::Space => 0,
            Key::OpenBracket => 27,
            Key::Backslash => 28,
            Key::CloseBracket => 29,
            _ => 255,
        };
        if control != 255 {
            return Some(if modifiers.alt {
                vec![27, control]
            } else {
                vec![control]
            });
        }
    }
    let arrow = match key {
        Key::ArrowUp => Some('A'),
        Key::ArrowDown => Some('B'),
        Key::ArrowRight => Some('C'),
        Key::ArrowLeft => Some('D'),
        Key::Home => Some('H'),
        Key::End => Some('F'),
        _ => None,
    };
    if let Some(code) = arrow {
        let modifier = 1
            + u8::from(modifiers.shift)
            + 2 * u8::from(modifiers.alt)
            + 4 * u8::from(modifiers.ctrl);
        return Some(if modifier > 1 {
            format!("\x1b[1;{modifier}{code}").into_bytes()
        } else {
            format!("\x1b{}{code}", if application { "O" } else { "[" }).into_bytes()
        });
    }
    let s = match key {
        Key::Enter => "\r",
        Key::Backspace => "\x7f",
        Key::Tab if modifiers.shift => "\x1b[Z",
        Key::Tab => "\t",
        Key::Escape => "\x1b",
        Key::Insert => "\x1b[2~",
        Key::Delete => "\x1b[3~",
        Key::PageUp => "\x1b[5~",
        Key::PageDown => "\x1b[6~",
        Key::F1 => "\x1bOP",
        Key::F2 => "\x1bOQ",
        Key::F3 => "\x1bOR",
        Key::F4 => "\x1bOS",
        Key::F5 => "\x1b[15~",
        Key::F6 => "\x1b[17~",
        Key::F7 => "\x1b[18~",
        Key::F8 => "\x1b[19~",
        Key::F9 => "\x1b[20~",
        Key::F10 => "\x1b[21~",
        Key::F11 => "\x1b[23~",
        Key::F12 => "\x1b[24~",
        _ => return None,
    };
    let mut bytes = Vec::new();
    if modifiers.alt {
        bytes.push(27);
    }
    bytes.extend_from_slice(s.as_bytes());
    Some(bytes)
}

/// Winit consumes these keys before emitting a Key event. Preserve terminal control bytes.
pub fn clipboard_control(event: &egui::Event, modifiers: Modifiers) -> Option<u8> {
    if !modifiers.ctrl || modifiers.shift {
        return None;
    }
    match event {
        egui::Event::Copy => Some(3),
        egui::Event::Cut => Some(24),
        egui::Event::Paste(_) => Some(22),
        _ => None,
    }
}

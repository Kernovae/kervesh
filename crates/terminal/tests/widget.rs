use egui::{Event, Modifiers, Pos2, Rect};
use kervesh_terminal::{
    ClipboardProfile, Terminal, TerminalAction, TerminalFontConfig, TerminalFontManager,
    TerminalProfile,
};
struct Harness {
    ctx: egui::Context,
    terminal: Terminal,
    time: f64,
}
impl Harness {
    fn new() -> Self {
        let ctx = egui::Context::default();
        let profile = TerminalProfile::default();
        TerminalFontManager::default().register(&ctx, &[TerminalFontConfig::from(&profile)]);
        let mut h = Self {
            ctx,
            terminal: Terminal::with_profile(80, 24, profile),
            time: 0.0,
        };
        h.frame(vec![], Modifiers::NONE);
        h.click();
        h
    }
    fn frame(
        &mut self,
        events: Vec<Event>,
        modifiers: Modifiers,
    ) -> (TerminalAction, egui::FullOutput) {
        self.time += 0.1;
        let mut action = TerminalAction::default();
        let input = egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, egui::vec2(640.0, 480.0))),
            time: Some(self.time),
            modifiers,
            events,
            ..Default::default()
        };
        let output = self.ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                action = self.terminal.show(ui, false);
            });
        });
        (action, output)
    }
    fn click(&mut self) {
        let pos = Pos2::new(30.0, 30.0);
        self.frame(
            vec![
                Event::PointerMoved(pos),
                Event::PointerButton {
                    pos,
                    button: egui::PointerButton::Primary,
                    pressed: true,
                    modifiers: Modifiers::NONE,
                },
            ],
            Modifiers::NONE,
        );
        self.frame(
            vec![Event::PointerButton {
                pos,
                button: egui::PointerButton::Primary,
                pressed: false,
                modifiers: Modifiers::NONE,
            }],
            Modifiers::NONE,
        );
    }
}
#[test]
fn focused_native_clipboard_events_copy_interrupt_and_paste_once() {
    let mut h = Harness::new();
    h.terminal.feed(b"hello world");
    h.terminal.select_range((0, 0), (0, 4));
    let (action, output) = h.frame(vec![Event::Copy], Modifiers::CTRL);
    assert!(action.input.is_empty());
    assert!(
        output
            .platform_output
            .commands
            .iter()
            .any(|c| matches!(c,egui::OutputCommand::CopyText(text) if text == "hello"))
    );
    h.terminal.clear_selection();
    assert_eq!(h.frame(vec![Event::Copy], Modifiers::CTRL).0.input, vec![3]);
    assert_eq!(
        h.frame(vec![Event::Paste("pwd".into())], Modifiers::CTRL)
            .0
            .input,
        b"pwd"
    );
    let mut profile = h.terminal.profile().clone();
    profile.clipboard_profile = ClipboardProfile::Traditional;
    h.terminal.set_profile(profile);
    assert_eq!(
        h.frame(vec![Event::Paste("not sent".into())], Modifiers::CTRL)
            .0
            .input,
        vec![22]
    );
}
#[test]
fn multiline_preview_never_writes_before_confirmation_and_escape_cancels() {
    let mut h = Harness::new();
    h.terminal.feed(b"\x1b[?2004h");
    let (action, _) = h.frame(vec![Event::Paste("echo a\necho b".into())], Modifiers::CTRL);
    assert!(action.input.is_empty());
    let escape = Event::Key {
        key: egui::Key::Escape,
        physical_key: None,
        pressed: true,
        repeat: false,
        modifiers: Modifiers::NONE,
    };
    assert!(h.frame(vec![escape], Modifiers::NONE).0.input.is_empty());
    assert_eq!(
        h.frame(vec![Event::Paste("ok".into())], Modifiers::CTRL)
            .0
            .input,
        b"\x1b[200~ok\x1b[201~"
    );
}
#[test]
fn render_is_independent_of_app_theme_and_idle_does_not_request_busy_repaints() {
    let mut h = Harness::new();
    h.terminal
        .feed("\x1b[31mRED\x1b[0m ┌─┐ █▓▒░ ←→ e\u{301}界".as_bytes());
    let (_, dark) = h.frame(vec![], Modifiers::NONE);
    h.ctx.set_visuals(egui::Visuals::light());
    let (_, light) = h.frame(vec![], Modifiers::NONE);
    let text_colors = |o: egui::FullOutput| {
        o.shapes
            .into_iter()
            .filter_map(|s| {
                if let egui::Shape::Text(t) = s.shape {
                    Some((t.galley.text().to_owned(), t.fallback_color))
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(text_colors(dark), text_colors(light));
    for _ in 0..4 {
        h.frame(vec![], Modifiers::NONE);
    }
    let (_, output) = h.frame(vec![], Modifiers::NONE);
    assert!(
        output.viewport_output[&egui::ViewportId::ROOT].repaint_delay
            > std::time::Duration::from_secs(1)
    );
}

#[test]
fn escape_tab_and_arrows_remain_terminal_keys() {
    let mut h = Harness::new();
    h.frame(vec![], Modifiers::NONE);
    for (key, expected) in [
        (egui::Key::Escape, vec![27]),
        (egui::Key::Tab, vec![9]),
        (egui::Key::ArrowUp, b"\x1b[A".to_vec()),
    ] {
        let event = Event::Key {
            key,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: Modifiers::NONE,
        };
        assert_eq!(h.frame(vec![event], Modifiers::NONE).0.input, expected);
    }
}

#[test]
fn ctrl_click_link_does_not_send_partial_remote_mouse_press() {
    let mut h = Harness::new();
    h.terminal
        .feed(b"https://example.com\x1b[?1000h\x1b[?1006h");
    let pos = Pos2::new(20.0, 20.0);
    let press = Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: true,
        modifiers: Modifiers::CTRL,
    };
    assert!(
        h.frame(vec![Event::PointerMoved(pos), press], Modifiers::CTRL)
            .0
            .input
            .is_empty()
    );
    let release = Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed: false,
        modifiers: Modifiers::CTRL,
    };
    let (action, output) = h.frame(vec![release], Modifiers::CTRL);
    assert!(action.input.is_empty());
    assert!(output.platform_output.commands.iter().any(
        |c| matches!(c,egui::OutputCommand::OpenUrl(url) if url.url == "https://example.com")
    ));
}

//! Native, offline terminal renderer fixture. No SSH connection or saved application data.
use kervesh_terminal::{
    Terminal, TerminalFontConfig, TerminalFontManager, TerminalPalette, TerminalProfile,
};
struct Fixture {
    terminals: [Terminal; 2],
}
impl eframe::App for Fixture {
    fn update(&mut self, ctx: &egui::Context, _: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Terminal Foundation — offline renderer fixture");
            ui.columns(2, |columns| {
                for (index, terminal) in self.terminals.iter_mut().enumerate() {
                    columns[index].label(if index == 0 {
                        "Kervesh Dark"
                    } else {
                        "Kervesh Light"
                    });
                    terminal.show(&mut columns[index], false);
                }
            });
        });
    }
}
fn main() -> eframe::Result {
    let dark = TerminalProfile::default();
    let light = TerminalProfile {
        palette: TerminalPalette::light(),
        ..dark.clone()
    };
    let configs = [
        TerminalFontConfig::from(&dark),
        TerminalFontConfig::from(&light),
    ];
    let mut terminals = [
        Terminal::with_profile(65, 30, dark),
        Terminal::with_profile(65, 30, light),
    ];
    let mut fixture = String::from(
        "ASCII Latin-1: Hello é ñ ø ß\r\n\x1b[31mRED\x1b[0m \x1b[32mGREEN\x1b[0m \x1b[34mBLUE\x1b[0m\r\n┌──────┬──────┐\r\n│ left │ right│\r\n├──────┼──────┤\r\n│ test │ ok   │\r\n└──────┴──────┘\r\n█▓▒░ ─ │ ┌ ┐ └ ┘ ├ ┤ ┬ ┴ ┼ ← → ↑ ↓\r\n\x1b[38;2;255;100;50mTRUECOLOR\x1b[0m\r\n\x1b[1mBOLD\x1b[0m \x1b[2mDIM\x1b[0m \x1b[7mINVERSE\x1b[0m \x1b[4mUNDERLINE\x1b[0m\r\nCombining: e\u{301} a\u{308} | Math: ∑ √ ≠ ≤ ≥\r\n",
    );
    for start in (0x2500..=0x2590).step_by(16) {
        fixture.push_str(&format!("{start:04X} "));
        for code in start..start + 16 {
            fixture.push(char::from_u32(code).unwrap());
        }
        fixture.push_str("\r\n");
    }
    fixture.push_str(
        "\x1b]8;;https://example.com\x1b\\OSC 8 link\x1b]8;;\x1b\\  https://example.org\r\n",
    );
    for terminal in &mut terminals {
        terminal.feed(fixture.as_bytes());
    }
    eframe::run_native(
        "Terminal Foundation fixture",
        eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default().with_inner_size([1200.0, 820.0]),
            ..Default::default()
        },
        Box::new(move |cc| {
            TerminalFontManager::default().register(&cc.egui_ctx, &configs);
            Ok(Box::new(Fixture { terminals }))
        }),
    )
}

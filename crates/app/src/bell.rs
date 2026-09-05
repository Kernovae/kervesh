/// Desktop audio runs on the existing blocking pool, never the terminal render thread.
pub(crate) fn play() {
    #[cfg(windows)]
    {
        #[link(name = "user32")]
        unsafe extern "system" {
            fn MessageBeep(kind: u32) -> i32;
        }
        // MessageBeep has no pointer arguments and does not retain application state.
        unsafe {
            MessageBeep(0xFFFF_FFFF);
        }
    }
    #[cfg(target_os = "linux")]
    {
        use std::{
            io::Write,
            process::{Command, Stdio},
        };
        let Ok(mut child) = Command::new("paplay")
            .args(["--raw", "--format=s16le", "--rate=22050", "--channels=1"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
        else {
            return;
        };
        if let Some(mut input) = child.stdin.take() {
            let samples: Vec<u8> = (0..2205)
                .flat_map(|i| {
                    let envelope = (1.0 - i as f32 / 2205.0).max(0.0);
                    let sample = ((i as f32 * 880.0 * std::f32::consts::TAU / 22050.0).sin()
                        * 4000.0
                        * envelope) as i16;
                    sample.to_le_bytes()
                })
                .collect();
            let _ = input.write_all(&samples);
        }
        let _ = child.wait();
    }
}

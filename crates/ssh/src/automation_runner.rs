use anyhow::Result;
use kervesh_core::{AutomationMacro, MacroStep};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{RwLock, mpsc};

pub async fn execute_macro(
    mac: AutomationMacro,
    input_tx: mpsc::Sender<Vec<u8>>,
    output_tracker: Arc<RwLock<String>>,
    events: crate::EventSink,
) -> Result<()> {
    let total = mac.steps.len();
    for (idx, step) in mac.steps.into_iter().enumerate() {
        events
            .send(crate::Event::MacroStatus {
                id: mac.id.clone(),
                step_index: idx,
                total_steps: total,
                done: false,
                error: None,
            })
            .await;

        match step {
            MacroStep::SendText {
                text,
                append_newline,
            } => {
                let mut data = text.into_bytes();
                if append_newline && !data.ends_with(b"\n") {
                    data.push(b'\n');
                }
                let _ = input_tx.send(data).await;
            }
            MacroStep::DelayMs(ms) => {
                tokio::time::sleep(Duration::from_millis(ms)).await;
            }
            MacroStep::ExpectPrompt(prompt) => {
                let start = std::time::Instant::now();
                let timeout = Duration::from_secs(10);
                let mut matched = false;
                while start.elapsed() < timeout {
                    {
                        let out = output_tracker.read().await;
                        if out.contains(&prompt) {
                            matched = true;
                            break;
                        }
                    }
                    tokio::time::sleep(Duration::from_millis(50)).await;
                }
                if !matched {
                    events
                        .send(crate::Event::MacroStatus {
                            id: mac.id.clone(),
                            step_index: idx,
                            total_steps: total,
                            done: true,
                            error: Some(format!("Timed out waiting for prompt: {prompt}")),
                        })
                        .await;
                    return Ok(());
                }
            }
        }
    }

    events
        .send(crate::Event::MacroStatus {
            id: mac.id,
            step_index: total,
            total_steps: total,
            done: true,
            error: None,
        })
        .await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_macro_execution_flow() {
        let (input_tx, mut input_rx) = mpsc::channel(16);
        let (event_tx, mut event_rx) = mpsc::channel(16);
        let sink = crate::EventSink::new(event_tx, Arc::new(|| {}));
        let tracker = Arc::new(RwLock::new(String::from("root@host:~# ")));

        let mut mac = AutomationMacro::new("TestMacro", "Test execution");
        mac.steps.push(MacroStep::SendText {
            text: "ls -la".into(),
            append_newline: true,
        });
        mac.steps.push(MacroStep::ExpectPrompt("#".into()));

        let handle = tokio::spawn(async move {
            execute_macro(mac, input_tx, tracker, sink).await.unwrap();
        });

        // Verify sent text
        let bytes = input_rx.recv().await.unwrap();
        assert_eq!(bytes, b"ls -la\n");

        handle.await.unwrap();

        // Verify macro status events received
        let mut status_count = 0;
        while let Ok(event) = event_rx.try_recv() {
            if let crate::Event::MacroStatus { .. } = event {
                status_count += 1;
            }
        }
        assert!(status_count >= 2);
    }
}

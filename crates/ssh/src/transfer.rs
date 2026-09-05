use crate::{Direction, Event, EventSink, TransferRequest, TransferState};
use anyhow::{Context, Result, bail, ensure};
use russh_sftp::{client::SftpSession, protocol::OpenFlags};
use std::time::Instant;
use tokio::{
    fs,
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
};
use tokio_util::sync::CancellationToken;

/// Fixed-size streaming buffer keeps memory independent of the file size.
pub async fn copy_stream<R, W, F>(
    reader: &mut R,
    writer: &mut W,
    cancel: &CancellationToken,
    mut progress: F,
) -> Result<u64>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
    F: FnMut(u64),
{
    let mut buffer = vec![0u8; 64 * 1024];
    let mut done = 0;
    loop {
        if cancel.is_cancelled() {
            bail!("Transfer cancelled");
        }
        let count = tokio::select! {biased; _=cancel.cancelled()=>bail!("Transfer cancelled"),result=reader.read(&mut buffer)=>result?};
        if count == 0 {
            break;
        }
        tokio::select! {biased; _=cancel.cancelled()=>bail!("Transfer cancelled"),result=writer.write_all(&buffer[..count])=>result?};
        done += count as u64;
        progress(done);
    }
    tokio::select! {biased; _=cancel.cancelled()=>bail!("Transfer cancelled"),result=writer.flush()=>result?};
    Ok(done)
}
pub async fn transfer(
    sftp: &SftpSession,
    request: &TransferRequest,
    events: &EventSink,
) -> Result<()> {
    ensure!(!request.cancel.is_cancelled(), "Transfer cancelled");
    let start = Instant::now();
    let mut last = Instant::now();
    let total = match request.direction {
        Direction::Upload => fs::metadata(&request.local).await?.len(),
        Direction::Download => sftp.metadata(&request.remote).await?.size.unwrap_or(0),
    };
    let progress = |done| {
        if last.elapsed().as_millis() >= 100 || done == total {
            events.progress(Event::Transfer {
                id: request.id,
                done,
                total,
                speed: done as f64 / start.elapsed().as_secs_f64().max(0.001),
                state: TransferState::Running,
            });
            last = Instant::now();
        }
    };
    events
        .send(Event::Transfer {
            id: request.id,
            done: 0,
            total,
            speed: 0.0,
            state: TransferState::Running,
        })
        .await;
    match request.direction {
        Direction::Upload => {
            let meta = fs::metadata(&request.local).await?;
            ensure!(meta.is_file(), "Select a regular file for upload");
            let existing = sftp.try_exists(&request.remote).await?;
            ensure!(
                !existing || request.overwrite,
                "Destination exists; confirm overwrite first"
            );
            let temp = format!("{}.kervesh-{}.part", request.remote, request.id);
            let mut source = fs::File::open(&request.local).await?;
            let mut target = sftp
                .open_with_flags(
                    &temp,
                    OpenFlags::CREATE | OpenFlags::EXCLUDE | OpenFlags::WRITE,
                )
                .await?;
            let copied = copy_stream(&mut source, &mut target, &request.cancel, progress).await;
            let closed = target.close().await;
            if let Err(e) = copied.and_then(|n| closed.map(|_| n).map_err(Into::into)) {
                let _ = sftp.remove_file(&temp).await;
                return Err(e);
            }
            let result = if existing {
                let backup = format!("{}.kervesh-{}.backup", request.remote, request.id);
                ensure!(
                    !sftp.try_exists(&backup).await?,
                    "Backup already exists: {backup}"
                );
                sftp.rename(&request.remote, &backup).await?;
                match sftp.rename(&temp, &request.remote).await {
                    Ok(()) => sftp
                        .remove_file(&backup)
                        .await
                        .context("Upload saved; backup cleanup failed"),
                    Err(e) => {
                        let restored = sftp.rename(&backup, &request.remote).await;
                        Err(anyhow::anyhow!(
                            "Could not replace destination: {e}; restore result: {restored:?}; backup: {backup}"
                        ))
                    }
                }
            } else {
                sftp.rename(&temp, &request.remote)
                    .await
                    .map_err(Into::into)
            };
            if result.is_err() {
                let _ = sftp.remove_file(&temp).await;
            }
            result?;
        }
        Direction::Download => {
            ensure!(
                sftp.metadata(&request.remote).await?.is_regular(),
                "Select a regular file for download"
            );
            let parent = request.local.parent().context("No destination directory")?;
            let temp = parent.join(format!(".kervesh-{}.part", request.id));
            let mut source = sftp.open(&request.remote).await?;
            let mut target = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&temp)
                .await?;
            let copied = copy_stream(&mut source, &mut target, &request.cancel, progress).await;
            let synced = target.sync_all().await;
            drop(target);
            let _ = source.close().await;
            if let Err(e) = copied.and_then(|n| synced.map(|_| n).map_err(Into::into)) {
                let _ = fs::remove_file(&temp).await;
                return Err(e);
            }
            let result = if request.overwrite {
                fs::rename(&temp, &request.local).await
            } else {
                fs::hard_link(&temp, &request.local).await
            };
            let _ = fs::remove_file(&temp).await;
            result.context("Could not commit download; destination may exist")?;
        }
    }
    events
        .send(Event::Transfer {
            id: request.id,
            done: total,
            total,
            speed: total as f64 / start.elapsed().as_secs_f64().max(0.001),
            state: TransferState::Complete,
        })
        .await;
    Ok(())
}

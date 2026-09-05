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

    match request.direction {
        Direction::Upload => {
            let meta = fs::metadata(&request.local).await?;
            if meta.is_dir() {
                let total = compute_local_dir_size(&request.local).await.unwrap_or(0);
                events
                    .send(Event::Transfer {
                        id: request.id,
                        done: 0,
                        total,
                        speed: 0.0,
                        state: TransferState::Running,
                    })
                    .await;
                let mut transferred = 0u64;
                upload_dir_recursive(
                    sftp,
                    &request.local,
                    &request.remote,
                    &request.cancel,
                    &mut transferred,
                    total,
                    start,
                    &mut last,
                    events,
                    request.id,
                )
                .await?;
                events
                    .send(Event::Transfer {
                        id: request.id,
                        done: total,
                        total,
                        speed: total as f64 / start.elapsed().as_secs_f64().max(0.001),
                        state: TransferState::Complete,
                    })
                    .await;
                return Ok(());
            }

            ensure!(
                meta.is_file(),
                "Select a regular file or directory for upload"
            );
            let total = meta.len();
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

            events
                .send(Event::Transfer {
                    id: request.id,
                    done: total,
                    total,
                    speed: total as f64 / start.elapsed().as_secs_f64().max(0.001),
                    state: TransferState::Complete,
                })
                .await;
        }
        Direction::Download => {
            let remote_meta = sftp.metadata(&request.remote).await?;
            if remote_meta.is_dir() {
                let total = compute_remote_dir_size(sftp, &request.remote)
                    .await
                    .unwrap_or(0);
                events
                    .send(Event::Transfer {
                        id: request.id,
                        done: 0,
                        total,
                        speed: 0.0,
                        state: TransferState::Running,
                    })
                    .await;
                let mut transferred = 0u64;
                download_dir_recursive(
                    sftp,
                    &request.remote,
                    &request.local,
                    &request.cancel,
                    &mut transferred,
                    total,
                    start,
                    &mut last,
                    events,
                    request.id,
                )
                .await?;
                events
                    .send(Event::Transfer {
                        id: request.id,
                        done: total,
                        total,
                        speed: total as f64 / start.elapsed().as_secs_f64().max(0.001),
                        state: TransferState::Complete,
                    })
                    .await;
                return Ok(());
            }

            ensure!(
                remote_meta.is_regular(),
                "Select a regular file or directory for download"
            );
            let total = remote_meta.size.unwrap_or(0);
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

            events
                .send(Event::Transfer {
                    id: request.id,
                    done: total,
                    total,
                    speed: total as f64 / start.elapsed().as_secs_f64().max(0.001),
                    state: TransferState::Complete,
                })
                .await;
        }
    }
    Ok(())
}

async fn compute_local_dir_size(path: &std::path::Path) -> Result<u64> {
    let mut total = 0;
    let mut read_dir = fs::read_dir(path).await?;
    while let Some(entry) = read_dir.next_entry().await? {
        let meta = entry.metadata().await?;
        if meta.is_file() {
            total += meta.len();
        } else if meta.is_dir() {
            total += Box::pin(compute_local_dir_size(&entry.path())).await?;
        }
    }
    Ok(total)
}

async fn compute_remote_dir_size(sftp: &SftpSession, path: &str) -> Result<u64> {
    let mut total = 0;
    let entries = sftp.read_dir(path).await?;
    for entry in entries {
        let file_name = entry.file_name();
        if file_name == "." || file_name == ".." {
            continue;
        }
        let subpath = if path.ends_with('/') {
            format!("{path}{file_name}")
        } else {
            format!("{path}/{file_name}")
        };
        let meta = entry.metadata();
        if meta.is_dir() {
            total += Box::pin(compute_remote_dir_size(sftp, &subpath)).await?;
        } else {
            total += meta.size.unwrap_or(0);
        }
    }
    Ok(total)
}

#[allow(clippy::too_many_arguments)]
async fn upload_dir_recursive(
    sftp: &SftpSession,
    local: &std::path::Path,
    remote: &str,
    cancel: &CancellationToken,
    transferred: &mut u64,
    total: u64,
    start: Instant,
    last: &mut Instant,
    events: &EventSink,
    request_id: u64,
) -> Result<()> {
    if !sftp.try_exists(remote).await? {
        let _ = sftp.create_dir(remote).await;
    }
    let mut read_dir = fs::read_dir(local).await?;
    while let Some(entry) = read_dir.next_entry().await? {
        if cancel.is_cancelled() {
            bail!("Transfer cancelled");
        }
        let name = entry.file_name();
        let name_str = name.to_string_lossy();
        let remote_child = if remote.ends_with('/') {
            format!("{remote}{name_str}")
        } else {
            format!("{remote}/{name_str}")
        };
        let meta = entry.metadata().await?;
        if meta.is_dir() {
            Box::pin(upload_dir_recursive(
                sftp,
                &entry.path(),
                &remote_child,
                cancel,
                transferred,
                total,
                start,
                last,
                events,
                request_id,
            ))
            .await?;
        } else if meta.is_file() {
            let mut source = fs::File::open(entry.path()).await?;
            let mut target = sftp
                .open_with_flags(
                    &remote_child,
                    OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
                )
                .await?;
            let mut buffer = vec![0u8; 64 * 1024];
            loop {
                if cancel.is_cancelled() {
                    let _ = target.close().await;
                    bail!("Transfer cancelled");
                }
                let count = tokio::select! {
                    biased;
                    _ = cancel.cancelled() => bail!("Transfer cancelled"),
                    res = source.read(&mut buffer) => res?
                };
                if count == 0 {
                    break;
                }
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => bail!("Transfer cancelled"),
                    res = target.write_all(&buffer[..count]) => res?
                };
                *transferred += count as u64;
                if last.elapsed().as_millis() >= 100 || *transferred == total {
                    events.progress(Event::Transfer {
                        id: request_id,
                        done: *transferred,
                        total,
                        speed: *transferred as f64 / start.elapsed().as_secs_f64().max(0.001),
                        state: TransferState::Running,
                    });
                    *last = Instant::now();
                }
            }
            target.flush().await?;
            target.close().await?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn download_dir_recursive(
    sftp: &SftpSession,
    remote: &str,
    local: &std::path::Path,
    cancel: &CancellationToken,
    transferred: &mut u64,
    total: u64,
    start: Instant,
    last: &mut Instant,
    events: &EventSink,
    request_id: u64,
) -> Result<()> {
    fs::create_dir_all(local).await?;
    let entries = sftp.read_dir(remote).await?;
    for entry in entries {
        if cancel.is_cancelled() {
            bail!("Transfer cancelled");
        }
        let file_name = entry.file_name();
        if file_name == "." || file_name == ".." {
            continue;
        }
        let remote_child = if remote.ends_with('/') {
            format!("{remote}{file_name}")
        } else {
            format!("{remote}/{file_name}")
        };
        let local_child = local.join(file_name);
        let meta = entry.metadata();
        if meta.is_dir() {
            Box::pin(download_dir_recursive(
                sftp,
                &remote_child,
                &local_child,
                cancel,
                transferred,
                total,
                start,
                last,
                events,
                request_id,
            ))
            .await?;
        } else {
            let mut source = sftp.open(&remote_child).await?;
            let mut target = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(true)
                .open(&local_child)
                .await?;
            let mut buffer = vec![0u8; 64 * 1024];
            loop {
                if cancel.is_cancelled() {
                    drop(target);
                    let _ = source.close().await;
                    bail!("Transfer cancelled");
                }
                let count = tokio::select! {
                    biased;
                    _ = cancel.cancelled() => bail!("Transfer cancelled"),
                    res = source.read(&mut buffer) => res?
                };
                if count == 0 {
                    break;
                }
                tokio::select! {
                    biased;
                    _ = cancel.cancelled() => bail!("Transfer cancelled"),
                    res = target.write_all(&buffer[..count]) => res?
                };
                *transferred += count as u64;
                if last.elapsed().as_millis() >= 100 || *transferred == total {
                    events.progress(Event::Transfer {
                        id: request_id,
                        done: *transferred,
                        total,
                        speed: *transferred as f64 / start.elapsed().as_secs_f64().max(0.001),
                        state: TransferState::Running,
                    });
                    *last = Instant::now();
                }
            }
            target.sync_all().await?;
            drop(target);
            let _ = source.close().await;
        }
    }
    Ok(())
}

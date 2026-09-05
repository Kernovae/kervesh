use anyhow::{Result, bail};
use kervesh_core::{FileMetadataEntry, SyncActionKind, SyncPlan};
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::OpenFlags;
use std::path::Path;
use std::sync::Arc;
use std::time::{Instant, UNIX_EPOCH};
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_util::sync::CancellationToken;

pub async fn walk_local_tree(base: &Path) -> Result<Vec<FileMetadataEntry>> {
    let mut entries = Vec::new();
    let mut stack = vec![base.to_path_buf()];

    while let Some(current) = stack.pop() {
        let mut read_dir = match fs::read_dir(&current).await {
            Ok(rd) => rd,
            Err(_) => continue,
        };

        while let Ok(Some(entry)) = read_dir.next_entry().await {
            let path = entry.path();
            let meta = match entry.metadata().await {
                Ok(m) => m,
                Err(_) => continue,
            };

            let rel = match path.strip_prefix(base) {
                Ok(p) => p.to_string_lossy().replace('\\', "/"),
                Err(_) => continue,
            };

            let mtime = meta
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);

            if meta.is_dir() {
                entries.push(FileMetadataEntry {
                    rel_path: rel,
                    size: 0,
                    mtime,
                    is_dir: true,
                });
                stack.push(path);
            } else if meta.is_file() {
                entries.push(FileMetadataEntry {
                    rel_path: rel,
                    size: meta.len(),
                    mtime,
                    is_dir: false,
                });
            }
        }
    }

    entries.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(entries)
}

pub async fn walk_remote_tree(sftp: &SftpSession, base: &str) -> Result<Vec<FileMetadataEntry>> {
    let mut entries = Vec::new();
    let base_canonical = sftp
        .canonicalize(base)
        .await
        .unwrap_or_else(|_| base.to_string());
    let mut stack = vec![base_canonical.clone()];

    while let Some(current) = stack.pop() {
        let dir_entries = match sftp.read_dir(&current).await {
            Ok(de) => de,
            Err(_) => continue,
        };

        for entry in dir_entries {
            let name = entry.file_name();
            if name == "." || name == ".." {
                continue;
            }

            let full_path = format!("{}/{}", current.trim_end_matches('/'), name);
            let rel = if full_path.starts_with(&base_canonical) {
                full_path[base_canonical.len()..]
                    .trim_start_matches('/')
                    .to_string()
            } else {
                name
            };

            let attr = entry.metadata();
            let is_dir = attr.is_dir();
            let mtime = attr.mtime.map(|t| t as u64).unwrap_or(0);
            let size = attr.size.unwrap_or(0);

            if is_dir {
                entries.push(FileMetadataEntry {
                    rel_path: rel.clone(),
                    size: 0,
                    mtime,
                    is_dir: true,
                });
                stack.push(full_path);
            } else {
                entries.push(FileMetadataEntry {
                    rel_path: rel,
                    size,
                    mtime,
                    is_dir: false,
                });
            }
        }
    }

    entries.sort_by(|a, b| a.rel_path.cmp(&b.rel_path));
    Ok(entries)
}

pub async fn execute_sync(
    sftp: Arc<SftpSession>,
    plan: SyncPlan,
    transfer_id: u64,
    events: crate::EventSink,
    cancel: CancellationToken,
) -> Result<()> {
    let mut transferred_bytes = 0u64;
    let total_bytes = plan.total_bytes;
    let start = Instant::now();
    let mut last_progress = Instant::now();

    events
        .send(crate::Event::Transfer {
            id: transfer_id,
            done: 0,
            total: total_bytes,
            speed: 0.0,
            state: crate::TransferState::Running,
        })
        .await;

    for item in &plan.items {
        if cancel.is_cancelled() {
            events
                .send(crate::Event::Transfer {
                    id: transfer_id,
                    done: transferred_bytes,
                    total: total_bytes,
                    speed: 0.0,
                    state: crate::TransferState::Cancelled,
                })
                .await;
            bail!("Sync cancelled");
        }

        let local_file = plan.local_dir.join(&item.rel_path);
        let remote_file = format!(
            "{}/{}",
            plan.remote_dir.trim_end_matches('/'),
            item.rel_path
        );

        match item.action {
            SyncActionKind::Upload => {
                if let Some(parent) = Path::new(&remote_file).parent() {
                    let _ = sftp.create_dir(parent.to_string_lossy().as_ref()).await;
                }
                let mut reader = fs::File::open(&local_file).await?;
                let mut writer = sftp
                    .open_with_flags(
                        &remote_file,
                        OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE,
                    )
                    .await?;

                let mut buf = vec![0u8; 64 * 1024];
                loop {
                    if cancel.is_cancelled() {
                        let _ = writer.close().await;
                        bail!("Sync cancelled");
                    }
                    let n = reader.read(&mut buf).await?;
                    if n == 0 {
                        break;
                    }
                    writer.write_all(&buf[..n]).await?;
                    transferred_bytes += n as u64;

                    if last_progress.elapsed().as_millis() >= 100 {
                        let elapsed = start.elapsed().as_secs_f64();
                        let speed = if elapsed > 0.0 {
                            transferred_bytes as f64 / elapsed
                        } else {
                            0.0
                        };
                        events.progress(crate::Event::Transfer {
                            id: transfer_id,
                            done: transferred_bytes,
                            total: total_bytes,
                            speed,
                            state: crate::TransferState::Running,
                        });
                        last_progress = Instant::now();
                    }
                }
                writer.flush().await?;
                let _ = writer.close().await;
            }
            SyncActionKind::Download => {
                if let Some(parent) = local_file.parent() {
                    let _ = fs::create_dir_all(parent).await;
                }
                let mut reader = sftp.open(&remote_file).await?;
                let mut writer = fs::File::create(&local_file).await?;

                let mut buf = vec![0u8; 64 * 1024];
                loop {
                    if cancel.is_cancelled() {
                        let _ = reader.close().await;
                        bail!("Sync cancelled");
                    }
                    let n = reader.read(&mut buf).await?;
                    if n == 0 {
                        break;
                    }
                    writer.write_all(&buf[..n]).await?;
                    transferred_bytes += n as u64;

                    if last_progress.elapsed().as_millis() >= 100 {
                        let elapsed = start.elapsed().as_secs_f64();
                        let speed = if elapsed > 0.0 {
                            transferred_bytes as f64 / elapsed
                        } else {
                            0.0
                        };
                        events.progress(crate::Event::Transfer {
                            id: transfer_id,
                            done: transferred_bytes,
                            total: total_bytes,
                            speed,
                            state: crate::TransferState::Running,
                        });
                        last_progress = Instant::now();
                    }
                }
                writer.flush().await?;
                let _ = reader.close().await;
            }
            SyncActionKind::DeleteRemote => {
                let _ = sftp.remove_file(&remote_file).await;
            }
            SyncActionKind::DeleteLocal => {
                let _ = fs::remove_file(&local_file).await;
            }
            SyncActionKind::Identical | SyncActionKind::Conflict => {}
        }
    }

    let elapsed = start.elapsed().as_secs_f64();
    let speed = if elapsed > 0.0 {
        transferred_bytes as f64 / elapsed
    } else {
        0.0
    };

    events
        .send(crate::Event::Transfer {
            id: transfer_id,
            done: transferred_bytes,
            total: total_bytes,
            speed,
            state: crate::TransferState::Complete,
        })
        .await;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_walk_local_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let f1 = tmp.path().join("a.txt");
        let sub = tmp.path().join("sub");
        let f2 = sub.join("b.txt");
        fs::write(&f1, "hello").await.unwrap();
        fs::create_dir_all(&sub).await.unwrap();
        fs::write(&f2, "world").await.unwrap();

        let entries = walk_local_tree(tmp.path()).await.unwrap();
        assert_eq!(entries.len(), 3);
        assert!(entries.iter().any(|e| e.rel_path == "a.txt" && !e.is_dir));
        assert!(entries.iter().any(|e| e.rel_path == "sub" && e.is_dir));
        assert!(
            entries
                .iter()
                .any(|e| e.rel_path == "sub/b.txt" && !e.is_dir)
        );
    }
}

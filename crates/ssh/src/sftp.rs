use anyhow::{Result, ensure};
use russh_sftp::{
    client::SftpSession,
    protocol::{FileAttributes, OpenFlags},
};

#[derive(Clone, Debug)]
pub struct RemoteEntry {
    pub name: String,
    pub directory: bool,
    pub symlink: bool,
    pub size: u64,
    pub modified: Option<u32>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub permissions: Option<u32>,
}
#[derive(Clone, Debug)]
pub enum FileOperation {
    List(String),
    CreateFile(String),
    CreateDirectory(String),
    Rename(String, String),
    Delete(String, bool),
    Permissions(String, u32),
    Read(String),
    Write(String, String, u64),
}
pub fn remote_join(parent: &str, name: &str) -> Result<String> {
    ensure!(
        !name.is_empty()
            && ![".", ".."].contains(&name)
            && !name.contains('/')
            && !name.contains('\0'),
        "Enter a single file or directory name"
    );
    Ok(format!("{}/{}", parent.trim_end_matches('/'), name))
}
pub async fn list(sftp: &SftpSession, path: &str) -> Result<(String, Vec<RemoteEntry>)> {
    let path = sftp.canonicalize(path).await?;
    let mut entries = Vec::new();
    for entry in sftp.read_dir(&path).await? {
        let name = entry.file_name();
        if name == "." || name == ".." {
            continue;
        }
        let attr = entry.metadata();
        entries.push(RemoteEntry {
            name,
            directory: attr.is_dir(),
            symlink: attr.is_symlink(),
            size: attr.size.unwrap_or(0),
            modified: attr.mtime,
            uid: attr.uid,
            gid: attr.gid,
            permissions: attr.permissions,
        });
    }
    entries.sort_by(|a, b| {
        b.directory
            .cmp(&a.directory)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    Ok((path, entries))
}
pub async fn read_file(sftp: &SftpSession, path: &str) -> Result<String> {
    use tokio::io::AsyncReadExt;
    let meta = sftp.metadata(path).await?;
    ensure!(meta.is_regular(), "Selected path is not a regular file");
    ensure!(
        meta.size.unwrap_or(0) <= 2 * 1024 * 1024,
        "File exceeds 2 MB limit for inline editor"
    );
    let mut file = sftp.open(path).await?;
    let mut buffer = Vec::new();
    file.read_to_end(&mut buffer).await?;
    let _ = file.close().await;
    let content =
        String::from_utf8(buffer).map_err(|_| anyhow::anyhow!("File is not valid UTF-8 text"))?;
    Ok(content)
}
async fn write_file_to_temp(
    sftp: &SftpSession,
    path: &str,
    content: &str,
    temp_path: String,
    backup_path: String,
) -> Result<()> {
    use tokio::io::AsyncWriteExt;
    let existing = if sftp.try_exists(path).await? {
        let metadata = sftp.metadata(path).await?;
        ensure!(
            metadata.is_regular(),
            "Selected destination is not a regular file"
        );
        Some(metadata)
    } else {
        None
    };
    let mut file = sftp
        .open_with_flags(
            &temp_path,
            OpenFlags::CREATE | OpenFlags::EXCLUDE | OpenFlags::WRITE,
        )
        .await?;
    let result = async {
        let write_result = file.write_all(content.as_bytes()).await;
        let flush_result = if write_result.is_ok() {
            file.flush().await
        } else {
            Ok(())
        };
        let close_result = file.close().await;
        write_result?;
        flush_result?;
        close_result?;

        if let Some(metadata) = &existing {
            // Preserve the attributes that can be applied without changing
            // ownership. Some servers reject ownership/time fields, so retry
            // with the portable permission field before giving up.
            let attributes = FileAttributes {
                uid: metadata.uid,
                user: metadata.user.clone(),
                gid: metadata.gid,
                group: metadata.group.clone(),
                permissions: metadata.permissions,
                atime: metadata.atime,
                mtime: metadata.mtime,
                ..Default::default()
            };
            if sftp.set_metadata(&temp_path, attributes).await.is_err()
                && let Some(permissions) = metadata.permissions
            {
                sftp.set_metadata(
                    &temp_path,
                    FileAttributes {
                        permissions: Some(permissions),
                        ..Default::default()
                    },
                )
                .await?;
            }

            // Keep the old destination until the new file has been published;
            // this gives servers without overwrite-on-rename a safe path and
            // lets us restore the old file if publication fails.
            ensure!(
                !sftp.try_exists(&backup_path).await?,
                "Refusing overwrite: backup path already exists"
            );
            sftp.rename(path, &backup_path).await?;
            match sftp.rename(&temp_path, path).await {
                Ok(()) => {
                    sftp.remove_file(&backup_path).await.map_err(|error| {
                        anyhow::anyhow!(
                            "File saved, but old destination cleanup failed: {error}; backup: {backup_path}"
                        )
                    })?;
                }
                Err(error) => {
                    let restored = sftp.rename(&backup_path, path).await;
                    return Err(anyhow::anyhow!(
                        "Could not replace destination: {error}; restore result: {restored:?}"
                    ));
                }
            }
        } else {
            sftp.rename(&temp_path, path).await?;
        }
        Ok::<_, anyhow::Error>(())
    }
    .await;
    if result.is_err() {
        let _ = sftp.remove_file(&temp_path).await;
    }
    result
}

pub async fn write_file(sftp: &SftpSession, path: &str, content: &str) -> Result<()> {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    write_file_to_temp(
        sftp,
        path,
        content,
        format!("{path}.kervesh-tmp-{nonce}"),
        format!("{path}.kervesh-backup-{nonce}"),
    )
    .await
}
pub async fn operate(sftp: &SftpSession, operation: FileOperation) -> Result<()> {
    match operation {
        FileOperation::CreateFile(path) => {
            sftp.open_with_flags(
                path,
                OpenFlags::CREATE | OpenFlags::EXCLUDE | OpenFlags::WRITE,
            )
            .await?
            .close()
            .await?;
        }
        FileOperation::CreateDirectory(path) => sftp.create_dir(path).await?,
        FileOperation::Rename(from, to) => sftp.rename(from, to).await?,
        FileOperation::Delete(path, directory) => {
            if directory {
                sftp.remove_dir(path).await?;
            } else {
                sftp.remove_file(path).await?;
            }
        }
        FileOperation::Permissions(path, mode) => {
            ensure!(mode <= 0o7777, "Invalid permissions");
            sftp.set_metadata(
                path,
                FileAttributes {
                    permissions: Some(mode),
                    ..Default::default()
                },
            )
            .await?;
        }
        FileOperation::List(_) | FileOperation::Read(_) | FileOperation::Write(_, _, _) => {}
    }
    Ok(())
}

/// Write through a uniquely named sibling and publish with one remote rename.
/// A failed write leaves the existing destination untouched when the server
/// provides the usual SFTP rename semantics.
pub async fn write_file_atomic(
    sftp: &SftpSession,
    path: &str,
    content: &str,
    operation_id: u64,
) -> Result<()> {
    write_file_to_temp(
        sftp,
        path,
        content,
        format!("{path}.kervesh-tmp-{operation_id}"),
        format!("{path}.kervesh-backup-{operation_id}"),
    )
    .await
}

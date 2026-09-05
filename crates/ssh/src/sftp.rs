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
        FileOperation::List(_) => {}
    }
    Ok(())
}

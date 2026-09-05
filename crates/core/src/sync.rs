use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncDirection {
    LocalToRemote,
    RemoteToLocal,
    BiDirectional,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncConflictPolicy {
    NewerWins,
    Overwrite,
    Skip,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum SyncActionKind {
    Upload,
    Download,
    DeleteRemote,
    DeleteLocal,
    Identical,
    Conflict,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMetadataEntry {
    pub rel_path: String,
    pub size: u64,
    pub mtime: u64,
    pub is_dir: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncItem {
    pub rel_path: String,
    pub local_size: Option<u64>,
    pub local_mtime: Option<u64>,
    pub remote_size: Option<u64>,
    pub remote_mtime: Option<u64>,
    pub action: SyncActionKind,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncPlan {
    pub local_dir: PathBuf,
    pub remote_dir: String,
    pub direction: SyncDirection,
    pub policy: SyncConflictPolicy,
    pub items: Vec<SyncItem>,
    pub total_bytes: u64,
    pub total_files: usize,
}

impl SyncPlan {
    pub fn compute(
        local_dir: PathBuf,
        remote_dir: String,
        direction: SyncDirection,
        policy: SyncConflictPolicy,
        local_files: &[FileMetadataEntry],
        remote_files: &[FileMetadataEntry],
    ) -> Self {
        let mut local_map: HashMap<&str, &FileMetadataEntry> = HashMap::new();
        for f in local_files {
            if !f.is_dir {
                local_map.insert(&f.rel_path, f);
            }
        }

        let mut remote_map: HashMap<&str, &FileMetadataEntry> = HashMap::new();
        for f in remote_files {
            if !f.is_dir {
                remote_map.insert(&f.rel_path, f);
            }
        }

        let mut all_paths: Vec<&str> = local_map.keys().chain(remote_map.keys()).copied().collect();
        all_paths.sort();
        all_paths.dedup();

        let mut items = Vec::new();
        let mut total_bytes = 0u64;
        let mut total_files = 0usize;

        for path in all_paths {
            let local = local_map.get(path);
            let remote = remote_map.get(path);

            let action = match (local, remote) {
                (Some(loc), None) => match direction {
                    SyncDirection::LocalToRemote | SyncDirection::BiDirectional => {
                        total_bytes += loc.size;
                        total_files += 1;
                        SyncActionKind::Upload
                    }
                    SyncDirection::RemoteToLocal => SyncActionKind::DeleteLocal,
                },
                (None, Some(rem)) => match direction {
                    SyncDirection::RemoteToLocal | SyncDirection::BiDirectional => {
                        total_bytes += rem.size;
                        total_files += 1;
                        SyncActionKind::Download
                    }
                    SyncDirection::LocalToRemote => SyncActionKind::DeleteRemote,
                },
                (Some(loc), Some(rem)) => {
                    if loc.size == rem.size
                        && (loc.mtime == rem.mtime
                            || (loc.mtime as i64 - rem.mtime as i64).abs() <= 2)
                    {
                        SyncActionKind::Identical
                    } else {
                        match direction {
                            SyncDirection::LocalToRemote => {
                                total_bytes += loc.size;
                                total_files += 1;
                                SyncActionKind::Upload
                            }
                            SyncDirection::RemoteToLocal => {
                                total_bytes += rem.size;
                                total_files += 1;
                                SyncActionKind::Download
                            }
                            SyncDirection::BiDirectional => match policy {
                                SyncConflictPolicy::NewerWins => {
                                    if loc.mtime > rem.mtime {
                                        total_bytes += loc.size;
                                        total_files += 1;
                                        SyncActionKind::Upload
                                    } else {
                                        total_bytes += rem.size;
                                        total_files += 1;
                                        SyncActionKind::Download
                                    }
                                }
                                SyncConflictPolicy::Overwrite => {
                                    total_bytes += loc.size;
                                    total_files += 1;
                                    SyncActionKind::Upload
                                }
                                SyncConflictPolicy::Skip => SyncActionKind::Conflict,
                            },
                        }
                    }
                }
                (None, None) => continue,
            };

            items.push(SyncItem {
                rel_path: path.to_string(),
                local_size: local.map(|f| f.size),
                local_mtime: local.map(|f| f.mtime),
                remote_size: remote.map(|f| f.size),
                remote_mtime: remote.map(|f| f.mtime),
                action,
            });
        }

        Self {
            local_dir,
            remote_dir,
            direction,
            policy,
            items,
            total_bytes,
            total_files,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_plan_computation() {
        let local = vec![
            FileMetadataEntry {
                rel_path: "index.html".into(),
                size: 100,
                mtime: 2000,
                is_dir: false,
            },
            FileMetadataEntry {
                rel_path: "style.css".into(),
                size: 50,
                mtime: 1000,
                is_dir: false,
            },
        ];
        let remote = vec![
            FileMetadataEntry {
                rel_path: "index.html".into(),
                size: 100,
                mtime: 2000,
                is_dir: false,
            },
            FileMetadataEntry {
                rel_path: "script.js".into(),
                size: 300,
                mtime: 1500,
                is_dir: false,
            },
        ];

        let plan = SyncPlan::compute(
            PathBuf::from("/local/site"),
            "/var/www/site".into(),
            SyncDirection::LocalToRemote,
            SyncConflictPolicy::NewerWins,
            &local,
            &remote,
        );

        assert_eq!(plan.items.len(), 3);
        let index_item = plan
            .items
            .iter()
            .find(|i| i.rel_path == "index.html")
            .unwrap();
        assert_eq!(index_item.action, SyncActionKind::Identical);

        let style_item = plan
            .items
            .iter()
            .find(|i| i.rel_path == "style.css")
            .unwrap();
        assert_eq!(style_item.action, SyncActionKind::Upload);

        let js_item = plan
            .items
            .iter()
            .find(|i| i.rel_path == "script.js")
            .unwrap();
        assert_eq!(js_item.action, SyncActionKind::DeleteRemote);
    }
}

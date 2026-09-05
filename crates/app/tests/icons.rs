use kervesh::classify::FileType;
use kervesh::icons::UiIcon;

#[test]
fn all_ui_icons_and_file_types_are_accessible() {
    let all_ui = [
        UiIcon::NewConnection,
        UiIcon::Split,
        UiIcon::Sftp,
        UiIcon::Monitor,
        UiIcon::Settings,
        UiIcon::Back,
        UiIcon::Forward,
        UiIcon::Parent,
        UiIcon::Refresh,
        UiIcon::Upload,
        UiIcon::NewFile,
        UiIcon::NewFolder,
        UiIcon::Download,
        UiIcon::Copy,
        UiIcon::Rename,
        UiIcon::Permissions,
        UiIcon::Delete,
        UiIcon::Pause,
        UiIcon::Cancel,
        UiIcon::Retry,
    ];
    assert_eq!(all_ui.len(), 20);

    let all_file_types = [
        FileType::Folder,
        FileType::GenericFile,
        FileType::Pdf,
        FileType::Text,
        FileType::Markdown,
        FileType::Rust,
        FileType::Shell,
        FileType::Config,
        FileType::Json,
        FileType::Image,
        FileType::Archive,
        FileType::Database,
        FileType::Key,
        FileType::Certificate,
        FileType::Executable,
        FileType::Code,
        FileType::Log,
        FileType::Symlink,
    ];
    assert_eq!(all_file_types.len(), 18);
}

use kervesh_ssh::RemoteEntry;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum FileType {
    Folder,
    Symlink,
    GenericFile,
    Pdf,
    Text,
    Markdown,
    Rust,
    Shell,
    Config,
    Json,
    Image,
    Archive,
    Database,
    Key,
    Certificate,
    Executable,
    Code,
    Log,
}

impl FileType {
    pub fn classify(entry: &RemoteEntry) -> Self {
        if entry.directory {
            return Self::Folder;
        }
        if entry.symlink {
            return Self::Symlink;
        }
        Self::from_filename(&entry.name)
    }

    pub fn from_filename(name: &str) -> Self {
        let lower = name.to_ascii_lowercase();

        // Exact name or prefix checks
        if lower == "id_rsa"
            || lower == "id_ed25519"
            || lower == "id_ecdsa"
            || lower == "id_dsa"
            || lower.starts_with("id_rsa.")
            || lower.starts_with("id_ed25519.")
        {
            return Self::Key;
        }

        if lower == ".env" || lower.starts_with(".env.") {
            return Self::Config;
        }

        // Multi-part extensions like .tar.gz
        if lower.ends_with(".tar.gz")
            || lower.ends_with(".tar.bz2")
            || lower.ends_with(".tar.xz")
            || lower.ends_with(".tar.zst")
        {
            return Self::Archive;
        }

        let ext = lower.rsplit('.').next().unwrap_or("");
        if ext.is_empty() || ext == lower {
            return Self::GenericFile;
        }

        match ext {
            "pdf" => Self::Pdf,
            "txt" | "text" | "nfo" => Self::Text,
            "md" | "markdown" => Self::Markdown,
            "rs" => Self::Rust,
            "sh" | "bash" | "zsh" | "fish" => Self::Shell,
            "yaml" | "yml" | "toml" | "ini" | "conf" | "cfg" | "properties" | "env" => Self::Config,
            "json" | "jsonl" | "ndjson" => Self::Json,
            "png" | "jpg" | "jpeg" | "gif" | "webp" | "svg" | "bmp" | "ico" => Self::Image,
            "zip" | "tar" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" | "zst" => Self::Archive,
            "db" | "sqlite" | "sqlite3" | "sql" | "duckdb" | "parquet" => Self::Database,
            "key" | "pem" | "ppk" => Self::Key,
            "crt" | "cer" | "p12" | "pfx" => Self::Certificate,
            "exe" | "msi" | "appimage" | "deb" | "rpm" => Self::Executable,
            "c" | "h" | "cpp" | "hpp" | "go" | "py" | "js" | "ts" | "java" | "kt" | "lua"
            | "rb" => Self::Code,
            "log" | "out" => Self::Log,
            _ => Self::GenericFile,
        }
    }

    pub fn label(&self) -> &'static str {
        match self {
            Self::Folder => "Directory",
            Self::Symlink => "Symbolic Link",
            Self::GenericFile => "File",
            Self::Pdf => "PDF Document",
            Self::Text => "Text Document",
            Self::Markdown => "Markdown Document",
            Self::Rust => "Rust Source",
            Self::Shell => "Shell Script",
            Self::Config => "Configuration",
            Self::Json => "JSON Document",
            Self::Image => "Image",
            Self::Archive => "Archive",
            Self::Database => "Database File",
            Self::Key => "Private Key",
            Self::Certificate => "Certificate",
            Self::Executable => "Executable / Package",
            Self::Code => "Source Code",
            Self::Log => "Log File",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_entry(name: &str, directory: bool, symlink: bool) -> RemoteEntry {
        RemoteEntry {
            name: name.to_string(),
            directory,
            symlink,
            size: 1024,
            modified: Some(1700000000),
            uid: Some(1000),
            gid: Some(1000),
            permissions: Some(0o644),
        }
    }

    #[test]
    fn test_directory_and_symlink() {
        assert_eq!(
            FileType::classify(&make_entry("docs", true, false)),
            FileType::Folder
        );
        assert_eq!(
            FileType::classify(&make_entry("link_to_file", false, true)),
            FileType::Symlink
        );
    }

    #[test]
    fn test_all_file_types_case_insensitive() {
        let cases = [
            ("document.pdf", FileType::Pdf),
            ("DOCUMENT.PDF", FileType::Pdf),
            ("notes.txt", FileType::Text),
            ("info.NFO", FileType::Text),
            ("README.md", FileType::Markdown),
            ("GUIDE.MARKDOWN", FileType::Markdown),
            ("main.rs", FileType::Rust),
            ("deploy.SH", FileType::Shell),
            ("config.toml", FileType::Config),
            ("settings.yaml", FileType::Config),
            (".env.production", FileType::Config),
            (".env", FileType::Config),
            ("data.json", FileType::Json),
            ("events.ndjson", FileType::Json),
            ("logo.png", FileType::Image),
            ("PHOTO.JPEG", FileType::Image),
            ("bundle.tar.gz", FileType::Archive),
            ("archive.ZIP", FileType::Archive),
            ("database.sqlite3", FileType::Database),
            ("dump.sql", FileType::Database),
            ("id_rsa", FileType::Key),
            ("ID_ED25519", FileType::Key),
            ("server.key", FileType::Key),
            ("cert.pem", FileType::Key),
            ("certificate.crt", FileType::Certificate),
            ("identity.p12", FileType::Certificate),
            ("installer.exe", FileType::Executable),
            ("package.deb", FileType::Executable),
            ("script.py", FileType::Code),
            ("main.cpp", FileType::Code),
            ("service.log", FileType::Log),
            ("build.out", FileType::Log),
            ("unknown_binary_file", FileType::GenericFile),
            ("data.unknownext", FileType::GenericFile),
        ];

        for (filename, expected) in cases {
            let entry = make_entry(filename, false, false);
            assert_eq!(
                FileType::classify(&entry),
                expected,
                "Failed classification for {filename}"
            );
        }
    }

    #[test]
    fn test_all_file_type_labels() {
        let all_types = [
            (FileType::Folder, "Directory"),
            (FileType::Symlink, "Symbolic Link"),
            (FileType::GenericFile, "File"),
            (FileType::Pdf, "PDF Document"),
            (FileType::Text, "Text Document"),
            (FileType::Markdown, "Markdown Document"),
            (FileType::Rust, "Rust Source"),
            (FileType::Shell, "Shell Script"),
            (FileType::Config, "Configuration"),
            (FileType::Json, "JSON Document"),
            (FileType::Image, "Image"),
            (FileType::Archive, "Archive"),
            (FileType::Database, "Database File"),
            (FileType::Key, "Private Key"),
            (FileType::Certificate, "Certificate"),
            (FileType::Executable, "Executable / Package"),
            (FileType::Code, "Source Code"),
            (FileType::Log, "Log File"),
        ];

        for (ft, label) in all_types {
            assert_eq!(ft.label(), label);
        }
    }
}

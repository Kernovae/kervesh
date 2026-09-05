#[cfg(test)]
mod tests {
    #[test]
    fn workspace_opens_without_hosts_or_network_and_renders_host_form() {
        let store = kervesh_core::Store::open_memory().unwrap();
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let mut app = crate::app::App::new(store, runtime).unwrap();
        let ctx = egui::Context::default();
        let output = ctx.run(egui::RawInput::default(), |ctx| app.render(ctx));
        assert!(!output.shapes.is_empty());
        assert_eq!(app.tab_count(), 0);
        app.open_new_host();
        let output = ctx.run(egui::RawInput::default(), |ctx| app.render(ctx));
        assert!(!output.shapes.is_empty());
    }
}
mod app;
mod files;
mod hosts;
mod settings;

fn main() -> anyhow::Result<()> {
    let path = if let Some(path) = std::env::var_os("KERVESH_DATA_DIR") {
        let directory = std::path::PathBuf::from(path);
        std::fs::create_dir_all(&directory)?;
        directory.join("kervesh.db")
    } else {
        kervesh_core::Store::default_path()?
    };
    let store = kervesh_core::Store::open(&path)?;
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()?;
    let app = app::App::new(store, runtime)?;
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Kervesh — Native remote workspace")
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([850.0, 540.0]),
        ..Default::default()
    };
    eframe::run_native("Kervesh", options, Box::new(move |_cc| Ok(Box::new(app))))
        .map_err(|e| anyhow::anyhow!("Native window failed: {e}"))
}

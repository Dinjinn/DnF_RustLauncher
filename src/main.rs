#![windows_subsystem = "windows"]
mod app;
mod config;
mod db;
mod theme;

use anyhow::{Context, Result};
use std::sync::Arc;
use eframe::egui;
use tracing_subscriber::EnvFilter;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    let app_config = config::AppConfig::from_env().context("load env config")?;
    let db = Arc::new(db::Db::new(&app_config).context("load private key")?);
    run(app_config, db).context("run app")
}

fn run(app_config: config::AppConfig, db: Arc<db::Db>) -> Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([400.0, 650.0]),
        ..Default::default()
    };

    eframe::run_native(
        "ADNF LAUNCHER",
        options,
        Box::new(|_cc| Ok(Box::new(app::LauncherApp::new(app_config.clone(), Arc::clone(&db))))),
    )
    .map_err(|err| anyhow::anyhow!("run eframe app: {err}"))?;

    Ok(())
}

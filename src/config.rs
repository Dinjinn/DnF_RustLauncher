use std::env;
use std::fs;
use std::path::Path;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug)]
pub struct AppConfig {
    pub db_main_url: String,
    pub db_billing_url: String,
    pub db_char_url: String,
    pub db_inventory_url: String,
    pub db_login_url: String,
    pub dnf_exe_path: String,
}

#[derive(Serialize, Deserialize, Default, Clone)]
pub struct UserConfig {
    pub username: String,
    pub password: String,
    pub remember: bool,
}

impl AppConfig {
    pub fn from_env() -> Result<Self> {
        let _ = dotenvy::dotenv();

        let dnf_exe_path = env::var("DNF_EXE_PATH").unwrap_or_else(|_| "ADNF.exe".to_string());

        if let Ok(base_url) = env::var("DFO_DB_BASE_URL") {
            let base = base_url.trim_end_matches('/');
            return Ok(Self {
                db_main_url: format!("{base}/d_taiwan"),
                db_billing_url: format!("{base}/taiwan_billing"),
                db_char_url: format!("{base}/taiwan_cain"),
                db_inventory_url: format!("{base}/taiwan_cain_2nd"),
                db_login_url: format!("{base}/taiwan_login"),
                dnf_exe_path,
            });
        }

        Ok(Self {
            db_main_url: env::var("DFO_DB_MAIN_URL").context("DFO_DB_MAIN_URL missing")?,
            db_billing_url: env::var("DFO_DB_BILLING_URL").context("DFO_DB_BILLING_URL missing")?,
            db_char_url: env::var("DFO_DB_CHAR_URL").context("DFO_DB_CHAR_URL missing")?,
            db_inventory_url: env::var("DFO_DB_INVENTORY_URL")
                .context("DFO_DB_INVENTORY_URL missing")?,
            db_login_url: env::var("DFO_DB_LOGIN_URL").context("DFO_DB_LOGIN_URL missing")?,
            dnf_exe_path,
        })
    }
}

pub fn read_json<T: for<'de> Deserialize<'de>>(path: impl AsRef<Path>) -> Option<T> {
    fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok())
}

pub fn write_json<T: Serialize>(path: impl AsRef<Path>, value: &T) -> Result<()> {
    let data = serde_json::to_string(value)?;
    fs::write(path, data)?;
    Ok(())
}

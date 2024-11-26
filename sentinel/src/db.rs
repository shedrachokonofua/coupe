use once_cell::sync::Lazy;
use sled::Db;
use tracing::error;
use std::{ env::{ self, current_dir }, fs, path::PathBuf, sync::Arc };
use anyhow::Result;

fn db_dir() -> Result<PathBuf> {
    env::var("DB_DIR")
        .map(PathBuf::from)
        .or_else(|_| current_dir().map(|p| p.join("db")))
        .map_err(|e| anyhow::anyhow!("Failed to determine database directory: {}", e))
}

fn ensure_db_dir() -> Result<()> {
    let dir = db_dir()?;
    fs::create_dir_all(&dir).map_err(|e|
        anyhow::anyhow!("Failed to create database directory: {}", e)
    )
}

fn open_db() -> Result<Arc<Db>> {
    ensure_db_dir()?;
    sled::open(db_dir()?)
        .map(Arc::new)
        .map_err(|e| anyhow::anyhow!("Failed to open database: {}", e))
}

pub static DB: Lazy<Arc<Db>> = Lazy::new(|| {
    open_db()
        .inspect_err(|e| {
            error!("Failed to open database: {}", e);
        })
        .expect("Failed to open database")
});

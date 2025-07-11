use coupe::{CoupeError, Result};
use fjall::TransactionalKeyspace;
use std::sync::LazyLock;
use std::{
    env::{self, current_dir},
    fs,
    path::PathBuf,
};
use tracing::error;

fn db_dir() -> Result<PathBuf> {
    env::var("DB_DIR")
        .map(PathBuf::from)
        .or_else(|_| current_dir().map(|p| p.join("db")))
        .map_err(|e| CoupeError::Io(e))
}

fn ensure_db_dir() -> Result<()> {
    let dir = db_dir()?;
    fs::create_dir_all(&dir).map_err(|e| CoupeError::Io(e))
}

fn open_db() -> Result<TransactionalKeyspace> {
    ensure_db_dir()?;
    let keyspace = fjall::Config::new(db_dir()?)
        .open_transactional()
        .map_err(|e| CoupeError::Database(e.to_string()))?;
    Ok(keyspace)
}

pub static DB: LazyLock<TransactionalKeyspace> = LazyLock::new(|| {
    open_db()
        .inspect_err(|e| {
            error!(error = %e, "Failed to open database");
        })
        .expect("Failed to open database")
});

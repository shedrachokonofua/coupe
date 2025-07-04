use thiserror::Error;

pub type Result<T, E = CoupeError> = std::result::Result<T, E>;

#[derive(Error, Debug)]
pub enum CoupeError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Invalid input: {0}")]
    InvalidInput(String),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Docker error: {0}")]
    Docker(String),

    #[error("SSH connection error: {0}")]
    SshConnection(String),

    #[error("SSH command error: {0}")]
    SshCommand(String),

    #[error("File system error: {0}")]
    FileSystem(String),

    #[error("Database error: {0}")]
    Database(String),

    #[error("Date/Time error: {0}")]
    DateTime(String),
}

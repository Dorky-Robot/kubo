use thiserror::Error;

#[derive(Debug, Error)]
pub enum KuboError {
    #[error("docker is not available: {0}")]
    DockerNotFound(String),

    #[error("container error: {0}")]
    Container(String),

    #[error("path error: {0}")]
    InvalidPath(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

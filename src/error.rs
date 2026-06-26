use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("TOML parse error: {0}")]
    Toml(#[from] toml::de::Error),

    #[error("ZIP write error: {0}")]
    Zip(#[from] zip::result::ZipError),

    #[error("unsupported feature: {0}")]
    UnsupportedFeature(&'static str),

    #[error("invalid style value: {0}")]
    InvalidStyle(String),

    #[error("missing image file: {0}")]
    MissingImage(PathBuf),

    #[error("unsupported image format: {0}")]
    UnsupportedImageFormat(PathBuf),
}

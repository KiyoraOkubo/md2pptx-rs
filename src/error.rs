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

    #[error("invalid Markdown: {0}")]
    InvalidMarkdown(String),

    #[error("invalid style value: {0}")]
    InvalidStyle(String),

    #[error("missing image file: {0}")]
    MissingImage(PathBuf),

    #[error("unsupported image format: {0}")]
    UnsupportedImageFormat(PathBuf),

    #[error("Mermaid renderer not found: {0}")]
    MermaidRendererNotFound(String),

    #[error("Mermaid renderer failed: {0}")]
    MermaidRendererFailed(String),

    #[error("invalid Mermaid renderer output: {0}")]
    InvalidMermaidOutput(String),
}

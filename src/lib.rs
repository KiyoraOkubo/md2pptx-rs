pub mod cli;
pub mod diagnostics;
pub mod error;
pub mod markdown;
pub mod model;
pub mod pptx;
pub mod style;

use std::path::Path;

use error::Result;

pub fn convert(markdown: &Path, output: &Path, style: Option<&Path>) -> Result<Vec<String>> {
    let markdown_text = std::fs::read_to_string(markdown)?;
    let style = style::Style::load(style)?;
    // Image paths in Markdown are resolved from the input file location, not
    // from the process working directory.
    let presentation = markdown::parse_markdown(
        &markdown_text,
        markdown.parent().unwrap_or(Path::new(".")),
        style.math.renderer,
    )?;
    pptx::write_pptx(&presentation, &style, output)
}

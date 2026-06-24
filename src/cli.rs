use std::path::PathBuf;

use clap::Parser;

#[derive(Debug, Parser)]
#[command(name = "md2pptx")]
#[command(about = "Convert Markdown slides to a minimal PPTX file.")]
pub struct Args {
    pub input: PathBuf,

    #[arg(short, long)]
    pub output: PathBuf,

    #[arg(long)]
    pub style: Option<PathBuf>,
}

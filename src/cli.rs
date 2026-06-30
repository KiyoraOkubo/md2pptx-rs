use std::path::PathBuf;

use clap::{Parser, ValueEnum};

#[derive(Debug, Parser)]
#[command(name = "md2pptx")]
#[command(about = "Convert Markdown slides to a minimal PPTX file.")]
pub struct Args {
    pub input: PathBuf,

    #[arg(short, long)]
    pub output: PathBuf,

    #[arg(long)]
    pub style: Option<PathBuf>,

    #[arg(long, value_enum, default_value = "auto")]
    pub color: ColorMode,

    #[arg(short, long)]
    pub quiet: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ColorMode {
    Auto,
    Always,
    Never,
}

impl ColorMode {
    pub fn anstream_choice(self) -> anstream::ColorChoice {
        match self {
            Self::Auto => anstream::ColorChoice::Auto,
            Self::Always => anstream::ColorChoice::Always,
            Self::Never => anstream::ColorChoice::Never,
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Args, ColorMode};

    #[test]
    fn uses_default_diagnostic_options() {
        let args = Args::parse_from(["md2pptx", "slides.md", "-o", "slides.pptx"]);

        assert_eq!(args.color, ColorMode::Auto);
        assert!(!args.quiet);
    }

    #[test]
    fn parses_color_mode_and_quiet_flag() {
        let args = Args::parse_from([
            "md2pptx",
            "slides.md",
            "-o",
            "slides.pptx",
            "--color",
            "never",
            "--quiet",
        ]);

        assert_eq!(args.color, ColorMode::Never);
        assert!(args.quiet);
    }
}

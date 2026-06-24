use std::path::Path;

use serde::Deserialize;

use crate::error::Result;

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Style {
    pub slide: SlideStyle,
    pub title: TextStyle,
    pub body: TextStyle,
    pub list: ListStyle,
    pub code_inline: BoxStyle,
    pub code_block: BoxStyle,
    pub quote: QuoteStyle,
    pub image: ImageStyle,
    pub math: MathStyle,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            slide: SlideStyle::default(),
            title: TextStyle {
                font_family: "Arial".into(),
                font_size: 36.0,
                color: "#111111".into(),
                bold: true,
                italic: false,
                margin: 0.0,
                margin_bottom: 24.0,
                line_spacing: 1.1,
            },
            body: TextStyle::default(),
            list: ListStyle::default(),
            code_inline: BoxStyle {
                font_family: "Consolas".into(),
                font_size: 20.0,
                color: "#c7254e".into(),
                background: "#f9f2f4".into(),
                padding: 0.0,
                margin: 0.0,
            },
            code_block: BoxStyle::default(),
            quote: QuoteStyle::default(),
            image: ImageStyle::default(),
            math: MathStyle::default(),
        }
    }
}

impl Style {
    pub fn load(path: Option<&Path>) -> Result<Self> {
        match path {
            // Every style struct has #[serde(default)], so style files can
            // override only the values they care about.
            Some(path) => Ok(toml::from_str(&std::fs::read_to_string(path)?)?),
            None => Ok(Self::default()),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct SlideStyle {
    pub size: SlideSize,
    pub background: String,
    pub padding: f64,
}

impl Default for SlideStyle {
    fn default() -> Self {
        Self {
            size: SlideSize::Wide16x9,
            background: "#ffffff".into(),
            padding: 40.0,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub enum SlideSize {
    #[serde(rename = "16:9")]
    Wide16x9,
    #[serde(rename = "4:3")]
    Standard4x3,
}

impl SlideSize {
    pub fn dimensions_pt(self) -> (f64, f64) {
        match self {
            // Use common PowerPoint point dimensions; conversion to EMUs
            // happens at XML emission time.
            SlideSize::Wide16x9 => (960.0, 540.0),
            SlideSize::Standard4x3 => (720.0, 540.0),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct TextStyle {
    pub font_family: String,
    pub font_size: f64,
    pub color: String,
    pub bold: bool,
    pub italic: bool,
    pub margin: f64,
    pub margin_bottom: f64,
    pub line_spacing: f64,
}

impl Default for TextStyle {
    fn default() -> Self {
        Self {
            font_family: "Arial".into(),
            font_size: 22.0,
            color: "#222222".into(),
            bold: false,
            italic: false,
            margin: 0.0,
            margin_bottom: 12.0,
            line_spacing: 1.2,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ListStyle {
    pub font_family: String,
    pub font_size: f64,
    pub color: String,
    pub indent: f64,
    pub margin: f64,
    pub margin_bottom: f64,
    pub line_spacing: f64,
}

impl Default for ListStyle {
    fn default() -> Self {
        Self {
            font_family: "Arial".into(),
            font_size: 22.0,
            color: "#222222".into(),
            indent: 28.0,
            margin: 0.0,
            margin_bottom: 8.0,
            line_spacing: 1.2,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct BoxStyle {
    pub font_family: String,
    pub font_size: f64,
    pub color: String,
    pub background: String,
    pub padding: f64,
    pub margin: f64,
}

impl Default for BoxStyle {
    fn default() -> Self {
        Self {
            font_family: "Consolas".into(),
            font_size: 16.0,
            color: "#111111".into(),
            background: "#f5f5f5".into(),
            padding: 16.0,
            margin: 12.0,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct QuoteStyle {
    pub font_family: String,
    pub font_size: f64,
    pub color: String,
    pub border_color: String,
    pub padding: f64,
    pub margin: f64,
}

impl Default for QuoteStyle {
    fn default() -> Self {
        Self {
            font_family: "Arial".into(),
            font_size: 20.0,
            color: "#555555".into(),
            border_color: "#cccccc".into(),
            padding: 12.0,
            margin: 12.0,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ImageStyle {
    pub max_width: String,
    pub margin: f64,
}

impl Default for ImageStyle {
    fn default() -> Self {
        Self {
            max_width: "100%".into(),
            margin: 16.0,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MathStyle {
    pub renderer: MathRenderer,
}

impl Default for MathStyle {
    fn default() -> Self {
        Self {
            renderer: MathRenderer::Literal,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MathRenderer {
    None,
    Literal,
    External,
    Katex,
    Typst,
    Tectonic,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_minimal_style_toml() {
        let style: Style = toml::from_str(
            r##"
[slide]
size = "4:3"
background = "#eeeeee"
padding = 32

[title]
font_size = 30
"##,
        )
        .unwrap();

        assert!(matches!(style.slide.size, SlideSize::Standard4x3));
        assert_eq!(style.slide.padding, 32.0);
        assert_eq!(style.title.font_size, 30.0);
        assert_eq!(style.body.font_size, 22.0);
        assert_eq!(style.math.renderer, MathRenderer::Literal);
    }
}

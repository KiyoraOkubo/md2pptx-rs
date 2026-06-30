use std::path::Path;

use serde::Deserialize;

use crate::error::{Error, Result};

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Style {
    pub slide: SlideStyle,
    #[serde(default = "title_style")]
    pub title: TextStyle,
    #[serde(default = "heading_2_style")]
    pub heading_2: TextStyle,
    #[serde(default = "heading_3_style")]
    pub heading_3: TextStyle,
    #[serde(default = "heading_4_style")]
    pub heading_4: TextStyle,
    #[serde(default = "heading_5_style")]
    pub heading_5: TextStyle,
    #[serde(default = "heading_6_style")]
    pub heading_6: TextStyle,
    pub body: TextStyle,
    pub list: ListStyle,
    #[serde(default = "code_inline_style")]
    pub code_inline: BoxStyle,
    pub code_block: BoxStyle,
    pub quote: QuoteStyle,
    pub image: ImageStyle,
    pub columns: ColumnsStyle,
    pub math: MathStyle,
}

impl Default for Style {
    fn default() -> Self {
        Self {
            slide: SlideStyle::default(),
            title: title_style(),
            heading_2: heading_2_style(),
            heading_3: heading_3_style(),
            heading_4: heading_4_style(),
            heading_5: heading_5_style(),
            heading_6: heading_6_style(),
            body: TextStyle::default(),
            list: ListStyle::default(),
            code_inline: code_inline_style(),
            code_block: BoxStyle::default(),
            quote: QuoteStyle::default(),
            image: ImageStyle::default(),
            columns: ColumnsStyle::default(),
            math: MathStyle::default(),
        }
    }
}

impl Style {
    pub fn load(path: Option<&Path>) -> Result<Self> {
        let style = match path {
            // Every style struct has #[serde(default)], so style files can
            // override only the values they care about.
            Some(path) => toml::from_str(&std::fs::read_to_string(path)?)?,
            None => Self::default(),
        };
        style.validate()?;
        Ok(style)
    }

    pub fn validate(&self) -> Result<()> {
        validate_color("slide.background", &self.slide.background)?;
        validate_non_negative("slide.padding", self.slide.padding)?;

        validate_text_style("title", &self.title)?;
        validate_text_style("heading_2", &self.heading_2)?;
        validate_text_style("heading_3", &self.heading_3)?;
        validate_text_style("heading_4", &self.heading_4)?;
        validate_text_style("heading_5", &self.heading_5)?;
        validate_text_style("heading_6", &self.heading_6)?;
        validate_text_style("body", &self.body)?;
        validate_list_style("list", &self.list)?;
        validate_box_style("code_inline", &self.code_inline)?;
        validate_box_style("code_block", &self.code_block)?;
        validate_quote_style("quote", &self.quote)?;
        validate_image_style("image", &self.image)?;
        validate_columns_style("columns", &self.columns)?;
        Ok(())
    }
}

fn title_style() -> TextStyle {
    TextStyle {
        font_family: "Arial".into(),
        font_size: 36.0,
        color: "#111111".into(),
        bold: true,
        italic: false,
        margin: 0.0,
        margin_bottom: 24.0,
        line_spacing: 1.1,
    }
}

fn heading_2_style() -> TextStyle {
    heading_style(30.0)
}

fn heading_3_style() -> TextStyle {
    heading_style(26.0)
}

fn heading_4_style() -> TextStyle {
    heading_style(23.0)
}

fn heading_5_style() -> TextStyle {
    heading_style(21.0)
}

fn heading_6_style() -> TextStyle {
    heading_style(19.0)
}

fn heading_style(font_size: f64) -> TextStyle {
    TextStyle {
        font_family: "Arial".into(),
        font_size,
        color: "#222222".into(),
        bold: true,
        italic: false,
        margin: 0.0,
        margin_bottom: 14.0,
        line_spacing: 1.15,
    }
}

fn code_inline_style() -> BoxStyle {
    BoxStyle {
        font_family: "Consolas".into(),
        font_size: 20.0,
        color: "#c7254e".into(),
        background: "#f9f2f4".into(),
        padding: 0.0,
        margin: 0.0,
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
    pub align: ImageAlign,
    pub margin: f64,
}

impl Default for ImageStyle {
    fn default() -> Self {
        Self {
            max_width: "100%".into(),
            align: ImageAlign::Left,
            margin: 16.0,
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ColumnsStyle {
    pub gap: f64,
}

impl Default for ColumnsStyle {
    fn default() -> Self {
        Self { gap: 24.0 }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ImageAlign {
    Left,
    Center,
    Right,
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

fn validate_text_style(section: &str, style: &TextStyle) -> Result<()> {
    validate_font_family(section, &style.font_family)?;
    validate_positive(&format!("{section}.font_size"), style.font_size)?;
    validate_color(&format!("{section}.color"), &style.color)?;
    validate_non_negative(&format!("{section}.margin"), style.margin)?;
    validate_non_negative(&format!("{section}.margin_bottom"), style.margin_bottom)?;
    validate_positive(&format!("{section}.line_spacing"), style.line_spacing)
}

fn validate_list_style(section: &str, style: &ListStyle) -> Result<()> {
    validate_font_family(section, &style.font_family)?;
    validate_positive(&format!("{section}.font_size"), style.font_size)?;
    validate_color(&format!("{section}.color"), &style.color)?;
    validate_non_negative(&format!("{section}.indent"), style.indent)?;
    validate_non_negative(&format!("{section}.margin"), style.margin)?;
    validate_non_negative(&format!("{section}.margin_bottom"), style.margin_bottom)?;
    validate_positive(&format!("{section}.line_spacing"), style.line_spacing)
}

fn validate_box_style(section: &str, style: &BoxStyle) -> Result<()> {
    validate_font_family(section, &style.font_family)?;
    validate_positive(&format!("{section}.font_size"), style.font_size)?;
    validate_color(&format!("{section}.color"), &style.color)?;
    validate_color(&format!("{section}.background"), &style.background)?;
    validate_non_negative(&format!("{section}.padding"), style.padding)?;
    validate_non_negative(&format!("{section}.margin"), style.margin)
}

fn validate_quote_style(section: &str, style: &QuoteStyle) -> Result<()> {
    validate_font_family(section, &style.font_family)?;
    validate_positive(&format!("{section}.font_size"), style.font_size)?;
    validate_color(&format!("{section}.color"), &style.color)?;
    validate_color(&format!("{section}.border_color"), &style.border_color)?;
    validate_non_negative(&format!("{section}.padding"), style.padding)?;
    validate_non_negative(&format!("{section}.margin"), style.margin)
}

fn validate_image_style(section: &str, style: &ImageStyle) -> Result<()> {
    validate_image_max_width(&format!("{section}.max_width"), &style.max_width)?;
    validate_non_negative(&format!("{section}.margin"), style.margin)
}

fn validate_columns_style(section: &str, style: &ColumnsStyle) -> Result<()> {
    validate_non_negative(&format!("{section}.gap"), style.gap)
}

fn validate_font_family(section: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        return Err(Error::InvalidStyle(format!(
            "{section}.font_family must not be empty"
        )));
    }
    Ok(())
}

fn validate_color(field: &str, value: &str) -> Result<()> {
    let hex = value.trim().strip_prefix('#').unwrap_or(value.trim());
    if hex.len() == 6 && hex.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Ok(());
    }
    Err(Error::InvalidStyle(format!(
        "{field} must be a hex RGB color such as #RRGGBB or RRGGBB"
    )))
}

fn validate_positive(field: &str, value: f64) -> Result<()> {
    if value.is_finite() && value > 0.0 {
        return Ok(());
    }
    Err(Error::InvalidStyle(format!(
        "{field} must be greater than 0"
    )))
}

fn validate_non_negative(field: &str, value: f64) -> Result<()> {
    if value.is_finite() && value >= 0.0 {
        return Ok(());
    }
    Err(Error::InvalidStyle(format!("{field} must not be negative")))
}

fn validate_image_max_width(field: &str, value: &str) -> Result<()> {
    let trimmed = value.trim();
    if let Some(percent) = trimmed.strip_suffix('%') {
        if percent
            .trim()
            .parse::<f64>()
            .is_ok_and(|value| value.is_finite() && value > 0.0 && value <= 100.0)
        {
            return Ok(());
        }
    } else if trimmed
        .parse::<f64>()
        .is_ok_and(|value| value.is_finite() && value > 0.0)
    {
        return Ok(());
    }

    Err(Error::InvalidStyle(format!(
        "{field} must be a point value greater than 0 or a percent greater than 0% and at most 100%"
    )))
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
        assert_eq!(style.heading_2.font_size, 30.0);
        assert!(style.heading_2.bold);
        assert_eq!(style.body.font_size, 22.0);
        assert_eq!(style.columns.gap, 24.0);
        assert_eq!(style.math.renderer, MathRenderer::Literal);
        style.validate().unwrap();
    }

    #[test]
    fn loads_empty_style_toml_with_full_defaults() {
        let style: Style = toml::from_str("").unwrap();

        assert_eq!(style.title.font_size, 36.0);
        assert_eq!(style.code_inline.font_size, 20.0);
        assert_eq!(style.heading_6.font_size, 19.0);
        assert_eq!(style.columns.gap, 24.0);
        style.validate().unwrap();
    }

    #[test]
    fn rejects_invalid_hex_color() {
        let mut style = Style::default();
        style.body.color = "blue".into();

        let error = style.validate().unwrap_err().to_string();
        assert!(error.contains("body.color"));
    }

    #[test]
    fn rejects_empty_font_family() {
        let mut style = Style::default();
        style.title.font_family = " ".into();

        let error = style.validate().unwrap_err().to_string();
        assert!(error.contains("title.font_family"));
    }

    #[test]
    fn rejects_non_positive_font_size() {
        let mut style = Style::default();
        style.heading_3.font_size = 0.0;

        let error = style.validate().unwrap_err().to_string();
        assert!(error.contains("heading_3.font_size"));
    }

    #[test]
    fn rejects_negative_spacing_values() {
        let mut style = Style::default();
        style.list.indent = -1.0;

        let error = style.validate().unwrap_err().to_string();
        assert!(error.contains("list.indent"));
    }

    #[test]
    fn rejects_invalid_image_max_width() {
        for value in ["0%", "-10%", "120%", "wide"] {
            let mut style = Style::default();
            style.image.max_width = value.into();

            let error = style.validate().unwrap_err().to_string();
            assert!(error.contains("image.max_width"));
        }
    }

    #[test]
    fn rejects_negative_columns_gap() {
        let mut style = Style::default();
        style.columns.gap = -1.0;

        let error = style.validate().unwrap_err().to_string();
        assert!(error.contains("columns.gap"));
    }
}

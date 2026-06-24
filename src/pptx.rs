use std::{
    fs::File,
    io::{Read, Write},
    path::{Path, PathBuf},
};

use zip::{ZipWriter, write::SimpleFileOptions};

use crate::{
    error::{Error, Result},
    model::{Block, Inline, Presentation, Slide, TableAlignment},
    style::{BoxStyle, ListStyle, QuoteStyle, Style, TextStyle},
};

const EMU_PER_PT: f64 = 12_700.0;

pub fn write_pptx(
    presentation: &Presentation,
    style: &Style,
    output: &Path,
) -> Result<Vec<String>> {
    let file = File::create(output)?;
    let mut zip = ZipWriter::new(file);
    let options = SimpleFileOptions::default();
    let mut warnings = Vec::new();
    let mut media = Vec::new();

    // Discover media before writing [Content_Types].xml and relationships,
    // because those package parts must list image content types up front.
    let mut media_number = 0;
    for (slide_index, slide) in presentation.slides.iter().enumerate() {
        for block in &slide.blocks {
            if let Block::Image { path, .. } = block {
                media_number += 1;
                media.push(MediaFile::new(slide_index + 1, media_number, path)?);
            }
        }
    }

    write_file(
        &mut zip,
        options,
        "[Content_Types].xml",
        &content_types(presentation.slides.len(), &media),
    )?;
    write_file(&mut zip, options, "_rels/.rels", ROOT_RELS)?;
    write_file(&mut zip, options, "docProps/app.xml", APP_PROPS)?;
    write_file(&mut zip, options, "docProps/core.xml", CORE_PROPS)?;
    write_file(
        &mut zip,
        options,
        "ppt/presentation.xml",
        &presentation_xml(presentation.slides.len(), style),
    )?;
    write_file(
        &mut zip,
        options,
        "ppt/_rels/presentation.xml.rels",
        &presentation_rels(presentation.slides.len()),
    )?;
    // PowerPoint treats these package parts as part of a normal presentation,
    // even when every slide is rendered from explicit shapes.
    write_file(
        &mut zip,
        options,
        "ppt/slideMasters/slideMaster1.xml",
        SLIDE_MASTER,
    )?;
    write_file(
        &mut zip,
        options,
        "ppt/slideMasters/_rels/slideMaster1.xml.rels",
        SLIDE_MASTER_RELS,
    )?;
    write_file(
        &mut zip,
        options,
        "ppt/slideLayouts/slideLayout1.xml",
        SLIDE_LAYOUT,
    )?;
    write_file(
        &mut zip,
        options,
        "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
        SLIDE_LAYOUT_RELS,
    )?;
    write_file(&mut zip, options, "ppt/theme/theme1.xml", THEME)?;

    for (index, slide) in presentation.slides.iter().enumerate() {
        let slide_number = index + 1;
        // Relationships are scoped per slide, while media filenames are
        // package-wide. Keep both numbering schemes explicit.
        let slide_media = media
            .iter()
            .filter(|media| media.slide_number == slide_number)
            .cloned()
            .collect::<Vec<_>>();
        let rendered = render_slide(slide, style, slide_number, &slide_media, &mut warnings);
        write_file(
            &mut zip,
            options,
            &format!("ppt/slides/slide{slide_number}.xml"),
            &rendered,
        )?;
        write_file(
            &mut zip,
            options,
            &format!("ppt/slides/_rels/slide{slide_number}.xml.rels"),
            &slide_rels(&slide_media),
        )?;
        for media_file in slide_media {
            write_bytes(
                &mut zip,
                options,
                &format!(
                    "ppt/media/image{}.{}",
                    media_file.media_number, media_file.extension
                ),
                &media_file.bytes,
            )?;
        }
    }

    zip.finish()?;
    Ok(warnings)
}

fn write_file<W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    options: SimpleFileOptions,
    name: &str,
    content: &str,
) -> Result<()> {
    zip.start_file(name, options)?;
    zip.write_all(content.as_bytes())?;
    Ok(())
}

fn write_bytes<W: Write + std::io::Seek>(
    zip: &mut ZipWriter<W>,
    options: SimpleFileOptions,
    name: &str,
    content: &[u8],
) -> Result<()> {
    zip.start_file(name, options)?;
    zip.write_all(content)?;
    Ok(())
}

#[derive(Clone)]
struct MediaFile {
    slide_number: usize,
    media_number: usize,
    path: PathBuf,
    dimensions: ImageDimensions,
    extension: String,
    content_type: &'static str,
    bytes: Vec<u8>,
}

#[derive(Clone, Copy)]
struct ImageDimensions {
    width: u32,
    height: u32,
}

impl MediaFile {
    fn new(slide_number: usize, media_number: usize, path: &Path) -> Result<Self> {
        if !path.exists() {
            return Err(Error::MissingImage(path.to_path_buf()));
        }
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase())
            .ok_or_else(|| Error::UnsupportedImageFormat(path.to_path_buf()))?;
        let content_type = match extension.as_str() {
            "png" => "image/png",
            "jpg" | "jpeg" => "image/jpeg",
            _ => return Err(Error::UnsupportedImageFormat(path.to_path_buf())),
        };
        let mut bytes = Vec::new();
        File::open(path)?.read_to_end(&mut bytes)?;
        // Dimensions are needed to preserve aspect ratio in the slide XML.
        let dimensions = image_dimensions(&extension, &bytes)
            .ok_or_else(|| Error::UnsupportedImageFormat(path.to_path_buf()))?;
        Ok(Self {
            slide_number,
            media_number,
            path: path.to_path_buf(),
            dimensions,
            extension,
            content_type,
            bytes,
        })
    }
}

fn render_slide(
    slide: &Slide,
    style: &Style,
    slide_number: usize,
    media: &[MediaFile],
    warnings: &mut Vec<String>,
) -> String {
    let (slide_w, slide_h) = style.slide.size.dimensions_pt();
    let padding = style.slide.padding;
    let content_w = slide_w - padding * 2.0;
    let max_y = slide_h - padding;
    let mut y = padding;
    // Shape IDs only need to be unique within one slide.
    let mut shape_id = 2;
    let mut image_index = 0;
    let mut shapes = String::new();

    if let Some(title) = &slide.title {
        let height = estimate_text_height(
            title,
            content_w,
            style.title.font_size,
            style.title.line_spacing,
        );
        shapes.push_str(&text_box(
            shape_id,
            padding,
            y,
            content_w,
            height,
            title,
            &style.title,
            None,
            false,
            false,
        ));
        shape_id += 1;
        y += height + style.title.margin_bottom;
    }

    for block in &slide.blocks {
        match block {
            Block::Paragraph(inlines) => {
                let height = estimate_text_height(
                    inlines,
                    content_w,
                    style.body.font_size,
                    style.body.line_spacing,
                );
                shapes.push_str(&text_box(
                    shape_id,
                    padding + style.body.margin,
                    y,
                    content_w - style.body.margin * 2.0,
                    height,
                    inlines,
                    &style.body,
                    Some(&style.code_inline),
                    false,
                    false,
                ));
                shape_id += 1;
                y += height + style.body.margin_bottom;
            }
            Block::List { ordered, items } => {
                for (item_index, item) in items.iter().enumerate() {
                    let mut item_inlines = Vec::new();
                    let marker = if *ordered {
                        format!("{}. ", item_index + 1)
                    } else {
                        "- ".to_string()
                    };
                    item_inlines.push(Inline::Text(marker));
                    item_inlines.extend(item.clone());
                    let height = estimate_text_height(
                        &item_inlines,
                        content_w - style.list.indent,
                        style.list.font_size,
                        style.list.line_spacing,
                    );
                    shapes.push_str(&list_text_box(
                        shape_id,
                        padding + style.list.margin + style.list.indent,
                        y,
                        content_w - style.list.margin * 2.0 - style.list.indent,
                        height,
                        &item_inlines,
                        &style.list,
                    ));
                    shape_id += 1;
                    y += height + style.list.margin_bottom;
                }
            }
            Block::CodeBlock { code, .. } => {
                let inlines = vec![Inline::Text(code.clone())];
                let height = estimate_code_height(code, content_w, style.code_block.font_size)
                    + style.code_block.padding * 2.0;
                shapes.push_str(&box_text(
                    shape_id,
                    padding + style.code_block.margin,
                    y,
                    content_w - style.code_block.margin * 2.0,
                    height,
                    &inlines,
                    &style.code_block,
                ));
                shape_id += 1;
                y += height + style.code_block.margin;
            }
            Block::MathBlock(source) => {
                let inlines = vec![Inline::Text(source.clone())];
                let height = estimate_code_height(source, content_w, style.code_block.font_size)
                    + style.code_block.padding * 2.0;
                shapes.push_str(&box_text(
                    shape_id,
                    padding + style.code_block.margin,
                    y,
                    content_w - style.code_block.margin * 2.0,
                    height,
                    &inlines,
                    &style.code_block,
                ));
                shape_id += 1;
                y += height + style.code_block.margin;
            }
            Block::Table { alignments, rows } => {
                let (table_xml, next_shape_id, height) = table_shapes(
                    shape_id,
                    padding,
                    y,
                    content_w,
                    alignments,
                    rows,
                    &style.body,
                );
                shapes.push_str(&table_xml);
                shape_id = next_shape_id;
                y += height + style.body.margin_bottom;
            }
            Block::Quote(inlines) => {
                let height = estimate_text_height(inlines, content_w, style.quote.font_size, 1.2)
                    + style.quote.padding * 2.0;
                shapes.push_str(&quote_box(
                    shape_id,
                    padding + style.quote.margin,
                    y,
                    content_w - style.quote.margin * 2.0,
                    height,
                    inlines,
                    &style.quote,
                ));
                shape_id += 1;
                y += height + style.quote.margin;
            }
            Block::Image { alt, .. } => {
                if let Some(media_file) = media.get(image_index) {
                    // Images are laid out in the same vertical flow as text,
                    // but their height is derived from intrinsic dimensions.
                    let (width, height) = image_size(
                        content_w,
                        (max_y - y).max(80.0),
                        &style.image.max_width,
                        media_file.dimensions,
                    );
                    shapes.push_str(&image_shape(
                        shape_id,
                        padding,
                        y,
                        width,
                        height,
                        image_index + 1,
                        alt,
                    ));
                    shape_id += 1;
                    image_index += 1;
                    y += height + style.image.margin;
                    let _ = &media_file.path;
                }
            }
        }

        if y > max_y {
            warnings.push(format!(
                "slide {slide_number} content exceeds slide bounds by {:.1}pt",
                y - max_y
            ));
        }
    }

    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?>
<p:sld xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main">
  <p:cSld>
    <p:bg><p:bgPr><a:solidFill><a:srgbClr val="{}"/></a:solidFill><a:effectLst/></p:bgPr></p:bg>
    <p:spTree>
      <p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr>
      <p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr>
      {}
    </p:spTree>
  </p:cSld>
  <p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr>
</p:sld>"#,
        color(&style.slide.background),
        shapes
    )
}

fn table_shapes(
    start_id: usize,
    x: f64,
    y: f64,
    w: f64,
    alignments: &[TableAlignment],
    rows: &[crate::model::TableRow],
    style: &TextStyle,
) -> (String, usize, f64) {
    let column_count = rows
        .iter()
        .map(|row| row.cells.len())
        .max()
        .unwrap_or(0)
        .max(alignments.len());
    if column_count == 0 || rows.is_empty() {
        return (String::new(), start_id, 0.0);
    }

    let cell_padding = 6.0;
    let column_width = w / column_count as f64;
    let mut id = start_id;
    let mut current_y = y;
    let mut xml = String::new();

    for row in rows {
        let row_height = row
            .cells
            .iter()
            .map(|cell| {
                estimate_text_height(
                    cell,
                    column_width - cell_padding * 2.0,
                    style.font_size,
                    style.line_spacing,
                ) + cell_padding * 2.0
            })
            .fold(
                style.font_size * style.line_spacing + cell_padding * 2.0,
                f64::max,
            );

        for column_index in 0..column_count {
            let cell = row.cells.get(column_index).map_or(&[][..], Vec::as_slice);
            let alignment = alignments
                .get(column_index)
                .copied()
                .unwrap_or(TableAlignment::Default);
            xml.push_str(&table_cell_shape(
                id,
                x + column_width * column_index as f64,
                current_y,
                column_width,
                row_height,
                cell,
                style,
                alignment,
                row.is_header,
            ));
            id += 1;
        }

        current_y += row_height;
    }

    (xml, id, current_y - y)
}

fn table_cell_shape(
    id: usize,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    inlines: &[Inline],
    style: &TextStyle,
    alignment: TableAlignment,
    is_header: bool,
) -> String {
    let fill = if is_header { "#eef1f5" } else { "#ffffff" };
    let mut text_style = style.clone();
    text_style.bold = is_header || style.bold;
    let paragraph_props = match alignment {
        TableAlignment::Center => r#"<a:pPr algn="ctr"/>"#,
        TableAlignment::Right => r#"<a:pPr algn="r"/>"#,
        TableAlignment::Default | TableAlignment::Left => "",
    };

    format!(
        r#"<p:sp>
  <p:nvSpPr><p:cNvPr id="{id}" name="Table Cell {id}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr>
  <p:spPr><a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom><a:solidFill><a:srgbClr val="{}"/></a:solidFill><a:ln w="6350"><a:solidFill><a:srgbClr val="C9CDD3"/></a:solidFill></a:ln></p:spPr>
  <p:txBody><a:bodyPr wrap="square" lIns="{}" tIns="{}" rIns="{}" bIns="{}"/><a:lstStyle/><a:p>{paragraph_props}{}</a:p></p:txBody>
</p:sp>"#,
        emu(x),
        emu(y),
        emu(w),
        emu(h),
        color(fill),
        emu(6.0),
        emu(4.0),
        emu(6.0),
        emu(4.0),
        runs(inlines, &text_style, None, false, false)
    )
}

fn text_box(
    id: usize,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    inlines: &[Inline],
    style: &TextStyle,
    inline_code: Option<&BoxStyle>,
    force_bold: bool,
    force_italic: bool,
) -> String {
    shape(
        id,
        x,
        y,
        w,
        h,
        None,
        &runs(inlines, style, inline_code, force_bold, force_italic),
    )
}

fn list_text_box(
    id: usize,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    inlines: &[Inline],
    style: &ListStyle,
) -> String {
    let text_style = TextStyle {
        font_family: style.font_family.clone(),
        font_size: style.font_size,
        color: style.color.clone(),
        bold: false,
        italic: false,
        margin: style.margin,
        margin_bottom: style.margin_bottom,
        line_spacing: style.line_spacing,
    };
    text_box(id, x, y, w, h, inlines, &text_style, None, false, false)
}

fn box_text(
    id: usize,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    inlines: &[Inline],
    style: &BoxStyle,
) -> String {
    let text_style = TextStyle {
        font_family: style.font_family.clone(),
        font_size: style.font_size,
        color: style.color.clone(),
        bold: false,
        italic: false,
        margin: style.margin,
        margin_bottom: 0.0,
        line_spacing: 1.1,
    };
    shape(
        id,
        x,
        y,
        w,
        h,
        Some(&style.background),
        &runs(inlines, &text_style, None, false, false),
    )
}

fn quote_box(
    id: usize,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    inlines: &[Inline],
    style: &QuoteStyle,
) -> String {
    let text_style = TextStyle {
        font_family: style.font_family.clone(),
        font_size: style.font_size,
        color: style.color.clone(),
        bold: false,
        italic: true,
        margin: style.margin,
        margin_bottom: 0.0,
        line_spacing: 1.2,
    };
    // The quote border is a thin filled rectangle because the rest of the
    // writer only emits simple shapes.
    let border = shape(id, x, y, 4.0, h, Some(&style.border_color), "");
    let text = shape(
        id + 10_000,
        x + style.padding,
        y + style.padding,
        w - style.padding * 2.0,
        h - style.padding * 2.0,
        None,
        &runs(inlines, &text_style, None, false, true),
    );
    format!("{border}{text}")
}

fn shape(id: usize, x: f64, y: f64, w: f64, h: f64, fill: Option<&str>, runs: &str) -> String {
    let fill_xml = fill.map_or("<a:noFill/>".to_string(), |fill| {
        format!(
            r#"<a:solidFill><a:srgbClr val="{}"/></a:solidFill>"#,
            color(fill)
        )
    });
    format!(
        r#"<p:sp>
  <p:nvSpPr><p:cNvPr id="{id}" name="Text {id}"/><p:cNvSpPr txBox="1"/><p:nvPr/></p:nvSpPr>
  <p:spPr><a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom>{fill_xml}<a:ln><a:noFill/></a:ln></p:spPr>
  <p:txBody><a:bodyPr wrap="square"/><a:lstStyle/><a:p>{runs}</a:p></p:txBody>
</p:sp>"#,
        emu(x),
        emu(y),
        emu(w),
        emu(h)
    )
}

fn runs(
    inlines: &[Inline],
    style: &TextStyle,
    inline_code: Option<&BoxStyle>,
    force_bold: bool,
    force_italic: bool,
) -> String {
    let mut xml = String::new();
    for inline in inlines {
        let (text, bold, italic, code) = match inline {
            Inline::Text(value) => (
                value,
                force_bold || style.bold,
                force_italic || style.italic,
                false,
            ),
            Inline::Bold(value) => (value, true, force_italic || style.italic, false),
            Inline::Italic(value) => (value, force_bold || style.bold, true, false),
            Inline::Code(value) | Inline::Math(value) => (value, force_bold, force_italic, true),
        };
        let active = inline_code.filter(|_| code);
        let font = active.map_or(&style.font_family, |value| &value.font_family);
        let size = active.map_or(style.font_size, |value| value.font_size);
        let run_color = active.map_or(&style.color, |value| &value.color);
        xml.push_str(&format!(
            r#"<a:r><a:rPr lang="en-US" sz="{}"{}{}><a:solidFill><a:srgbClr val="{}"/></a:solidFill><a:latin typeface="{}"/></a:rPr><a:t>{}</a:t></a:r>"#,
            (size * 100.0).round() as i64,
            if bold { r#" b="1""# } else { "" },
            if italic { r#" i="1""# } else { "" },
            color(run_color),
            escape(font),
            escape(text)
        ));
    }
    xml
}

fn image_shape(id: usize, x: f64, y: f64, w: f64, h: f64, rel_index: usize, alt: &str) -> String {
    format!(
        r#"<p:pic>
  <p:nvPicPr><p:cNvPr id="{id}" name="{}"/><p:cNvPicPr/><p:nvPr/></p:nvPicPr>
  <p:blipFill><a:blip r:embed="rId{}"/><a:stretch><a:fillRect/></a:stretch></p:blipFill>
  <p:spPr><a:xfrm><a:off x="{}" y="{}"/><a:ext cx="{}" cy="{}"/></a:xfrm><a:prstGeom prst="rect"><a:avLst/></a:prstGeom></p:spPr>
</p:pic>"#,
        escape(alt),
        rel_index + 1,
        emu(x),
        emu(y),
        emu(w),
        emu(h)
    )
}

fn estimate_text_height(inlines: &[Inline], width: f64, font_size: f64, line_spacing: f64) -> f64 {
    let text = Inline::plain_text(inlines);
    // PowerPoint performs its own text layout. This estimate is only used to
    // advance the simple top-to-bottom flow and produce overflow warnings.
    let chars_per_line = (width / (font_size * 0.55)).max(12.0);
    let logical_lines = text
        .lines()
        .map(|line| ((line.chars().count() as f64 / chars_per_line).ceil() as usize).max(1))
        .sum::<usize>()
        .max(1);
    logical_lines as f64 * font_size * line_spacing
}

fn estimate_code_height(code: &str, width: f64, font_size: f64) -> f64 {
    let chars_per_line = (width / (font_size * 0.6)).max(10.0);
    let lines = code
        .lines()
        .map(|line| ((line.chars().count() as f64 / chars_per_line).ceil() as usize).max(1))
        .sum::<usize>()
        .max(1);
    lines as f64 * font_size * 1.15
}

fn image_width(content_w: f64, max_width: &str) -> f64 {
    if let Some(percent) = max_width.strip_suffix('%') {
        return percent
            .trim()
            .parse::<f64>()
            .map_or(content_w, |value| content_w * value / 100.0);
    }
    max_width
        .trim()
        .parse::<f64>()
        .unwrap_or(content_w)
        .min(content_w)
}

fn image_size(
    content_w: f64,
    available_h: f64,
    max_width: &str,
    dimensions: ImageDimensions,
) -> (f64, f64) {
    let mut width = image_width(content_w, max_width);
    let aspect = dimensions.height as f64 / dimensions.width as f64;
    let mut height = width * aspect;
    // If the image would run beyond the slide, shrink both dimensions instead
    // of stretching or cropping.
    if height > available_h {
        let scale = available_h / height;
        width *= scale;
        height = available_h;
    }
    (width, height)
}

fn image_dimensions(extension: &str, bytes: &[u8]) -> Option<ImageDimensions> {
    match extension {
        "png" => png_dimensions(bytes),
        "jpg" | "jpeg" => jpeg_dimensions(bytes),
        _ => None,
    }
}

fn png_dimensions(bytes: &[u8]) -> Option<ImageDimensions> {
    const PNG_SIGNATURE: &[u8; 8] = b"\x89PNG\r\n\x1a\n";
    if bytes.len() < 24 || &bytes[..8] != PNG_SIGNATURE || &bytes[12..16] != b"IHDR" {
        return None;
    }
    // PNG stores image dimensions in the IHDR chunk immediately after the
    // signature and chunk metadata.
    non_zero_dimensions(ImageDimensions {
        width: u32::from_be_bytes(bytes[16..20].try_into().ok()?),
        height: u32::from_be_bytes(bytes[20..24].try_into().ok()?),
    })
}

fn jpeg_dimensions(bytes: &[u8]) -> Option<ImageDimensions> {
    if bytes.len() < 4 || bytes[0] != 0xff || bytes[1] != 0xd8 {
        return None;
    }

    let mut index = 2;
    while index + 4 <= bytes.len() {
        // Walk JPEG marker segments until a Start Of Frame marker gives us
        // the encoded width and height.
        while index < bytes.len() && bytes[index] != 0xff {
            index += 1;
        }
        while index < bytes.len() && bytes[index] == 0xff {
            index += 1;
        }
        if index >= bytes.len() {
            break;
        }

        let marker = bytes[index];
        index += 1;
        if marker == 0xd9 || marker == 0xda {
            break;
        }
        if index + 2 > bytes.len() {
            break;
        }
        let length = u16::from_be_bytes([bytes[index], bytes[index + 1]]) as usize;
        if length < 2 || index + length > bytes.len() {
            break;
        }

        if is_jpeg_sof_marker(marker) {
            if length < 7 {
                return None;
            }
            return non_zero_dimensions(ImageDimensions {
                height: u16::from_be_bytes([bytes[index + 3], bytes[index + 4]]) as u32,
                width: u16::from_be_bytes([bytes[index + 5], bytes[index + 6]]) as u32,
            });
        }

        index += length;
    }
    None
}

fn is_jpeg_sof_marker(marker: u8) -> bool {
    matches!(
        marker,
        0xc0 | 0xc1 | 0xc2 | 0xc3 | 0xc5 | 0xc6 | 0xc7 | 0xc9 | 0xca | 0xcb | 0xcd | 0xce | 0xcf
    )
}

fn non_zero_dimensions(dimensions: ImageDimensions) -> Option<ImageDimensions> {
    if dimensions.width == 0 || dimensions.height == 0 {
        None
    } else {
        Some(dimensions)
    }
}

fn content_types(slide_count: usize, media: &[MediaFile]) -> String {
    let mut overrides = String::new();
    for slide in 1..=slide_count {
        overrides.push_str(&format!(
            r#"<Override PartName="/ppt/slides/slide{slide}.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slide+xml"/>"#
        ));
    }
    let mut defaults = String::from(
        r#"<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/>"#,
    );
    if media.iter().any(|m| m.content_type == "image/png") {
        defaults.push_str(r#"<Default Extension="png" ContentType="image/png"/>"#);
    }
    if media.iter().any(|m| m.content_type == "image/jpeg") {
        defaults.push_str(r#"<Default Extension="jpg" ContentType="image/jpeg"/><Default Extension="jpeg" ContentType="image/jpeg"/>"#);
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Types xmlns="http://schemas.openxmlformats.org/package/2006/content-types">{defaults}<Override PartName="/docProps/app.xml" ContentType="application/vnd.openxmlformats-officedocument.extended-properties+xml"/><Override PartName="/docProps/core.xml" ContentType="application/vnd.openxmlformats-package.core-properties+xml"/><Override PartName="/ppt/presentation.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.presentation.main+xml"/><Override PartName="/ppt/slideMasters/slideMaster1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideMaster+xml"/><Override PartName="/ppt/slideLayouts/slideLayout1.xml" ContentType="application/vnd.openxmlformats-officedocument.presentationml.slideLayout+xml"/><Override PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml"/>{overrides}</Types>"#
    )
}

fn presentation_xml(slide_count: usize, style: &Style) -> String {
    let (w, h) = style.slide.size.dimensions_pt();
    let master_rel_id = slide_count + 1;
    let ids = (1..=slide_count)
        .map(|idx| format!(r#"<p:sldId id="{}" r:id="rId{}"/>"#, 255 + idx, idx))
        .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:presentation xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:sldMasterIdLst><p:sldMasterId id="2147483648" r:id="rId{master_rel_id}"/></p:sldMasterIdLst><p:sldIdLst>{ids}</p:sldIdLst><p:sldSz cx="{}" cy="{}" type="screen16x9"/><p:notesSz cx="6858000" cy="9144000"/></p:presentation>"#,
        emu(w),
        emu(h)
    )
}

fn presentation_rels(slide_count: usize) -> String {
    let mut rels = String::new();
    for idx in 1..=slide_count {
        rels.push_str(&format!(
            r#"<Relationship Id="rId{idx}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slide" Target="slides/slide{idx}.xml"/>"#
        ));
    }
    let master_rel_id = slide_count + 1;
    rels.push_str(&format!(
        r#"<Relationship Id="rId{master_rel_id}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml"/>"#
    ));
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships">{rels}</Relationships>"#
    )
}

fn slide_rels(media: &[MediaFile]) -> String {
    let image_rels = media
        .iter()
        .enumerate()
        .map(|(idx, media)| {
            format!(
                r#"<Relationship Id="rId{}" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image{}.{}"/>"#,
                // rId1 is reserved for the required slide layout relationship.
                idx + 2,
                media.media_number,
                media.extension
            )
        })
        .collect::<String>();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/>{image_rels}</Relationships>"#
    )
}

fn emu(value: f64) -> i64 {
    (value * EMU_PER_PT).round() as i64
}

fn color(value: &str) -> String {
    value.trim().trim_start_matches('#').to_ascii_uppercase()
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

const ROOT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/officeDocument" Target="ppt/presentation.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/package/2006/relationships/metadata/core-properties" Target="docProps/core.xml"/><Relationship Id="rId3" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/extended-properties" Target="docProps/app.xml"/></Relationships>"#;
const APP_PROPS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Properties xmlns="http://schemas.openxmlformats.org/officeDocument/2006/extended-properties" xmlns:vt="http://schemas.openxmlformats.org/officeDocument/2006/docPropsVTypes"><Application>md2pptx</Application></Properties>"#;
const CORE_PROPS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><cp:coreProperties xmlns:cp="http://schemas.openxmlformats.org/package/2006/metadata/core-properties" xmlns:dc="http://purl.org/dc/elements/1.1/" xmlns:dcterms="http://purl.org/dc/terms/" xmlns:dcmitype="http://purl.org/dc/dcmitype/" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"><dc:creator>md2pptx</dc:creator><cp:lastModifiedBy>md2pptx</cp:lastModifiedBy></cp:coreProperties>"#;
const SLIDE_MASTER: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldMaster xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main"><p:cSld><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMap bg1="lt1" tx1="dk1" bg2="lt2" tx2="dk2" accent1="accent1" accent2="accent2" accent3="accent3" accent4="accent4" accent5="accent5" accent6="accent6" hlink="hlink" folHlink="folHlink"/><p:sldLayoutIdLst><p:sldLayoutId id="2147483649" r:id="rId1"/></p:sldLayoutIdLst><p:txStyles><p:titleStyle/><p:bodyStyle/><p:otherStyle/></p:txStyles></p:sldMaster>"#;
const SLIDE_MASTER_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml"/><Relationship Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/theme" Target="../theme/theme1.xml"/></Relationships>"#;
const SLIDE_LAYOUT: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><p:sldLayout xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" xmlns:r="http://schemas.openxmlformats.org/officeDocument/2006/relationships" xmlns:p="http://schemas.openxmlformats.org/presentationml/2006/main" type="blank" preserve="1"><p:cSld name="Blank"><p:spTree><p:nvGrpSpPr><p:cNvPr id="1" name=""/><p:cNvGrpSpPr/><p:nvPr/></p:nvGrpSpPr><p:grpSpPr><a:xfrm><a:off x="0" y="0"/><a:ext cx="0" cy="0"/><a:chOff x="0" y="0"/><a:chExt cx="0" cy="0"/></a:xfrm></p:grpSpPr></p:spTree></p:cSld><p:clrMapOvr><a:masterClrMapping/></p:clrMapOvr></p:sldLayout>"#;
const SLIDE_LAYOUT_RELS: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><Relationships xmlns="http://schemas.openxmlformats.org/package/2006/relationships"><Relationship Id="rId1" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="../slideMasters/slideMaster1.xml"/></Relationships>"#;
const THEME: &str = r#"<?xml version="1.0" encoding="UTF-8" standalone="yes"?><a:theme xmlns:a="http://schemas.openxmlformats.org/drawingml/2006/main" name="md2pptx"><a:themeElements><a:clrScheme name="md2pptx"><a:dk1><a:sysClr val="windowText" lastClr="000000"/></a:dk1><a:lt1><a:sysClr val="window" lastClr="FFFFFF"/></a:lt1><a:dk2><a:srgbClr val="1F1F1F"/></a:dk2><a:lt2><a:srgbClr val="F2F2F2"/></a:lt2><a:accent1><a:srgbClr val="4472C4"/></a:accent1><a:accent2><a:srgbClr val="ED7D31"/></a:accent2><a:accent3><a:srgbClr val="A5A5A5"/></a:accent3><a:accent4><a:srgbClr val="FFC000"/></a:accent4><a:accent5><a:srgbClr val="5B9BD5"/></a:accent5><a:accent6><a:srgbClr val="70AD47"/></a:accent6><a:hlink><a:srgbClr val="0563C1"/></a:hlink><a:folHlink><a:srgbClr val="954F72"/></a:folHlink></a:clrScheme><a:fontScheme name="md2pptx"><a:majorFont><a:latin typeface="Arial"/><a:ea typeface=""/><a:cs typeface=""/></a:majorFont><a:minorFont><a:latin typeface="Arial"/><a:ea typeface=""/><a:cs typeface=""/></a:minorFont></a:fontScheme><a:fmtScheme name="md2pptx"><a:fillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:gradFill rotWithShape="1"><a:gsLst><a:gs pos="0"><a:schemeClr val="phClr"/></a:gs><a:gs pos="100000"><a:schemeClr val="phClr"/></a:gs></a:gsLst><a:lin ang="5400000" scaled="0"/></a:gradFill><a:gradFill rotWithShape="1"><a:gsLst><a:gs pos="0"><a:schemeClr val="phClr"/></a:gs><a:gs pos="100000"><a:schemeClr val="phClr"/></a:gs></a:gsLst><a:lin ang="5400000" scaled="0"/></a:gradFill></a:fillStyleLst><a:lnStyleLst><a:ln w="9525" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/></a:ln><a:ln w="25400" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/></a:ln><a:ln w="38100" cap="flat" cmpd="sng" algn="ctr"><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:prstDash val="solid"/></a:ln></a:lnStyleLst><a:effectStyleLst><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle><a:effectStyle><a:effectLst/></a:effectStyle></a:effectStyleLst><a:bgFillStyleLst><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill><a:solidFill><a:schemeClr val="phClr"/></a:solidFill></a:bgFillStyleLst></a:fmtScheme></a:themeElements><a:objectDefaults/><a:extraClrSchemeLst/></a:theme>"#;

#[cfg(test)]
mod tests {
    use std::{
        fs,
        io::Read,
        time::{SystemTime, UNIX_EPOCH},
    };

    use zip::ZipArchive;

    use super::*;
    use crate::{markdown::parse_markdown, style::MathRenderer};

    #[test]
    fn writes_zip_pptx() {
        let out = temp_pptx_path();
        let presentation =
            parse_markdown("# Title\n\nBody", Path::new("."), MathRenderer::Literal).unwrap();
        let warnings = write_pptx(&presentation, &Style::default(), &out).unwrap();
        assert!(warnings.is_empty());
        let bytes = fs::read(&out).unwrap();
        assert!(bytes.starts_with(b"PK"));
        let _ = fs::remove_file(out);
    }

    #[test]
    fn writes_powerpoint_package_scaffolding() {
        let out = temp_pptx_path();
        let presentation = parse_markdown(
            "# First\n\nBody\n\n---\n\n# Second",
            Path::new("."),
            MathRenderer::Literal,
        )
        .unwrap();
        write_pptx(&presentation, &Style::default(), &out).unwrap();

        let file = File::open(&out).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();

        for name in [
            "[Content_Types].xml",
            "_rels/.rels",
            "ppt/presentation.xml",
            "ppt/_rels/presentation.xml.rels",
            "ppt/slideMasters/slideMaster1.xml",
            "ppt/slideMasters/_rels/slideMaster1.xml.rels",
            "ppt/slideLayouts/slideLayout1.xml",
            "ppt/slideLayouts/_rels/slideLayout1.xml.rels",
            "ppt/theme/theme1.xml",
            "ppt/slides/slide1.xml",
            "ppt/slides/_rels/slide1.xml.rels",
            "ppt/slides/slide2.xml",
            "ppt/slides/_rels/slide2.xml.rels",
        ] {
            assert!(archive.by_name(name).is_ok(), "missing PPTX part: {name}");
        }

        assert_contains(
            &mut archive,
            "ppt/presentation.xml",
            r#"<p:sldMasterId id="2147483648" r:id="rId3"/>"#,
        );
        assert_contains(
            &mut archive,
            "ppt/_rels/presentation.xml.rels",
            r#"Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideMaster" Target="slideMasters/slideMaster1.xml""#,
        );
        assert_contains(
            &mut archive,
            "ppt/slides/_rels/slide1.xml.rels",
            r#"Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/slideLayout" Target="../slideLayouts/slideLayout1.xml""#,
        );
        assert_contains(
            &mut archive,
            "[Content_Types].xml",
            r#"PartName="/ppt/theme/theme1.xml" ContentType="application/vnd.openxmlformats-officedocument.theme+xml""#,
        );

        let _ = fs::remove_file(out);
    }

    #[test]
    fn writes_table_cells_as_shapes() {
        let out = temp_pptx_path();
        let presentation = parse_markdown(
            "| Name | Count |\n| :--- | ---: |\n| A | 1 |",
            Path::new("."),
            MathRenderer::Literal,
        )
        .unwrap();
        write_pptx(&presentation, &Style::default(), &out).unwrap();

        let file = File::open(&out).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        assert_contains(&mut archive, "ppt/slides/slide1.xml", "Table Cell");
        assert_contains(
            &mut archive,
            "ppt/slides/slide1.xml",
            r#"<a:pPr algn="r"/>"#,
        );
        assert_contains(
            &mut archive,
            "ppt/slides/slide1.xml",
            r#"<a:solidFill><a:srgbClr val="EEF1F5"/></a:solidFill>"#,
        );

        let _ = fs::remove_file(out);
    }

    #[test]
    fn writes_image_media_relationship_and_aspect_size() {
        let dir = temp_test_dir();
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("chart.png"), png_header_with_dimensions(320, 160)).unwrap();

        let out = dir.join("out.pptx");
        let presentation =
            parse_markdown("![chart](chart.png)", &dir, MathRenderer::Literal).unwrap();
        write_pptx(&presentation, &Style::default(), &out).unwrap();

        let file = File::open(&out).unwrap();
        let mut archive = ZipArchive::new(file).unwrap();
        assert!(archive.by_name("ppt/media/image1.png").is_ok());
        assert_contains(
            &mut archive,
            "[Content_Types].xml",
            r#"<Default Extension="png" ContentType="image/png"/>"#,
        );
        assert_contains(
            &mut archive,
            "ppt/slides/_rels/slide1.xml.rels",
            r#"Id="rId2" Type="http://schemas.openxmlformats.org/officeDocument/2006/relationships/image" Target="../media/image1.png""#,
        );
        assert_contains(&mut archive, "ppt/slides/slide1.xml", r#"r:embed="rId2""#);
        assert_contains(
            &mut archive,
            "ppt/slides/slide1.xml",
            r#"<a:ext cx="11176000" cy="5588000"/>"#,
        );

        let _ = fs::remove_dir_all(dir);
    }

    #[test]
    fn reads_png_dimensions() {
        let bytes = [
            0x89, b'P', b'N', b'G', b'\r', b'\n', 0x1a, b'\n', 0, 0, 0, 13, b'I', b'H', b'D', b'R',
            0, 0, 1, 0x40, 0, 0, 0, 0xf0,
        ];
        let dimensions = png_dimensions(&bytes).unwrap();
        assert_eq!(dimensions.width, 320);
        assert_eq!(dimensions.height, 240);
    }

    #[test]
    fn reads_jpeg_dimensions() {
        let bytes = [
            0xff, 0xd8, 0xff, 0xe0, 0x00, 0x04, 0x00, 0x00, 0xff, 0xc0, 0x00, 0x0b, 0x08, 0x01,
            0x2c, 0x01, 0x90, 0x03, 0x01, 0x11, 0x00,
        ];
        let dimensions = jpeg_dimensions(&bytes).unwrap();
        assert_eq!(dimensions.width, 400);
        assert_eq!(dimensions.height, 300);
    }

    #[test]
    fn image_size_preserves_aspect_ratio() {
        let dimensions = ImageDimensions {
            width: 1600,
            height: 900,
        };
        let (width, height) = image_size(800.0, 600.0, "50%", dimensions);
        assert_eq!(width, 400.0);
        assert_eq!(height, 225.0);
    }

    #[test]
    fn image_size_scales_down_to_available_height() {
        let dimensions = ImageDimensions {
            width: 400,
            height: 800,
        };
        let (width, height) = image_size(600.0, 300.0, "100%", dimensions);
        assert_eq!(width, 150.0);
        assert_eq!(height, 300.0);
    }

    fn temp_pptx_path() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("md2pptx-test-{}-{stamp}.pptx", std::process::id()))
    }

    fn temp_test_dir() -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("md2pptx-test-{}-{stamp}", std::process::id()))
    }

    fn png_header_with_dimensions(width: u32, height: u32) -> Vec<u8> {
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&height.to_be_bytes());
        bytes
    }

    fn assert_contains(archive: &mut ZipArchive<File>, name: &str, needle: &str) {
        let mut file = archive.by_name(name).unwrap();
        let mut content = String::new();
        file.read_to_string(&mut content).unwrap();
        assert!(
            content.contains(needle),
            "{name} did not contain expected XML fragment: {needle}"
        );
    }
}

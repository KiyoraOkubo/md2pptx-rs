use std::path::Path;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::{
    error::{Error, Result},
    model::{Block, Inline, Presentation, Slide, TableAlignment, TableRow},
    style::MathRenderer,
};

struct CollectedInlines {
    inlines: Vec<Inline>,
    images: Vec<(std::path::PathBuf, String)>,
}

pub fn parse_markdown(
    markdown: &str,
    base_dir: &Path,
    math_renderer: MathRenderer,
) -> Result<Presentation> {
    ensure_supported_math_renderer(math_renderer)?;
    let mut slides = Vec::new();
    for raw_slide in split_slides(markdown) {
        if raw_slide.trim().is_empty() {
            continue;
        }
        slides.push(parse_slide(&raw_slide, base_dir, math_renderer)?);
    }
    Ok(Presentation { slides })
}

fn ensure_supported_math_renderer(math_renderer: MathRenderer) -> Result<()> {
    match math_renderer {
        MathRenderer::None | MathRenderer::Literal => Ok(()),
        MathRenderer::External => Err(Error::UnsupportedFeature("math renderer: external")),
        MathRenderer::Katex => Err(Error::UnsupportedFeature("math renderer: katex")),
        MathRenderer::Typst => Err(Error::UnsupportedFeature("math renderer: typst")),
        MathRenderer::Tectonic => Err(Error::UnsupportedFeature("math renderer: tectonic")),
    }
}

fn split_slides(markdown: &str) -> Vec<String> {
    let mut slides = Vec::new();
    let mut current = Vec::new();
    for line in markdown.lines() {
        // Slide boundaries are only horizontal-rule lines by themselves.
        if line.trim() == "---" {
            slides.push(current.join("\n"));
            current.clear();
        } else {
            current.push(line.to_string());
        }
    }
    slides.push(current.join("\n"));
    slides
}

fn parse_slide(markdown: &str, base_dir: &Path, math_renderer: MathRenderer) -> Result<Slide> {
    let mut title = None;
    let mut blocks = Vec::new();

    for segment in split_display_math(markdown, math_renderer)? {
        match segment {
            SlideSegment::Markdown(markdown) => {
                parse_markdown_segment(&markdown, base_dir, math_renderer, &mut title, &mut blocks)?
            }
            SlideSegment::MathBlock(source) => {
                if math_renderer == MathRenderer::None {
                    return Err(Error::UnsupportedFeature("math"));
                }
                blocks.push(Block::MathBlock(source));
            }
        }
    }

    Ok(Slide { title, blocks })
}

enum SlideSegment {
    Markdown(String),
    MathBlock(String),
}

fn split_display_math(markdown: &str, math_renderer: MathRenderer) -> Result<Vec<SlideSegment>> {
    let mut segments = Vec::new();
    let mut markdown_lines = Vec::new();
    let mut math_lines = Vec::new();
    let mut in_math = false;

    for line in markdown.lines() {
        if line.trim() == "$$" {
            if math_renderer == MathRenderer::None {
                return Err(Error::UnsupportedFeature("math"));
            }
            if in_math {
                segments.push(SlideSegment::MathBlock(math_lines.join("\n")));
                math_lines.clear();
                in_math = false;
            } else {
                if !markdown_lines.is_empty() {
                    segments.push(SlideSegment::Markdown(markdown_lines.join("\n")));
                    markdown_lines.clear();
                }
                in_math = true;
            }
        } else if in_math {
            math_lines.push(line.to_string());
        } else {
            markdown_lines.push(line.to_string());
        }
    }

    if in_math {
        return Err(Error::UnsupportedFeature("unterminated math"));
    }
    if !markdown_lines.is_empty() {
        segments.push(SlideSegment::Markdown(markdown_lines.join("\n")));
    }

    Ok(segments)
}

fn parse_markdown_segment(
    markdown: &str,
    base_dir: &Path,
    math_renderer: MathRenderer,
    title: &mut Option<Vec<Inline>>,
    blocks: &mut Vec<Block>,
) -> Result<()> {
    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES;
    let mut parser = Parser::new_ext(markdown, options).peekable();

    while let Some(event) = parser.next() {
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                let collected =
                    collect_inlines(&mut parser, TagEnd::Heading(level), base_dir, math_renderer)?;
                // Only the first H1 is promoted to the title. Other headings
                // stay in the body so the slide keeps all author content.
                if level == HeadingLevel::H1 && title.is_none() {
                    *title = Some(collected.inlines);
                } else {
                    blocks.push(Block::Paragraph(collected.inlines));
                }
            }
            Event::Start(Tag::Paragraph) => {
                let collected =
                    collect_inlines(&mut parser, TagEnd::Paragraph, base_dir, math_renderer)?;
                // pulldown-cmark reports images inside paragraphs. Split them
                // into image blocks while preserving any real paragraph text.
                let has_text = collected.inlines.iter().any(|inline| {
                    !Inline::plain_text(std::slice::from_ref(inline))
                        .trim()
                        .is_empty()
                });
                if has_text {
                    blocks.push(Block::Paragraph(collected.inlines));
                }
                for (path, alt) in collected.images {
                    blocks.push(Block::Image { path, alt });
                }
            }
            Event::Start(Tag::List(start)) => {
                blocks.push(collect_list(
                    &mut parser,
                    start.is_some(),
                    base_dir,
                    math_renderer,
                )?);
            }
            Event::Start(Tag::Table(alignments)) => {
                blocks.push(collect_table(
                    &mut parser,
                    alignments,
                    base_dir,
                    math_renderer,
                )?);
            }
            Event::Start(Tag::BlockQuote) => {
                let collected =
                    collect_inlines(&mut parser, TagEnd::BlockQuote, base_dir, math_renderer)?;
                blocks.push(Block::Quote(collected.inlines));
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                let language = match kind {
                    CodeBlockKind::Fenced(value) => {
                        let lang = value.to_string();
                        if lang.eq_ignore_ascii_case("mermaid") {
                            return Err(Error::UnsupportedFeature("mermaid"));
                        }
                        if lang.eq_ignore_ascii_case("math") && math_renderer == MathRenderer::None
                        {
                            return Err(Error::UnsupportedFeature("math"));
                        }
                        Some(lang)
                    }
                    CodeBlockKind::Indented => None,
                };
                let mut code = String::new();
                for event in parser.by_ref() {
                    match event {
                        Event::Text(value) => code.push_str(&value),
                        Event::End(TagEnd::CodeBlock) => break,
                        _ => {}
                    }
                }
                if language
                    .as_deref()
                    .is_some_and(|lang| lang.eq_ignore_ascii_case("math"))
                {
                    blocks.push(Block::MathBlock(code));
                } else {
                    blocks.push(Block::CodeBlock { language, code });
                }
            }
            Event::Start(Tag::Image {
                dest_url,
                title: alt,
                ..
            }) => {
                let image_path = base_dir.join(dest_url.as_ref());
                blocks.push(Block::Image {
                    path: image_path,
                    alt: alt.to_string(),
                });
                skip_until(&mut parser, TagEnd::Image)?;
            }
            _ => {}
        }
    }

    Ok(())
}

fn collect_table<'a, I>(
    parser: &mut I,
    alignments: Vec<Alignment>,
    base_dir: &Path,
    math_renderer: MathRenderer,
) -> Result<Block>
where
    I: Iterator<Item = Event<'a>>,
{
    let mut rows = Vec::new();

    while let Some(event) = parser.next() {
        match event {
            Event::Start(Tag::TableHead) => {
                rows.push(collect_table_head(parser, base_dir, math_renderer)?);
            }
            Event::Start(Tag::TableRow) => {
                rows.push(collect_table_row(parser, false, base_dir, math_renderer)?);
            }
            Event::End(TagEnd::Table) => break,
            _ => {}
        }
    }

    Ok(Block::Table {
        alignments: alignments.into_iter().map(table_alignment).collect(),
        rows,
    })
}

fn collect_table_head<'a, I>(
    parser: &mut I,
    base_dir: &Path,
    math_renderer: MathRenderer,
) -> Result<TableRow>
where
    I: Iterator<Item = Event<'a>>,
{
    let mut cells = Vec::new();
    while let Some(event) = parser.next() {
        match event {
            Event::Start(Tag::TableCell) => {
                cells.push(
                    collect_inlines(parser, TagEnd::TableCell, base_dir, math_renderer)?.inlines,
                );
            }
            Event::End(TagEnd::TableHead) => break,
            _ => {}
        }
    }
    Ok(TableRow {
        cells,
        is_header: true,
    })
}

fn collect_table_row<'a, I>(
    parser: &mut I,
    is_header: bool,
    base_dir: &Path,
    math_renderer: MathRenderer,
) -> Result<TableRow>
where
    I: Iterator<Item = Event<'a>>,
{
    let mut cells = Vec::new();
    while let Some(event) = parser.next() {
        match event {
            Event::Start(Tag::TableCell) => {
                cells.push(
                    collect_inlines(parser, TagEnd::TableCell, base_dir, math_renderer)?.inlines,
                );
            }
            Event::End(TagEnd::TableRow) => break,
            _ => {}
        }
    }
    Ok(TableRow { cells, is_header })
}

fn table_alignment(alignment: Alignment) -> TableAlignment {
    match alignment {
        Alignment::None => TableAlignment::Default,
        Alignment::Left => TableAlignment::Left,
        Alignment::Center => TableAlignment::Center,
        Alignment::Right => TableAlignment::Right,
    }
}

fn collect_list<'a, I>(
    parser: &mut I,
    ordered: bool,
    base_dir: &Path,
    math_renderer: MathRenderer,
) -> Result<Block>
where
    I: Iterator<Item = Event<'a>>,
{
    let mut items = Vec::new();
    while let Some(event) = parser.next() {
        match event {
            Event::Start(Tag::Item) => {
                items.push(collect_inlines(parser, TagEnd::Item, base_dir, math_renderer)?.inlines);
            }
            Event::End(TagEnd::List(_)) => break,
            _ => {}
        }
    }
    Ok(Block::List { ordered, items })
}

fn collect_inlines<'a, I>(
    parser: &mut I,
    until: TagEnd,
    base_dir: &Path,
    math_renderer: MathRenderer,
) -> Result<CollectedInlines>
where
    I: Iterator<Item = Event<'a>>,
{
    let mut inlines = Vec::new();
    let mut images = Vec::new();
    let mut bold = false;
    let mut italic = false;

    // The IR stores simple styled spans. Nested emphasis is deliberately
    // flattened for now because PPTX run generation is also simple.
    while let Some(event) = parser.next() {
        match event {
            Event::End(end) if end == until => break,
            Event::Start(Tag::Strong) => bold = true,
            Event::End(TagEnd::Strong) => bold = false,
            Event::Start(Tag::Emphasis) => italic = true,
            Event::End(TagEnd::Emphasis) => italic = false,
            Event::Text(value) => {
                push_text_with_inline_math(&mut inlines, &value, bold, italic, math_renderer)?
            }
            Event::Code(value) => inlines.push(Inline::Code(value.to_string())),
            Event::SoftBreak | Event::HardBreak => inlines.push(Inline::Text("\n".into())),
            Event::Start(Tag::Image {
                dest_url,
                title: alt,
                ..
            }) => {
                images.push((base_dir.join(dest_url.as_ref()), alt.to_string()));
                skip_until(parser, TagEnd::Image)?;
            }
            _ => {}
        }
    }

    Ok(CollectedInlines { inlines, images })
}

fn push_text_with_inline_math(
    inlines: &mut Vec<Inline>,
    text: &str,
    bold: bool,
    italic: bool,
    math_renderer: MathRenderer,
) -> Result<()> {
    let mut rest = text;
    while let Some(start) = find_unescaped_dollar(rest) {
        let before = &rest[..start];
        push_styled_text(inlines, before, bold, italic);
        let after_start = &rest[start + 1..];
        if let Some(end) = find_unescaped_dollar(after_start) {
            if math_renderer == MathRenderer::None {
                return Err(Error::UnsupportedFeature("math"));
            }
            inlines.push(Inline::Math(after_start[..end].to_string()));
            rest = &after_start[end + 1..];
        } else {
            push_styled_text(inlines, &rest[start..], bold, italic);
            return Ok(());
        }
    }
    push_styled_text(inlines, rest, bold, italic);
    Ok(())
}

fn push_styled_text(inlines: &mut Vec<Inline>, text: &str, bold: bool, italic: bool) {
    if text.is_empty() {
        return;
    }
    if bold {
        inlines.push(Inline::Bold(text.to_string()));
    } else if italic {
        inlines.push(Inline::Italic(text.to_string()));
    } else {
        inlines.push(Inline::Text(text.to_string()));
    }
}

fn find_unescaped_dollar(text: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'$' {
            let backslash_count = bytes[..index]
                .iter()
                .rev()
                .take_while(|byte| **byte == b'\\')
                .count();
            if backslash_count % 2 == 0 {
                return Some(index);
            }
        }
        index += 1;
    }
    None
}

fn skip_until<'a, I>(parser: &mut I, until: TagEnd) -> Result<()>
where
    I: Iterator<Item = Event<'a>>,
{
    for event in parser.by_ref() {
        if event == Event::End(until) {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_slides_and_extracts_title() {
        let presentation = parse_markdown(
            "# One\n\nBody\n---\n# Two\n\n- A",
            Path::new("."),
            MathRenderer::Literal,
        )
        .unwrap();
        assert_eq!(presentation.slides.len(), 2);
        assert_eq!(
            Inline::plain_text(presentation.slides[0].title.as_ref().unwrap()),
            "One"
        );
    }

    #[test]
    fn rejects_mermaid() {
        let err = parse_markdown(
            "```mermaid\ngraph TD\n```",
            Path::new("."),
            MathRenderer::Literal,
        )
        .unwrap_err();
        assert!(matches!(err, Error::UnsupportedFeature("mermaid")));
    }

    #[test]
    fn rejects_math_when_renderer_is_none() {
        let err = parse_markdown("$$\nx = 1\n$$", Path::new("."), MathRenderer::None).unwrap_err();
        assert!(matches!(err, Error::UnsupportedFeature("math")));
    }

    #[test]
    fn parses_inline_math_as_literal_math() {
        let presentation = parse_markdown(
            "Energy is $E = mc^2$.",
            Path::new("."),
            MathRenderer::Literal,
        )
        .unwrap();
        assert_eq!(
            presentation.slides[0].blocks[0],
            Block::Paragraph(vec![
                Inline::Text("Energy is ".into()),
                Inline::Math("E = mc^2".into()),
                Inline::Text(".".into()),
            ])
        );
    }

    #[test]
    fn parses_display_math_as_literal_math_block() {
        let presentation =
            parse_markdown("$$\nx = 1\n$$", Path::new("."), MathRenderer::Literal).unwrap();
        assert_eq!(
            presentation.slides[0].blocks[0],
            Block::MathBlock("x = 1".into())
        );
    }

    #[test]
    fn parses_fenced_math_as_literal_math_block() {
        let presentation =
            parse_markdown("```math\nx = 1\n```", Path::new("."), MathRenderer::Literal).unwrap();
        assert_eq!(
            presentation.slides[0].blocks[0],
            Block::MathBlock("x = 1\n".into())
        );
    }

    #[test]
    fn parses_markdown_table() {
        let presentation = parse_markdown(
            "| Name | Count |\n| :--- | ---: |\n| A | 1 |\n| B | 2 |",
            Path::new("."),
            MathRenderer::Literal,
        )
        .unwrap();

        assert_eq!(
            presentation.slides[0].blocks[0],
            Block::Table {
                alignments: vec![TableAlignment::Left, TableAlignment::Right],
                rows: vec![
                    TableRow {
                        is_header: true,
                        cells: vec![
                            vec![Inline::Text("Name".into())],
                            vec![Inline::Text("Count".into())],
                        ],
                    },
                    TableRow {
                        is_header: false,
                        cells: vec![
                            vec![Inline::Text("A".into())],
                            vec![Inline::Text("1".into())],
                        ],
                    },
                    TableRow {
                        is_header: false,
                        cells: vec![
                            vec![Inline::Text("B".into())],
                            vec![Inline::Text("2".into())],
                        ],
                    },
                ],
            }
        );
    }
}

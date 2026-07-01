use std::path::Path;

use pulldown_cmark::{Alignment, CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};

use crate::{
    error::{Error, Result},
    model::{
        Block, ColumnBlock, ColumnsBlock, Inline, ListBlock, ListItem, Presentation, Slide,
        TableAlignment, TableRow,
    },
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
    let markdown = markdown.strip_prefix('\u{feff}').unwrap_or(markdown);
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
    parse_source_markdown(
        markdown,
        base_dir,
        math_renderer,
        Some(&mut title),
        &mut blocks,
    )?;

    Ok(Slide { title, blocks })
}

fn parse_source_markdown(
    markdown: &str,
    base_dir: &Path,
    math_renderer: MathRenderer,
    mut title: Option<&mut Option<Vec<Inline>>>,
    blocks: &mut Vec<Block>,
) -> Result<()> {
    for source_block in parse_source_blocks(markdown)? {
        match source_block {
            SourceBlock::Markdown(markdown) => {
                parse_markdown_source(
                    &markdown,
                    base_dir,
                    math_renderer,
                    title.as_deref_mut(),
                    blocks,
                )?;
            }
            SourceBlock::Directive(directive) => {
                blocks.push(convert_directive(directive, base_dir, math_renderer)?);
            }
        }
    }
    Ok(())
}

fn parse_markdown_source(
    markdown: &str,
    base_dir: &Path,
    math_renderer: MathRenderer,
    mut title: Option<&mut Option<Vec<Inline>>>,
    blocks: &mut Vec<Block>,
) -> Result<()> {
    for segment in split_display_math(markdown, math_renderer)? {
        match segment {
            SlideSegment::Markdown(markdown) => parse_markdown_segment(
                &markdown,
                base_dir,
                math_renderer,
                title.as_deref_mut(),
                blocks,
            )?,
            SlideSegment::MathBlock(source) => {
                if math_renderer == MathRenderer::None {
                    return Err(Error::UnsupportedFeature("math"));
                }
                blocks.push(Block::MathBlock(source));
            }
        }
    }
    Ok(())
}

#[derive(Debug, PartialEq)]
enum SourceBlock {
    Markdown(String),
    Directive(DirectiveBlock),
}

#[derive(Debug, PartialEq)]
struct DirectiveBlock {
    name: String,
    attrs: Vec<(String, String)>,
    children: Vec<SourceBlock>,
}

fn parse_source_blocks(markdown: &str) -> Result<Vec<SourceBlock>> {
    let lines = markdown.lines().collect::<Vec<_>>();
    let mut index = 0;
    parse_source_blocks_until(&lines, &mut index, false)
}

fn parse_source_blocks_until(
    lines: &[&str],
    index: &mut usize,
    allow_close: bool,
) -> Result<Vec<SourceBlock>> {
    let mut blocks = Vec::new();
    let mut markdown_lines = Vec::new();
    let mut code_fence: Option<String> = None;

    while *index < lines.len() {
        let line = lines[*index];
        let trimmed = line.trim();

        if let Some(fence) = &code_fence {
            markdown_lines.push(line.to_string());
            if closes_code_fence(trimmed, fence) {
                code_fence = None;
            }
            *index += 1;
            continue;
        }

        if let Some(fence) = opens_code_fence(trimmed) {
            markdown_lines.push(line.to_string());
            code_fence = Some(fence);
            *index += 1;
            continue;
        }

        if trimmed == ":::" {
            if allow_close {
                push_markdown_block(&mut blocks, &mut markdown_lines);
                *index += 1;
                return Ok(blocks);
            }
            return Err(Error::InvalidMarkdown(
                "unexpected directive closing fence".into(),
            ));
        }

        if let Some((name, attrs)) = parse_directive_open(trimmed)? {
            push_markdown_block(&mut blocks, &mut markdown_lines);
            *index += 1;
            let children = parse_source_blocks_until(lines, index, true)?;
            blocks.push(SourceBlock::Directive(DirectiveBlock {
                name,
                attrs,
                children,
            }));
            continue;
        }

        markdown_lines.push(line.to_string());
        *index += 1;
    }

    if allow_close {
        return Err(Error::InvalidMarkdown(
            "unterminated directive block".into(),
        ));
    }

    push_markdown_block(&mut blocks, &mut markdown_lines);
    Ok(blocks)
}

fn push_markdown_block(blocks: &mut Vec<SourceBlock>, markdown_lines: &mut Vec<String>) {
    if markdown_lines.iter().any(|line| !line.trim().is_empty()) {
        blocks.push(SourceBlock::Markdown(markdown_lines.join("\n")));
    }
    markdown_lines.clear();
}

fn opens_code_fence(trimmed: &str) -> Option<String> {
    for marker in ["```", "~~~"] {
        if trimmed.starts_with(marker) {
            let count = trimmed
                .chars()
                .take_while(|character| *character == marker.chars().next().unwrap())
                .count();
            if count >= 3 {
                return Some(marker.chars().next().unwrap().to_string().repeat(count));
            }
        }
    }
    None
}

fn closes_code_fence(trimmed: &str, fence: &str) -> bool {
    trimmed.starts_with(fence)
}

fn parse_directive_open(trimmed: &str) -> Result<Option<(String, Vec<(String, String)>)>> {
    let Some(rest) = trimmed.strip_prefix(":::") else {
        return Ok(None);
    };
    if rest.is_empty() || rest.starts_with(':') || !rest.starts_with(char::is_whitespace) {
        return Ok(None);
    }
    let rest = rest.trim();
    if rest.is_empty() {
        return Ok(None);
    }
    let (name, attr_source) = split_directive_name(rest);
    if !name
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
    {
        return Err(Error::InvalidMarkdown(format!(
            "invalid directive name: {name}"
        )));
    }
    let attrs = parse_directive_attrs(attr_source)?;
    Ok(Some((name.to_string(), attrs)))
}

fn split_directive_name(source: &str) -> (&str, &str) {
    source
        .find(char::is_whitespace)
        .map_or((source, ""), |index| {
            (&source[..index], source[index..].trim())
        })
}

fn parse_directive_attrs(source: &str) -> Result<Vec<(String, String)>> {
    let mut attrs = Vec::new();
    let mut rest = source.trim();

    while !rest.is_empty() {
        let Some(eq_index) = rest.find('=') else {
            return Err(Error::InvalidMarkdown(format!(
                "invalid directive attribute: {rest}"
            )));
        };
        let key = rest[..eq_index].trim();
        if key.is_empty()
            || !key
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'-')
        {
            return Err(Error::InvalidMarkdown(format!(
                "invalid directive attribute: {key}"
            )));
        }
        rest = rest[eq_index + 1..].trim_start();

        let (value, next_rest) = if let Some(quoted) = rest.strip_prefix('"') {
            let Some(end_quote) = quoted.find('"') else {
                return Err(Error::InvalidMarkdown(format!(
                    "unterminated quoted value for directive attribute: {key}"
                )));
            };
            (&quoted[..end_quote], quoted[end_quote + 1..].trim_start())
        } else {
            let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
            (&rest[..end], rest[end..].trim_start())
        };

        attrs.push((key.to_string(), value.to_string()));
        rest = next_rest;
    }

    Ok(attrs)
}

fn convert_directive(
    directive: DirectiveBlock,
    base_dir: &Path,
    math_renderer: MathRenderer,
) -> Result<Block> {
    match directive.name.as_str() {
        "columns" => convert_columns_directive(directive, base_dir, math_renderer),
        "column" => Err(Error::InvalidMarkdown(
            "column directive must be inside columns".into(),
        )),
        _ => Err(Error::InvalidMarkdown(format!(
            "unknown directive: {}",
            directive.name
        ))),
    }
}

fn convert_columns_directive(
    directive: DirectiveBlock,
    base_dir: &Path,
    math_renderer: MathRenderer,
) -> Result<Block> {
    reject_attrs("columns", &directive.attrs)?;
    let mut columns = Vec::new();
    for child in directive.children {
        let SourceBlock::Directive(column) = child else {
            return Err(Error::InvalidMarkdown(
                "columns directive may contain only column directives".into(),
            ));
        };
        if column.name != "column" {
            return Err(Error::InvalidMarkdown(
                "columns directive may contain only column directives".into(),
            ));
        }
        reject_attrs("column", &column.attrs)?;
        let mut blocks = Vec::new();
        for child in column.children {
            match child {
                SourceBlock::Markdown(markdown) => {
                    parse_source_markdown(&markdown, base_dir, math_renderer, None, &mut blocks)?;
                }
                SourceBlock::Directive(nested) if nested.name == "columns" => {
                    return Err(Error::InvalidMarkdown(
                        "nested columns directives are not supported".into(),
                    ));
                }
                SourceBlock::Directive(nested) => {
                    blocks.push(convert_directive(nested, base_dir, math_renderer)?);
                }
            }
        }
        columns.push(ColumnBlock { blocks });
    }

    if columns.len() != 2 {
        return Err(Error::InvalidMarkdown(
            "columns directive must contain exactly two column directives".into(),
        ));
    }

    Ok(Block::Columns(ColumnsBlock { columns }))
}

fn reject_attrs(name: &str, attrs: &[(String, String)]) -> Result<()> {
    if let Some((key, _)) = attrs.first() {
        return Err(Error::InvalidMarkdown(format!(
            "unknown attribute on {name} directive: {key}"
        )));
    }
    Ok(())
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
    mut title: Option<&mut Option<Vec<Inline>>>,
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
                if level == HeadingLevel::H1
                    && title
                        .as_ref()
                        .is_some_and(|current_title| current_title.is_none())
                {
                    *title.as_deref_mut().unwrap() = Some(collected.inlines);
                } else if let Some(level) = body_heading_level(level) {
                    blocks.push(Block::Heading {
                        level,
                        inlines: collected.inlines,
                    });
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
                blocks.push(Block::List(collect_list(
                    &mut parser,
                    start.is_some(),
                    base_dir,
                    math_renderer,
                )?));
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
                let (language, is_mermaid) = match kind {
                    CodeBlockKind::Fenced(value) => {
                        let lang = value.to_string();
                        if lang.eq_ignore_ascii_case("math") && math_renderer == MathRenderer::None
                        {
                            return Err(Error::UnsupportedFeature("math"));
                        }
                        let is_mermaid = lang.eq_ignore_ascii_case("mermaid");
                        (Some(lang), is_mermaid)
                    }
                    CodeBlockKind::Indented => (None, false),
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
                } else if is_mermaid {
                    blocks.push(Block::Mermaid { source: code });
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

fn body_heading_level(level: HeadingLevel) -> Option<u8> {
    match level {
        HeadingLevel::H2 => Some(2),
        HeadingLevel::H3 => Some(3),
        HeadingLevel::H4 => Some(4),
        HeadingLevel::H5 => Some(5),
        HeadingLevel::H6 => Some(6),
        HeadingLevel::H1 => None,
    }
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
) -> Result<ListBlock>
where
    I: Iterator<Item = Event<'a>>,
{
    let mut items = Vec::new();
    while let Some(event) = parser.next() {
        match event {
            Event::Start(Tag::Item) => {
                items.push(collect_list_item(parser, base_dir, math_renderer)?);
            }
            Event::End(TagEnd::List(_)) => break,
            _ => {}
        }
    }
    Ok(ListBlock { ordered, items })
}

fn collect_list_item<'a, I>(
    parser: &mut I,
    base_dir: &Path,
    math_renderer: MathRenderer,
) -> Result<ListItem>
where
    I: Iterator<Item = Event<'a>>,
{
    let mut inlines = Vec::new();
    let mut children = Vec::new();

    while let Some(event) = parser.next() {
        match event {
            Event::End(TagEnd::Item) => break,
            Event::Start(Tag::Paragraph) => {
                let collected =
                    collect_inlines(parser, TagEnd::Paragraph, base_dir, math_renderer)?;
                append_item_inlines(&mut inlines, collected.inlines);
            }
            Event::Start(Tag::List(start)) => {
                children.push(collect_list(
                    parser,
                    start.is_some(),
                    base_dir,
                    math_renderer,
                )?);
            }
            Event::Text(value) => {
                inlines.push(Inline::Text(value.to_string()));
            }
            Event::Code(value) => {
                inlines.push(Inline::Code(value.to_string()));
            }
            Event::SoftBreak | Event::HardBreak => {
                inlines.push(Inline::Text("\n".into()));
            }
            _ => {}
        }
    }

    Ok(ListItem { inlines, children })
}

fn append_item_inlines(target: &mut Vec<Inline>, mut inlines: Vec<Inline>) {
    if !target.is_empty() && !inlines.is_empty() {
        target.push(Inline::Text("\n".into()));
    }
    target.append(&mut inlines);
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
    fn strips_utf8_bom_before_parsing_title() {
        let presentation = parse_markdown(
            "\u{feff}# Title\n\nBody",
            Path::new("."),
            MathRenderer::Literal,
        )
        .unwrap();

        assert_eq!(
            Inline::plain_text(presentation.slides[0].title.as_ref().unwrap()),
            "Title"
        );
        assert_eq!(
            presentation.slides[0].blocks[0],
            Block::Paragraph(vec![Inline::Text("Body".into())])
        );
    }

    #[test]
    fn parses_mermaid_fenced_block() {
        let presentation = parse_markdown(
            "```mermaid\ngraph TD\n```",
            Path::new("."),
            MathRenderer::Literal,
        )
        .unwrap();
        assert_eq!(
            presentation.slides[0].blocks[0],
            Block::Mermaid {
                source: "graph TD\n".into()
            }
        );
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

    #[test]
    fn parses_nested_lists() {
        let presentation = parse_markdown(
            "- Parent\n  1. Child\n     - Grandchild",
            Path::new("."),
            MathRenderer::Literal,
        )
        .unwrap();

        assert_eq!(
            presentation.slides[0].blocks[0],
            Block::List(ListBlock {
                ordered: false,
                items: vec![ListItem {
                    inlines: vec![Inline::Text("Parent".into())],
                    children: vec![ListBlock {
                        ordered: true,
                        items: vec![ListItem {
                            inlines: vec![Inline::Text("Child".into())],
                            children: vec![ListBlock {
                                ordered: false,
                                items: vec![ListItem {
                                    inlines: vec![Inline::Text("Grandchild".into())],
                                    children: vec![],
                                }],
                            }],
                        }],
                    }],
                }],
            })
        );
    }

    #[test]
    fn preserves_inline_code_inside_list_items() {
        let presentation = parse_markdown(
            "- Split with `---` marker",
            Path::new("."),
            MathRenderer::Literal,
        )
        .unwrap();

        assert_eq!(
            presentation.slides[0].blocks[0],
            Block::List(ListBlock {
                ordered: false,
                items: vec![ListItem {
                    inlines: vec![
                        Inline::Text("Split with ".into()),
                        Inline::Code("---".into()),
                        Inline::Text(" marker".into()),
                    ],
                    children: vec![],
                }],
            })
        );
    }

    #[test]
    fn parses_body_headings_with_levels() {
        let presentation = parse_markdown(
            "# Title\n\n## Section\n\n### Detail\n\n# Extra",
            Path::new("."),
            MathRenderer::Literal,
        )
        .unwrap();

        assert_eq!(
            presentation.slides[0].blocks,
            vec![
                Block::Heading {
                    level: 2,
                    inlines: vec![Inline::Text("Section".into())],
                },
                Block::Heading {
                    level: 3,
                    inlines: vec![Inline::Text("Detail".into())],
                },
                Block::Paragraph(vec![Inline::Text("Extra".into())]),
            ]
        );
    }

    #[test]
    fn parses_columns_directive() {
        let presentation = parse_markdown(
            "# Title\n\n::: columns\n::: column\n## Left\n\n- A\n:::\n::: column\nRight text\n:::\n:::",
            Path::new("."),
            MathRenderer::Literal,
        )
        .unwrap();

        assert_eq!(presentation.slides[0].blocks.len(), 1);
        let Block::Columns(columns) = &presentation.slides[0].blocks[0] else {
            panic!("expected columns block");
        };
        assert_eq!(columns.columns.len(), 2);
        assert_eq!(
            columns.columns[0].blocks[0],
            Block::Heading {
                level: 2,
                inlines: vec![Inline::Text("Left".into())],
            }
        );
        assert_eq!(
            columns.columns[1].blocks[0],
            Block::Paragraph(vec![Inline::Text("Right text".into())])
        );
    }

    #[test]
    fn rejects_columns_with_non_column_content() {
        let err = parse_markdown(
            "::: columns\nPlain text\n:::\n",
            Path::new("."),
            MathRenderer::Literal,
        )
        .unwrap_err();

        assert!(err.to_string().contains("columns directive"));
    }

    #[test]
    fn rejects_unterminated_directive() {
        let err = parse_markdown(
            "::: columns\n::: column\nLeft",
            Path::new("."),
            MathRenderer::Literal,
        )
        .unwrap_err();

        assert!(err.to_string().contains("unterminated directive"));
    }

    #[test]
    fn rejects_unknown_directive_attribute_after_parsing_quoted_value() {
        let err = parse_markdown(
            "::: columns label=\"two columns\"\n::: column\nLeft\n:::\n::: column\nRight\n:::\n:::",
            Path::new("."),
            MathRenderer::Literal,
        )
        .unwrap_err();

        assert!(err.to_string().contains("unknown attribute"));
    }
}

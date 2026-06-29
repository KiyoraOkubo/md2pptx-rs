# md2pptx

`md2pptx` is a small Rust CLI that converts Markdown slide decks into a minimal PPTX file.

## Usage

```powershell
cargo run -- examples\sample.md -o examples\sample.pptx --style examples\style.toml
```

The CLI expects:

```text
md2pptx <input.md> -o <output.pptx> [--style <style.toml>]
```

## Markdown Input

Slides are split by a line containing only:

```markdown
---
```

The first level-one heading in each slide becomes the slide title:

```markdown
# Slide Title
```

Supported content:

- Paragraphs
- Unordered lists
- Ordered lists
- Nested lists up to three levels
- Bold
- Italic
- Inline code
- Fenced code blocks
- Inline math as literal text
- Display math as literal text boxes
- Block quotes
- Markdown tables
- PNG, JPG, and JPEG images

Image paths are resolved relative to the Markdown file.

Mermaid blocks are rejected with unsupported-feature errors. Math is rendered as literal source text by default.

## Style TOML

Styles are configured with fixed sections:

```toml
[slide]
size = "16:9"
background = "#ffffff"
padding = 40

[title]
font_family = "Arial"
font_size = 36
color = "#111111"
bold = true
margin_bottom = 24
```

Supported sections:

- `slide`
- `title`
- `heading_2`
- `heading_3`
- `heading_4`
- `heading_5`
- `heading_6`
- `body`
- `list`
- `code_inline`
- `code_block`
- `quote`
- `image`
- `math`

Numeric style values are interpreted as points. `slide.size` currently supports `"16:9"` and `"4:3"`.
Images can be aligned with `image.align = "left"`, `"center"`, or `"right"`.

Math rendering is configured with:

```toml
[math]
renderer = "literal" # none | literal | external | katex | typst | tectonic
```

Only `none` and `literal` are implemented. `literal` emits inline math with inline-code styling and display/fenced math with code-block styling.

## Limitations

- Output is direct minimal Open XML, not based on a PowerPoint template.
- Layout is simple top-to-bottom flow.
- Overflow is reported as a warning, not an error.
- Code blocks do not have syntax highlighting.
- Inline code does not draw a separate background shape yet.
- Tables are rendered as positioned shapes, not PowerPoint-native editable tables.
- Images currently use a simple fixed sizing strategy.
- Animations, transitions, speaker notes, and complex layouts are not implemented.

## Development

```powershell
cargo fmt -- --check
cargo check
cargo test
```

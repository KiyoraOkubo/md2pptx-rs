use std::path::PathBuf;

// Intermediate representation between Markdown parsing and PPTX writing.
// Keeping this small makes unsupported Markdown behavior explicit.
#[derive(Debug, Clone, PartialEq)]
pub struct Presentation {
    pub slides: Vec<Slide>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Slide {
    pub title: Option<Vec<Inline>>,
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Block {
    Paragraph(Vec<Inline>),
    Heading {
        level: u8,
        inlines: Vec<Inline>,
    },
    List(ListBlock),
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    MathBlock(String),
    Table {
        alignments: Vec<TableAlignment>,
        rows: Vec<TableRow>,
    },
    Columns(ColumnsBlock),
    Quote(Vec<Inline>),
    Image {
        path: PathBuf,
        alt: String,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnsBlock {
    pub columns: Vec<ColumnBlock>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ColumnBlock {
    pub blocks: Vec<Block>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListBlock {
    pub ordered: bool,
    pub items: Vec<ListItem>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ListItem {
    pub inlines: Vec<Inline>,
    pub children: Vec<ListBlock>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TableRow {
    pub cells: Vec<Vec<Inline>>,
    pub is_header: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TableAlignment {
    Default,
    Left,
    Center,
    Right,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Inline {
    Text(String),
    Bold(String),
    Italic(String),
    Code(String),
    Math(String),
}

impl Inline {
    pub fn plain_text(inlines: &[Inline]) -> String {
        let mut text = String::new();
        for inline in inlines {
            match inline {
                Inline::Text(value)
                | Inline::Bold(value)
                | Inline::Italic(value)
                | Inline::Code(value)
                | Inline::Math(value) => text.push_str(value),
            }
        }
        text
    }
}

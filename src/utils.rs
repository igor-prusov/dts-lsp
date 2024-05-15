use tower_lsp::lsp_types::{Position, Range, Url};

#[derive(Clone)]
pub struct Symbol {
    pub uri: Url,
    pub range: Range,
}

impl Symbol {
    pub fn new(uri: Url, range: Range) -> Symbol {
        Symbol { uri, range }
    }
}

pub fn convert_range(range: &tree_sitter::Range) -> Range {
    Range::new(
        Position::new(
            u32::try_from(range.start_point.row).unwrap(),
            u32::try_from(range.start_point.column).unwrap(),
        ),
        Position::new(
            u32::try_from(range.end_point.row).unwrap(),
            u32::try_from(range.end_point.column).unwrap(),
        ),
    )
}

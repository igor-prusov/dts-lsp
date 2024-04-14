use tower_lsp::lsp_types::*;

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
            range.start_point.row as u32,
            range.start_point.column as u32,
        ),
        Position::new(range.end_point.row as u32, range.end_point.column as u32),
    )
}

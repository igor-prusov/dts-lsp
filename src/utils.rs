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

pub fn extension_one_of(url: &Url, exts: &[&str]) -> bool {
    let Some(url_ext) = std::path::Path::new(url.path()).extension() else {
        return false;
    };

    for ext in exts {
        if url_ext.eq_ignore_ascii_case(ext) {
            return true;
        }
    }
    false
}

pub fn is_header(uri: &Url) -> bool {
    extension_one_of(uri, &["h"])
}

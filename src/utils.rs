use std::borrow::Cow;
#[cfg(test)]
use std::sync::Mutex;
use tower_lsp::jsonrpc::Error;
use tower_lsp::jsonrpc::Result;
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

pub fn current_url() -> Result<Url> {
    let mut err = Error::internal_error();
    let Ok(dst) = std::env::current_dir() else {
        err.message = Cow::from("Failed to get current dir");
        return Err(err);
    };

    let Some(x) = dst.to_str() else {
        err.message = Cow::from("Failed to get path ");
        return Err(Error::internal_error());
    };

    let Ok(uri) = Url::from_file_path(x) else {
        err.message = Cow::from("Can't convert path {x} to Url");
        return Err(Error::internal_error());
    };
    Ok(uri)
}

// Leak object, but keep reference in static array to avoid valgrind errors
#[cfg(test)]
pub trait Leakable {
    fn leak(self) -> &'static Self
    where
        Self: Sized,
        Self: Sync,
        Self: Send,
    {
        static LEAKED: Mutex<Vec<&(dyn Leakable + Sync + Send)>> = Mutex::new(Vec::new());
        let leak = Box::leak(Box::new(self));
        let mut v = LEAKED.lock().unwrap();
        v.push(leak);
        leak
    }
}

pub fn url_exists(uri: &Url) -> bool {
    uri.to_file_path().map(|x| x.exists()).unwrap_or(false)
}

use crate::logger::Logger;
use crate::utils::convert_range;
use crate::utils::Symbol;
use crate::FileDepot;
use std::collections::HashMap;
use std::collections::HashSet;
use tokio::sync::Mutex;
use tower_lsp::lsp_types::*;
/*
 * 1. Add all references to map Reference (similar to Label) -> Vec[Range],
 * 2. Find references: Look-up label in all connected files
 *
 */

#[derive(Eq, Hash, PartialEq)]
struct Reference {
    uri: Url,
    name: String,
}

impl Reference {
    fn new(uri: &Url, name: &str) -> Reference {
        Reference {
            uri: uri.clone(),
            name: name.to_string(),
        }
    }
}

struct Data {
    reference_to_symbols: HashMap<Reference, Vec<Range>>,
    fd: FileDepot,
    _logger: Logger,
}

impl Data {
    fn new(logger: &Logger, fd: &FileDepot) -> Data {
        Data {
            reference_to_symbols: HashMap::new(),
            fd: fd.clone(),
            _logger: logger.clone(),
        }
    }

    fn add_reference(&mut self, name: &str, uri: &Url, range: tree_sitter::Range) {
        let r = Reference::new(uri, name);
        if let Some(ref mut v) = self.reference_to_symbols.get_mut(&r) {
            // TODO: Keep track of processed files to avoid repeated add_reference calls.
            if !v.contains(&convert_range(&range)) {
                v.push(convert_range(&range));
            }
        } else {
            let v = vec![convert_range(&range)];
            self.reference_to_symbols.insert(r, v);
        }
    }

    async fn find_references(&self, uri: &Url, name: &str) -> Vec<Symbol> {
        let mut to_visit = vec![uri.clone()];
        let mut visited = HashSet::new();
        let mut res = Vec::new();
        let v = self.fd.get_component(uri).await;
        for f in v.iter() {
            if !visited.contains(f) {
                to_visit.push(f.clone())
            }
        }

        while let Some(uri) = to_visit.pop() {
            if let Some(v) = self.reference_to_symbols.get(&Reference::new(&uri, name)) {
                res.extend(v.iter().map(|x| Symbol::new(uri.clone(), *x)));
            }

            visited.insert(uri);
        }
        res
    }
}

pub struct ReferencesDepot {
    data: Mutex<Data>,
    _logger: Logger,
}

impl ReferencesDepot {
    pub fn new(logger: Logger, fd: FileDepot) -> ReferencesDepot {
        ReferencesDepot {
            data: Mutex::new(Data::new(&logger, &fd)),
            _logger: logger,
        }
    }

    pub async fn add_reference(&self, name: &str, uri: &Url, range: tree_sitter::Range) {
        let mut data = self.data.lock().await;
        data.add_reference(name, uri, range);
    }

    pub async fn find_references(&self, uri: &Url, name: &str) -> Vec<Symbol> {
        let data = self.data.lock().await;
        data.find_references(uri, name).await
    }
}

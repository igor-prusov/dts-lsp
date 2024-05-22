use crate::utils::convert_range;
use crate::utils::Symbol;
use crate::FileDepot;
use crate::{info, log_message};
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Mutex;
use tower_lsp::lsp_types::{MessageType, Range, Url};
/*
 * 1. Add all references to map Reference (similar to Label) -> Vec[Range],
 * 2. Find references: Look-up label in all connected files
 *
 */

#[derive(Clone, Eq, Hash, PartialEq)]
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

#[derive(Clone)]
struct Data {
    reference_to_symbols: HashMap<Reference, Vec<Range>>,
    fd: FileDepot,
}

impl Data {
    fn new(fd: &FileDepot) -> Data {
        Data {
            reference_to_symbols: HashMap::new(),
            fd: fd.clone(),
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
        for f in &v {
            if !visited.contains(f) {
                to_visit.push(f.clone());
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

    #[cfg(test)]
    fn size(&self) -> usize {
        self.reference_to_symbols.keys().count()
    }
}

pub struct ReferencesDepot {
    data: Mutex<Data>,
}

impl ReferencesDepot {
    pub fn new(fd: &FileDepot) -> ReferencesDepot {
        ReferencesDepot {
            data: Mutex::new(Data::new(fd)),
        }
    }

    pub async fn add_reference(&self, name: &str, uri: &Url, range: tree_sitter::Range) {
        info!("ReferencesDepot::add_reference()");
        {
            let mut data = self.data.lock().unwrap();
            data.add_reference(name, uri, range);
        }
    }

    pub async fn find_references(&self, uri: &Url, name: &str) -> Vec<Symbol> {
        info!("ReferencesDepot::find_references()");
        {
            let x = self.data.lock().unwrap();
            x.clone()
        }
        .find_references(uri, name)
        .await
    }

    #[cfg(test)]
    pub async fn size(&self) -> usize {
        let data = self.data.lock().unwrap();
        data.size()
    }
}

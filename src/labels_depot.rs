use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use tower_lsp::lsp_types::Range;
use tower_lsp::lsp_types::*;

use crate::Logger;

#[derive(Clone)]
pub struct Symbol {
    pub uri: Url,
    pub range: Range,
}

impl Symbol {
    pub fn new(uri: Url, range: tree_sitter::Range) -> Symbol {
        let range = Range::new(
            Position::new(
                range.start_point.row as u32,
                range.start_point.column as u32,
            ),
            Position::new(range.end_point.row as u32, range.end_point.column as u32),
        );

        Symbol { uri, range }
    }
}

struct Entry {
    labels: HashMap<String, Symbol>,
    files: Vec<Url>,
}

impl Entry {
    fn new() -> Entry {
        Entry {
            labels: HashMap::new(),
            files: Vec::new(),
        }
    }
}

struct Data {
    url_to_labels: HashMap<Url, Arc<Mutex<Entry>>>,
}

impl Data {
    fn new() -> Data {
        Data {
            url_to_labels: HashMap::new(),
        }
    }
    fn add_label(&mut self, label: &str, uri: &Url, range: tree_sitter::Range) {
        let e = match self.url_to_labels.get(uri) {
            Some(x) => x.clone(),
            None => {
                let x = Arc::new(Mutex::new(Entry::new()));
                self.url_to_labels.insert(uri.clone(), x.clone());
                x
            }
        };
        let mut e = e.lock().unwrap();
        e.labels
            .insert(label.to_string(), Symbol::new(uri.clone(), range));
    }

    fn add_include(&mut self, uri: &Url, include_uri: &Url) {
        let e = match self.url_to_labels.get(uri) {
            Some(x) => x.clone(),
            None => {
                let x = Arc::new(Mutex::new(Entry::new()));
                self.url_to_labels.insert(uri.clone(), x.clone());
                x
            }
        };
        let mut e = e.lock().unwrap();
        e.files.push(include_uri.clone())
    }

    fn find_label(&self, uri: &Url, label: &str) -> Option<Symbol> {
        let mut visited = HashSet::new();
        let mut to_visit = Vec::new();

        to_visit.push(uri.clone());

        while let Some(uri) = to_visit.pop() {
            Logger::log(&format!("processing {}", uri));

            if let Some(labels) = self.url_to_labels.get(&uri) {
                let guard = labels.lock().unwrap();
                if let Some(s) = guard.labels.get(label) {
                    return Some(s.clone());
                }

                for f in &guard.files {
                    if !visited.contains(f) {
                        to_visit.push(f.clone())
                    }
                }
            }

            visited.insert(uri);
        }

        None
    }
}

pub struct LabelsDepot {
    data: Mutex<Data>,
}

impl LabelsDepot {
    pub fn new() -> LabelsDepot {
        LabelsDepot {
            data: Mutex::new(Data::new()),
        }
    }

    pub fn add_label(&self, label: &str, uri: &Url, range: tree_sitter::Range) {
        let mut data = self.data.lock().unwrap();
        data.add_label(label, uri, range);
    }

    pub fn add_include(&self, uri: &Url, include_uri: &Url) {
        let mut data = self.data.lock().unwrap();
        data.add_include(uri, include_uri);
    }

    pub fn find_label(&self, uri: &Url, label: &str) -> Option<Symbol> {
        let data = self.data.lock().unwrap();
        data.find_label(uri, label)
    }

    pub fn dump(&self) {
        let data = self.data.lock().unwrap();
        Logger::log("==== LD (labels) ====");
        for (k, v) in &data.url_to_labels {
            for label in v.lock().unwrap().labels.keys() {
                Logger::log(&format!("url: {}: {}", k, label));
            }
        }
        Logger::log("=====================");
        Logger::log("===== LD (files) ====");
        for (k, v) in &data.url_to_labels {
            for f in &v.lock().unwrap().files {
                Logger::log(&format!("url: {}: {}", k, f));
            }
        }
        Logger::log("=====================");
    }
}

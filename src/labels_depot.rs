use crate::utils::convert_range;
use crate::utils::Symbol;
use crate::FileDepot;
use crate::{info, log};
use std::collections::HashMap;
use std::collections::HashSet;
use tokio::sync::Mutex;
use tower_lsp::lsp_types::{MessageType, Range, Url};

#[derive(Eq, Hash, PartialEq)]
struct Label {
    uri: Url,
    name: String,
}

struct Data {
    label_to_symbol: HashMap<Label, Range>,
    fd: FileDepot,
}

impl Data {
    fn new(fd: &FileDepot) -> Data {
        Data {
            label_to_symbol: HashMap::new(),
            fd: fd.clone(),
        }
    }
    fn add_label(&mut self, label: &str, uri: &Url, range: tree_sitter::Range) {
        self.label_to_symbol.insert(
            Label {
                uri: uri.clone(),
                name: label.to_string(),
            },
            convert_range(&range),
        );
    }

    #[cfg(test)]
    fn size(&self) -> usize {
        self.label_to_symbol.keys().count()
    }

    async fn find_label(&self, uri: &Url, label: &str) -> Option<Symbol> {
        let mut visited = HashSet::new();
        let mut to_visit = Vec::new();

        to_visit.push(uri.clone());

        while let Some(uri) = to_visit.pop() {
            info!("processing {uri}");

            if let Some(range) = self.label_to_symbol.get(&Label {
                name: label.to_string(),
                uri: uri.clone(),
            }) {
                let s = Symbol::new(uri, *range);
                return Some(s);
            }

            for f in self.fd.get_neighbours(&uri).await {
                if !visited.contains(&f) {
                    to_visit.push(f);
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
    pub fn new(fd: &FileDepot) -> LabelsDepot {
        LabelsDepot {
            data: Mutex::new(Data::new(fd)),
        }
    }

    pub async fn add_label(&self, label: &str, uri: &Url, range: tree_sitter::Range) {
        let mut data = self.data.lock().await;
        data.add_label(label, uri, range);
    }

    pub async fn find_label(&self, uri: &Url, label: &str) -> Option<Symbol> {
        let data = self.data.lock().await;
        data.find_label(uri, label).await
    }

    #[cfg(test)]
    pub async fn size(&self) -> usize {
        let data = self.data.lock().await;
        data.size()
    }

    pub async fn dump(&self) {
        let data = self.data.lock().await;
        info!("====== (labels) ======");
        for k in data.label_to_symbol.keys() {
            info!("url: {}: {}", k.uri, k.name);
        }
        info!("======================");
    }
}

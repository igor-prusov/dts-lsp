use crate::utils::convert_range;
use crate::utils::Symbol;
use crate::FileDepot;
use crate::{info, log_message};
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Mutex;
use tower_lsp::lsp_types::{MessageType, Range, Url};

#[derive(Clone, Eq, Hash, PartialEq)]
struct Label {
    uri: Url,
    name: String,
}

#[derive(Clone)]
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

    async fn find_label(&self, uri: &Url, label: &str) -> Vec<Symbol> {
        let mut visited = HashSet::new();
        let mut to_visit = vec![uri.clone()];
        let mut res = Vec::new();

        self.fd.dump().await;
        let v = self.fd.get_component(uri).await;
        for f in &v {
            if !visited.contains(f) {
                to_visit.push(f.clone());
            }
        }

        while let Some(uri) = to_visit.pop() {
            info!("processing {uri}");

            if let Some(range) = self.label_to_symbol.get(&Label {
                name: label.to_string(),
                uri: uri.clone(),
            }) {
                res.push(Symbol::new(uri.clone(), *range));
            }

            visited.insert(uri);
        }

        res
    }

    fn invalidate(&mut self, uri: &Url) {
        let mut v = Vec::new();

        for k in self.label_to_symbol.keys() {
            if k.uri == *uri {
                v.push(k.clone());
            }
        }

        for label in v {
            self.label_to_symbol.remove(&label);
        }
    }

    async fn dump(&self) {
        info!("====== (labels) ======");
        for k in self.label_to_symbol.keys() {
            info!("url: {}: {}", k.uri, k.name);
        }
        info!("======================");
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
        info!("LabelsDepot::add_label()");
        let mut data = self.data.lock().unwrap();
        data.add_label(label, uri, range);
    }

    pub async fn find_label(&self, uri: &Url, label: &str) -> Vec<Symbol> {
        info!("LabelsDepot::find_label()");
        {
            let data = self.data.lock().unwrap();
            data.clone()
        }
        .find_label(uri, label)
        .await
    }

    pub async fn dump(&self) {
        info!("LabelsDepot::dump()");
        {
            let lock = self.data.lock().unwrap();
            lock.clone()
        }
        .dump()
        .await;
    }

    pub async fn invalidate(&self, uri: &Url) {
        info!("LabelsDepot::invalidate()");
        let mut data = self.data.lock().unwrap();
        data.invalidate(uri);
    }

    #[cfg(test)]
    pub async fn size(&self) -> usize {
        let data = self.data.lock().unwrap();
        data.size()
    }
}

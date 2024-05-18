use crate::logger::Logger;
use crate::utils::convert_range;
use crate::utils::Symbol;
use crate::FileDepot;
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
    logger: Logger,
}

impl Data {
    fn new(logger: &Logger, fd: &FileDepot) -> Data {
        Data {
            label_to_symbol: HashMap::new(),
            logger: logger.clone(),
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
            self.logger
                .log_message(MessageType::INFO, &format!("processing {uri}"))
                .await;

            if let Some(range) = self.label_to_symbol.get(&Label {
                name: label.to_string(),
                uri: uri.clone(),
            }) {
                let s = Symbol::new(uri, *range);
                return Some(s);
            }

            if let Some(x) = self.fd.get_neighbours(&uri).await {
                for f in x.lock().await.iter() {
                    if !visited.contains(f) {
                        to_visit.push(f.clone());
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
    logger: Logger,
}

impl LabelsDepot {
    pub fn new(logger: Logger, fd: &FileDepot) -> LabelsDepot {
        LabelsDepot {
            data: Mutex::new(Data::new(&logger, fd)),
            logger,
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
        self.logger
            .log_message(MessageType::INFO, "====== (labels) ======")
            .await;
        for k in data.label_to_symbol.keys() {
            self.logger
                .log_message(MessageType::INFO, &format!("url: {}: {}", k.uri, k.name))
                .await;
        }
        self.logger
            .log_message(MessageType::INFO, "======================")
            .await;
    }
}

use crate::utils::convert_range;
use crate::utils::Symbol;
use crate::FileDepot;
use std::collections::HashMap;
use std::collections::HashSet;
use tokio::sync::Mutex;
use tower_lsp::lsp_types::Range;
use tower_lsp::lsp_types::*;
use tower_lsp::Client;

#[derive(Eq, Hash, PartialEq)]
struct Label {
    uri: Url,
    name: String,
}

struct Data {
    label_to_symbol: HashMap<Label, Range>,
    fd: FileDepot,
    client: Client,
}

impl Data {
    fn new(client: &Client, fd: &FileDepot) -> Data {
        Data {
            label_to_symbol: HashMap::new(),
            client: client.clone(),
            fd: fd.clone(),
        }
    }
    async fn add_label(&mut self, label: &str, uri: &Url, range: tree_sitter::Range) {
        self.label_to_symbol.insert(
            Label {
                uri: uri.clone(),
                name: label.to_string(),
            },
            convert_range(&range),
        );
    }

    async fn find_label(&self, uri: &Url, label: &str) -> Option<Symbol> {
        let mut visited = HashSet::new();
        let mut to_visit = Vec::new();

        to_visit.push(uri.clone());

        while let Some(uri) = to_visit.pop() {
            self.client
                .log_message(MessageType::INFO, &format!("processing {}", uri))
                .await;

            if let Some(range) = self.label_to_symbol.get(&Label {
                name: label.to_string(),
                uri: uri.clone(),
            }) {
                //let range = convert_range(&range);
                let s = Symbol::new(uri, *range);
                return Some(s);
            }

            if let Some(x) = self.fd.get_neighbours(&uri).await {
                for f in x.lock().await.iter() {
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
    client: Client,
}

impl LabelsDepot {
    pub fn new(client: Client, fd: FileDepot) -> LabelsDepot {
        LabelsDepot {
            data: Mutex::new(Data::new(&client, &fd)),
            client,
        }
    }

    pub async fn add_label(&self, label: &str, uri: &Url, range: tree_sitter::Range) {
        let mut data = self.data.lock().await;
        data.add_label(label, uri, range).await;
    }

    pub async fn find_label(&self, uri: &Url, label: &str) -> Option<Symbol> {
        let data = self.data.lock().await;
        data.find_label(uri, label).await
    }

    pub async fn dump(&self) {
        let data = self.data.lock().await;
        self.client
            .log_message(MessageType::INFO, "====== (labels) ======")
            .await;
        for k in data.label_to_symbol.keys() {
            self.client
                .log_message(MessageType::INFO, &format!("url: {}: {}", k.uri, k.name))
                .await;
        }
        self.client
            .log_message(MessageType::INFO, "======================")
            .await;
    }
}

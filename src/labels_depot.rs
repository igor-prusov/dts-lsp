use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::lsp_types::Range;
use tower_lsp::lsp_types::*;
use tower_lsp::Client;

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
    client: Client,
}

impl Data {
    fn new(client: &Client) -> Data {
        Data {
            url_to_labels: HashMap::new(),
            client: client.clone(),
        }
    }
    async fn add_label(&mut self, label: &str, uri: &Url, range: tree_sitter::Range) {
        let e = match self.url_to_labels.get(uri) {
            Some(x) => x.clone(),
            None => {
                let x = Arc::new(Mutex::new(Entry::new()));
                self.url_to_labels.insert(uri.clone(), x.clone());
                x
            }
        };
        let mut e = e.lock().await;
        e.labels
            .insert(label.to_string(), Symbol::new(uri.clone(), range));
    }

    async fn add_include(&mut self, uri: &Url, include_uri: &Url) {
        let e = match self.url_to_labels.get(uri) {
            Some(x) => x.clone(),
            None => {
                let x = Arc::new(Mutex::new(Entry::new()));
                self.url_to_labels.insert(uri.clone(), x.clone());
                x
            }
        };
        let mut e = e.lock().await;
        e.files.push(include_uri.clone())
    }

    async fn find_label(&self, uri: &Url, label: &str) -> Option<Symbol> {
        let mut visited = HashSet::new();
        let mut to_visit = Vec::new();

        to_visit.push(uri.clone());

        while let Some(uri) = to_visit.pop() {
            self.client
                .log_message(MessageType::INFO, &format!("processing {}", uri))
                .await;

            if let Some(labels) = self.url_to_labels.get(&uri) {
                let guard = labels.lock().await;
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
    client: Client,
}

impl LabelsDepot {
    pub fn new(client: Client) -> LabelsDepot {
        LabelsDepot {
            data: Mutex::new(Data::new(&client)),
            client,
        }
    }

    pub async fn add_label(&self, label: &str, uri: &Url, range: tree_sitter::Range) {
        let mut data = self.data.lock().await;
        data.add_label(label, uri, range).await;
    }

    pub async fn add_include(&self, uri: &Url, include_uri: &Url) {
        let mut data = self.data.lock().await;
        data.add_include(uri, include_uri).await;
        data.add_include(include_uri, uri).await;
    }

    pub async fn find_label(&self, uri: &Url, label: &str) -> Option<Symbol> {
        let data = self.data.lock().await;
        data.find_label(uri, label).await
    }

    pub async fn dump(&self) {
        let data = self.data.lock().await;
        self.client
            .log_message(MessageType::INFO, "==== LD (labels) ====")
            .await;
        for (k, v) in &data.url_to_labels {
            for label in v.lock().await.labels.keys() {
                self.client
                    .log_message(MessageType::INFO, &format!("url: {}: {}", k, label))
                    .await;
            }
        }
        self.client
            .log_message(MessageType::INFO, "=====================")
            .await;
        self.client
            .log_message(MessageType::INFO, "===== LD (files) ====")
            .await;
        for (k, v) in &data.url_to_labels {
            for f in &v.lock().await.files {
                self.client
                    .log_message(MessageType::INFO, &format!("url: {}: {}", k, f))
                    .await;
            }
        }
        self.client
            .log_message(MessageType::INFO, "=====================")
            .await;
    }
}

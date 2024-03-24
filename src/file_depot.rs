use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use tokio::sync::Mutex;
use tower_lsp::lsp_types::*;
use tower_lsp::Client;

struct Data {
    url_to_text: HashMap<Url, String>,
    client: Client,
}

impl Data {
    fn new(client: Client) -> Data {
        Data {
            url_to_text: HashMap::new(),
            client,
        }
    }

    fn insert(&mut self, uri: &Url, text: String) {
        if self.url_to_text.get(uri).is_some() {
            return;
        }

        self.url_to_text.insert(uri.clone(), text);
    }

    async fn dump(&self) {
        self.client
            .log_message(MessageType::INFO, "===FILES===")
            .await;
        for uri in self.url_to_text.keys() {
            let mut file = File::open(uri.path()).unwrap();
            let mut s = String::new();
            file.read_to_string(&mut s).unwrap();

            self.client
                .log_message(MessageType::INFO, &format!("{}", uri))
                .await;
        }
        self.client
            .log_message(MessageType::INFO, "==========")
            .await;
    }

    fn get_text(&self, uri: &Url) -> Option<String> {
        self.url_to_text.get(uri).cloned()
    }
}

pub struct FileDepot {
    data: Mutex<Data>,
}

impl FileDepot {
    pub fn new(client: Client) -> FileDepot {
        FileDepot {
            data: Mutex::new(Data::new(client)),
        }
    }

    pub async fn insert(&self, uri: &Url, text: String) {
        let mut data = self.data.lock().await;
        data.insert(uri, text)
    }

    pub async fn dump(&self) {
        self.data.lock().await.dump().await;
    }

    pub async fn get_text(&self, uri: &Url) -> Option<String> {
        self.data.lock().await.get_text(uri)
    }
}

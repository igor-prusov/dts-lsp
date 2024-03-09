use std::collections::HashMap;
use std::sync::Mutex;
use tower_lsp::lsp_types::Url;

use crate::Logger;

struct Data {
    next_id: u32,
    id_to_url: HashMap<u32, Url>,
    url_to_id: HashMap<Url, u32>,
    url_to_text: HashMap<Url, Option<String>>,
}

impl Data {
    fn new() -> Data {
        Data {
            next_id: 0,
            id_to_url: HashMap::new(),
            url_to_id: HashMap::new(),
            url_to_text: HashMap::new(),
        }
    }

    fn insert(&mut self, uri: &Url, text: Option<String>) -> u32 {
        match self.url_to_text.get(uri) {
            Some(Some(_)) => return *self.url_to_id.get(uri).unwrap(),
            Some(None) => {
                self.url_to_text.insert(uri.clone(), text.clone());
                return *self.url_to_id.get(uri).unwrap();
            }
            None => (),
        }

        let id = self.next_id;
        self.next_id += 1;

        self.id_to_url.insert(id, uri.clone());
        self.url_to_id.insert(uri.clone(), id);
        self.url_to_text.insert(uri.clone(), text);

        id
    }

    fn dump(&self) {
        Logger::log(&format!("===DUMP==="));
        for (k, v) in &self.id_to_url {
            let has_text = match self.url_to_text.get(v) {
                Some(Some(_)) => "+",
                _ => "-",
            };
            Logger::log(&format!("{} -> {} [{}]", k, v, has_text));
        }
        Logger::log(&format!("=========="));
    }

    fn get_text(&self, uri: &Url) -> Option<String> {
        self.url_to_text.get(uri)?.clone()
    }
}

pub struct FileDepot {
    data: Mutex<Data>,
}

impl FileDepot {
    pub fn new() -> FileDepot {
        FileDepot {
            data: Mutex::new(Data::new()),
        }
    }

    pub fn insert(&self, uri: &Url, text: Option<String>) -> u32 {
        let mut data = self.data.lock().unwrap();
        data.insert(uri, text)
    }

    pub fn dump(&self) {
        self.data.lock().unwrap().dump();
    }

    pub fn get_text(&self, uri: &Url) -> Option<String> {
        self.data.lock().unwrap().get_text(uri)
    }
}

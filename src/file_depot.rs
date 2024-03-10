use std::collections::HashMap;
use std::fs::File;
use std::io::prelude::*;
use std::sync::Mutex;
use tower_lsp::lsp_types::Url;

use crate::Logger;

struct Data {
    url_to_text: HashMap<Url, String>,
}

impl Data {
    fn new() -> Data {
        Data {
            url_to_text: HashMap::new(),
        }
    }

    fn insert(&mut self, uri: &Url, text: String) {
        match self.url_to_text.get(uri) {
            Some(_) => return,
            None => (),
        }

        self.url_to_text.insert(uri.clone(), text);
    }

    fn dump(&self) {
        Logger::log(&format!("===DUMP==="));
        for uri in self.url_to_text.keys() {
            let mut file = File::open(uri.path()).unwrap();
            let mut s = String::new();
            file.read_to_string(&mut s).unwrap();

            Logger::log(&format!("{}", uri));
        }
        Logger::log(&format!("=========="));
    }

    fn get_text(&self, uri: &Url) -> Option<String> {
        self.url_to_text.get(uri).cloned()
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

    pub fn insert(&self, uri: &Url, text: String) {
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

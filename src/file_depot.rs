use crate::{info, log_message};
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use tower_lsp::lsp_types::{MessageType, Url};

#[derive(Default, Clone)]
struct FileEntry {
    text: Option<String>,
    includes: Vec<Url>,
    included_by: Vec<Url>,
}

#[derive(Clone)]
struct Data {
    entries: HashMap<Url, FileEntry>,
}

#[derive(PartialEq)]
pub enum InsertResult {
    Ok,
    Exists,
    Modified,
}

impl Data {
    fn new() -> Data {
        Data {
            entries: HashMap::new(),
        }
    }

    fn insert(&mut self, uri: &Url, text: &str) -> InsertResult {
        let e = self.entries.entry(uri.clone()).or_default();

        match &e.text {
            None => {
                e.text = Some(text.to_string());
                InsertResult::Ok
            }
            Some(x) if x == text => InsertResult::Exists,
            Some(_) => {
                e.text = Some(text.to_string());
                InsertResult::Modified
            }
        }
    }

    fn add_include(&mut self, uri: &Url, include_uri: &Url) {
        let e = self.entries.entry(uri.clone()).or_default();
        e.includes.push(include_uri.clone());

        let e = self.entries.entry(include_uri.clone()).or_default();
        e.included_by.push(uri.clone());
    }

    //TODO: -> HashSet<Url> or Make sure no repetition
    fn get_component(&self, uri: &Url) -> Vec<Url> {
        // Process includes
        let mut to_visit = vec![uri.clone()];
        let mut visited = HashSet::new();
        let mut res: Vec<Url> = Vec::new();
        while let Some(uri) = to_visit.pop() {
            if let Some(e) = self.entries.get(&uri) {
                for f in &e.includes {
                    if !visited.contains(f) {
                        to_visit.push(f.clone());
                        res.push(f.clone());
                        visited.insert(uri.clone());
                    }
                }
            }

            visited.insert(uri);
        }

        // Process included by
        let mut to_visit = Vec::new();
        if let Some(e) = self.entries.get(uri) {
            for f in &e.included_by {
                if !visited.contains(f) {
                    to_visit.push(f.clone());
                    visited.insert(f.clone());
                    res.push(f.clone());
                }
            }
        }

        while let Some(uri) = to_visit.pop() {
            if let Some(e) = self.entries.get(&uri) {
                for f in e.includes.iter().chain(e.included_by.iter()) {
                    if !visited.contains(f) {
                        to_visit.push(f.clone());
                        visited.insert(f.clone());
                        res.push(f.clone());
                    }
                }
            }
            visited.insert(uri);
        }
        res
    }

    fn get_neighbours(&self, uri: &Url) -> Vec<Url> {
        let mut res = Vec::new();

        if let Some(x) = self.entries.get(uri) {
            res.extend_from_slice(&x.includes);
            res.extend_from_slice(&x.included_by);
        }

        res
    }

    async fn dump(&self) {
        info!("===FILES===");
        for (k, v) in &self.entries {
            info!("{k}, hasText: {}", v.text.is_some());
        }
        info!("=INCLUDES=");
        for (k, v) in &self.entries {
            for f in v.includes.iter().chain(v.included_by.iter()) {
                info!("url: {k}: {f}");
            }
        }
        info!("==========");
    }

    fn exist(&self, uri: &Url) -> bool {
        self.entries.contains_key(uri)
    }

    fn get_text(&self, uri: &Url) -> Option<String> {
        self.entries.get(uri).and_then(|x| x.text.clone())
    }

    #[cfg(test)]
    fn size(&self) -> usize {
        self.entries.keys().count()
    }

    #[cfg(test)]
    fn n_with_text(&self) -> usize {
        self.entries
            .iter()
            .map(|x| x.1.text.is_some() as usize)
            .sum()
    }
}

#[derive(Clone)]
pub struct FileDepot {
    data: Arc<Mutex<Data>>,
}

impl FileDepot {
    pub fn new() -> FileDepot {
        FileDepot {
            data: Arc::new(Mutex::new(Data::new())),
        }
    }

    pub async fn insert(&self, uri: &Url, text: String) -> InsertResult {
        info!("FileDepot::insert({uri})");
        let mut data = self.data.lock().unwrap();
        data.insert(uri, &text)
    }

    pub async fn dump(&self) {
        info!("FileDepot::dump()");
        {
            let lock = self.data.lock().unwrap();
            lock.clone()
        }
        .dump()
        .await;
    }

    pub async fn get_text(&self, uri: &Url) -> Option<String> {
        info!("FileDepot::get_text()");
        self.data.lock().unwrap().get_text(uri)
    }

    pub async fn exist(&self, uri: &Url) -> bool {
        info!("FileDepot::exist()");
        self.data.lock().unwrap().exist(uri)
    }

    pub async fn add_include(&self, uri: &Url, include_uri: &Url) {
        info!("FileDepot::add_include()");
        self.data.lock().unwrap().add_include(uri, include_uri);
    }

    pub async fn get_neighbours(&self, uri: &Url) -> Vec<Url> {
        info!("FileDepot::get_neighbours()");
        self.data.lock().unwrap().get_neighbours(uri)
    }

    pub async fn get_component(&self, uri: &Url) -> Vec<Url> {
        info!("FileDepot::get_component()");
        self.data.lock().unwrap().get_component(uri)
    }

    #[cfg(test)]
    pub async fn size(&self) -> usize {
        self.data.lock().unwrap().size()
    }

    #[cfg(test)]
    pub async fn n_with_text(&self) -> usize {
        self.data.lock().unwrap().n_with_text()
    }
}

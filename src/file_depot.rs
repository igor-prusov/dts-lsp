use crate::{info, log};
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_lsp::lsp_types::{MessageType, Url};

#[allow(clippy::struct_field_names)]
struct Data {
    url_to_text: HashMap<Url, String>,
    url_includes: HashMap<Url, Arc<Mutex<Vec<Url>>>>,
    url_included_by: HashMap<Url, Arc<Mutex<Vec<Url>>>>,
}

impl Data {
    fn new() -> Data {
        Data {
            url_to_text: HashMap::new(),
            url_includes: HashMap::new(),
            url_included_by: HashMap::new(),
        }
    }

    fn insert(&mut self, uri: &Url, text: String) {
        if self.url_to_text.contains_key(uri) {
            return;
        }

        self.url_to_text.insert(uri.clone(), text);
    }

    async fn add_include(&mut self, uri: &Url, include_uri: &Url) {
        let e = self.url_includes.entry(uri.clone()).or_default();
        {
            let mut e = e.lock().await;
            e.push(include_uri.clone());
        }

        let e = self.url_included_by.entry(include_uri.clone()).or_default();
        {
            let mut e = e.lock().await;
            e.push(uri.clone());
        }
    }

    //TODO: -> HashSet<Url> or Make sure no repetition
    async fn get_component(&self, uri: &Url) -> Vec<Url> {
        // Process includes
        let mut to_visit = vec![uri.clone()];
        let mut visited = HashSet::new();
        let mut res: Vec<Url> = Vec::new();
        while let Some(uri) = to_visit.pop() {
            if let Some(e) = self.url_includes.get(&uri) {
                let x = e.lock().await.clone();
                for f in &x {
                    if !visited.contains(f) {
                        to_visit.push(f.clone());
                        res.push(f.clone());
                        visited.insert(f.clone());
                    }
                }
            }
            visited.insert(uri);
        }

        // Process included by
        let mut to_visit = Vec::new();
        //let mut visited = HashSet::new();
        if let Some(e) = self.url_included_by.get(uri) {
            let x = e.lock().await.clone();
            for f in &x {
                if !visited.contains(f) {
                    to_visit.push(f.clone());
                    visited.insert(f.clone());
                    res.push(f.clone());
                }
            }
        }
        while let Some(uri) = to_visit.pop() {
            if let Some(e) = self.url_includes.get(&uri) {
                let x = e.lock().await.clone();
                for f in &x {
                    if !visited.contains(f) {
                        to_visit.push(f.clone());
                        visited.insert(f.clone());
                        res.push(f.clone());
                    }
                }
            }
            if let Some(e) = self.url_included_by.get(&uri) {
                let x = e.lock().await.clone();
                for f in &x {
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

    async fn get_neighbours(&self, uri: &Url) -> Vec<Url> {
        let mut res = Vec::new();

        if let Some(x) = self.url_includes.get(uri) {
            res.extend_from_slice(&x.lock().await);
        }

        if let Some(x) = self.url_included_by.get(uri) {
            res.extend_from_slice(&x.lock().await);
        }

        res
    }

    async fn dump(&self) {
        info!("===FILES===");
        for uri in self.url_to_text.keys() {
            info!("{uri}");
        }
        info!("=INCLUDES=");
        for (k, v) in &self.url_includes {
            for f in v.lock().await.iter() {
                info!("url: {k}: {f}");
            }
        }
        for (k, v) in &self.url_included_by {
            for f in v.lock().await.iter() {
                info!("url: {k}: {f}");
            }
        }
        info!("==========");
    }

    fn exist(&self, uri: &Url) -> bool {
        self.url_to_text.contains_key(uri)
    }

    fn get_text(&self, uri: &Url) -> Option<String> {
        self.url_to_text.get(uri).cloned()
    }

    #[cfg(test)]
    fn size(&self) -> usize {
        self.url_to_text.keys().count()
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

    pub async fn insert(&self, uri: &Url, text: String) {
        let mut data = self.data.lock().await;
        data.insert(uri, text);
    }

    pub async fn dump(&self) {
        self.data.lock().await.dump().await;
    }

    pub async fn get_text(&self, uri: &Url) -> Option<String> {
        self.data.lock().await.get_text(uri)
    }

    pub async fn exist(&self, uri: &Url) -> bool {
        self.data.lock().await.exist(uri)
    }

    pub async fn add_include(&self, uri: &Url, include_uri: &Url) {
        self.data.lock().await.add_include(uri, include_uri).await;
        //self.data.lock().await.add_include(include_uri, uri).await;
    }

    pub async fn get_neighbours(&self, uri: &Url) -> Vec<Url> {
        self.data.lock().await.get_neighbours(uri).await
    }

    pub async fn get_component(&self, uri: &Url) -> Vec<Url> {
        self.data.lock().await.get_component(uri).await
    }

    #[cfg(test)]
    pub async fn size(&self) -> usize {
        self.data.lock().await.size()
    }
}

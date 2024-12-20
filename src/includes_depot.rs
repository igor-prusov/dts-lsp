use crate::file_depot::FileDepot;
use crate::utils::Symbol;
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::{Arc, Mutex};
use tower_lsp::lsp_types::{Range, Url};

#[cfg(test)]
use crate::{info, log_message};
#[cfg(test)]
use tower_lsp::lsp_types::MessageType;

#[derive(Eq, Hash, PartialEq, Clone)]
struct Define {
    uri: Url,
    name: String,
}

struct Data {
    define_to_symbol: HashMap<Define, (Range, String)>,
    fd: FileDepot,
}

impl Data {
    fn new(fd: &FileDepot) -> Data {
        Data {
            define_to_symbol: HashMap::new(),
            fd: fd.clone(),
        }
    }

    fn add_define(&mut self, name: &str, uri: &Url, range: Range, value: &str) {
        self.define_to_symbol.insert(
            Define {
                uri: uri.clone(),
                name: name.to_string(),
            },
            (range, value.to_string()),
        );
    }

    fn find_define(&self, uri: &Url, name: &str) -> Option<Symbol> {
        let mut visited = HashSet::new();
        let mut to_visit = vec![uri.clone()];

        let v = self.fd.get_component(uri);
        for f in &v {
            if !visited.contains(f) {
                to_visit.push(f.clone());
            }
        }

        while let Some(uri) = to_visit.pop() {
            if let Some(x) = self.define_to_symbol.get(&Define {
                name: name.to_string(),
                uri: uri.clone(),
            }) {
                return Some(Symbol::new(uri.clone(), x.0));
            }
            visited.insert(uri);
        }

        None
    }

    fn invalidate(&mut self, uri: &Url) {
        let mut v = Vec::new();

        for k in self.define_to_symbol.keys() {
            if k.uri == *uri {
                v.push(k.clone());
            }
        }

        for key in v {
            self.define_to_symbol.remove(&key);
        }
    }

    #[cfg(test)]
    fn dump(&self) {
        info!("====== (defines) ======");
        for (k, v) in &self.define_to_symbol {
            info!("url: {}: {}", k.uri, v.1);
        }
        info!("======================");
    }
}

#[derive(Clone)]
pub struct IncludesDepot {
    data: Arc<Mutex<Data>>,
}

impl IncludesDepot {
    pub fn new(fd: &FileDepot) -> IncludesDepot {
        IncludesDepot {
            data: Arc::new(Mutex::new(Data::new(fd))),
        }
    }

    pub fn add_define(&self, name: &str, uri: &Url, range: Range, value: &str) {
        self.data
            .lock()
            .unwrap()
            .add_define(name, uri, range, value);
    }

    pub fn find_define(&self, uri: &Url, name: &str) -> Option<Symbol> {
        self.data.lock().unwrap().find_define(uri, name)
    }

    #[cfg(test)]
    pub fn dump(&self) {
        self.data.lock().unwrap().dump();
    }

    pub fn invalidate(&self, uri: &Url) {
        self.data.lock().unwrap().invalidate(uri);
    }
}

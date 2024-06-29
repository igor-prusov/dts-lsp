use crate::utils::Symbol;
use crate::FileDepot;
use crate::{info, log_message};
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Mutex;
use tower_lsp::lsp_types::{MessageType, Range, Url};

#[derive(Eq, Hash, PartialEq)]
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
        info!("find_define: {uri}, {name}");

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
            info!("... not found in {uri}");
            visited.insert(uri);
        }

        None
    }

    fn dump(&self) {
        info!("====== (defines) ======");
        for (k, v) in &self.define_to_symbol {
            info!("url: {}: {}", k.uri, v.1);
        }
        info!("======================");
    }
}

pub struct IncludesDepot {
    data: Mutex<Data>,
}

impl IncludesDepot {
    pub fn new(fd: &FileDepot) -> IncludesDepot {
        IncludesDepot {
            data: Mutex::new(Data::new(fd)),
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

    pub fn dump(&self) {
        self.data.lock().unwrap().dump();
    }
}

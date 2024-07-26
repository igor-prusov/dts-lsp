use crate::utils::Symbol;
use crate::FileDepot;
use crate::{error, info, log_message};
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Mutex;
use tower_lsp::lsp_types::{MessageType, Range, Url};

#[derive(Clone, Eq, Hash, PartialEq)]
struct Label {
    uri: Url,
    name: String,
}

#[derive(Clone)]
struct Data {
    label_to_symbol: HashMap<Label, Range>,
    fd: FileDepot,
}

impl Data {
    fn new(fd: &FileDepot) -> Data {
        Data {
            label_to_symbol: HashMap::new(),
            fd: fd.clone(),
        }
    }
    fn add_label(&mut self, label: &str, uri: &Url, range: Range) {
        self.label_to_symbol.insert(
            Label {
                uri: uri.clone(),
                name: label.to_string(),
            },
            range,
        );
    }

    #[cfg(test)]
    fn size(&self) -> usize {
        self.label_to_symbol.keys().count()
    }

    fn find_label(&self, uri: &Url, label: &str) -> Vec<Symbol> {
        let mut visited = HashSet::new();
        let mut to_visit = vec![uri.clone()];
        let mut res = Vec::new();

        self.fd.dump();
        let v = self.fd.get_component(uri);
        for f in &v {
            if !visited.contains(f) {
                to_visit.push(f.clone());
            }
        }

        while let Some(uri) = to_visit.pop() {
            info!("processing {uri}");

            if let Some(range) = self.label_to_symbol.get(&Label {
                name: label.to_string(),
                uri: uri.clone(),
            }) {
                res.push(Symbol::new(uri.clone(), *range));
            }

            visited.insert(uri);
        }

        res
    }

    fn rename(&mut self, uri: &Url, old_name: &str, new_name: &str) -> Result<(), String> {
        let old = self.label_to_symbol.remove_entry(&Label {
            name: old_name.to_string(),
            uri: uri.clone(),
        });

        match old {
            None => Err(format!("Renaming non-existant label: {old_name}")),
            Some((old_label, mut range)) => {
                let new_name_len = u32::try_from(new_name.len()).map_err(|e| format!("{e}"))?;
                range.end.character = range.start.character + new_name_len;
                self.label_to_symbol.insert(
                    Label {
                        name: new_name.to_string(),
                        uri: old_label.uri,
                    },
                    range,
                );
                Ok(())
            }
        }
    }

    fn invalidate(&mut self, uri: &Url) {
        let mut v = Vec::new();

        for k in self.label_to_symbol.keys() {
            if k.uri == *uri {
                v.push(k.clone());
            }
        }

        for label in v {
            self.label_to_symbol.remove(&label);
        }
    }

    fn dump(&self) {
        info!("====== (labels) ======");
        for k in self.label_to_symbol.keys() {
            info!("url: {}: {}", k.uri, k.name);
        }
        info!("======================");
    }
}

pub struct LabelsDepot {
    data: Mutex<Data>,
}

impl LabelsDepot {
    pub fn new(fd: &FileDepot) -> LabelsDepot {
        LabelsDepot {
            data: Mutex::new(Data::new(fd)),
        }
    }

    pub fn add_label(&self, label: &str, uri: &Url, range: Range) {
        info!("LabelsDepot::add_label()");
        let mut data = self.data.lock().unwrap();
        data.add_label(label, uri, range);
    }

    pub fn find_label(&self, uri: &Url, label: &str) -> Vec<Symbol> {
        info!("LabelsDepot::find_label()");
        {
            let data = self.data.lock().unwrap();
            data.clone()
        }
        .find_label(uri, label)
    }

    pub fn dump(&self) {
        info!("LabelsDepot::dump()");
        {
            let lock = self.data.lock().unwrap();
            lock.clone()
        }
        .dump();
    }

    pub fn invalidate(&self, uri: &Url) {
        info!("LabelsDepot::invalidate()");
        let mut data = self.data.lock().unwrap();
        data.invalidate(uri);
    }

    pub fn rename(&self, uri: &Url, old_name: &str, new_name: &str) {
        let res = {
            let mut data = self.data.lock().unwrap();
            data.rename(uri, old_name, new_name)
        };

        match res {
            Ok(()) => (),
            Err(e) => error!("{}", e),
        }
    }

    #[cfg(test)]
    pub fn size(&self) -> usize {
        let data = self.data.lock().unwrap();
        data.size()
    }
}

#[cfg(test)]
impl PartialEq for LabelsDepot {
    fn eq(&self, other: &Self) -> bool {
        let me = self.data.lock().unwrap();
        let other = other.data.lock().unwrap();
        me.label_to_symbol == other.label_to_symbol
    }
}

use crate::utils::Symbol;
use crate::FileDepot;
use crate::{error, info, log_message};
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Mutex;
use tower_lsp::lsp_types::{MessageType, Range, Url};
/*
 * 1. Add all references to map Reference (similar to Label) -> Vec[Range],
 * 2. Find references: Look-up label in all connected files
 *
 */

#[derive(Clone, Eq, Hash, PartialEq)]
struct Reference {
    uri: Url,
    name: String,
}

impl Reference {
    fn new(uri: &Url, name: &str) -> Reference {
        Reference {
            uri: uri.clone(),
            name: name.to_string(),
        }
    }
}

#[derive(Clone)]
struct Data {
    reference_to_symbols: HashMap<Reference, Vec<Range>>,
    fd: FileDepot,
}

impl Data {
    fn new(fd: &FileDepot) -> Data {
        Data {
            reference_to_symbols: HashMap::new(),
            fd: fd.clone(),
        }
    }

    fn add_reference(&mut self, name: &str, uri: &Url, range: Range) {
        let r = Reference::new(uri, name);
        if let Some(ref mut v) = self.reference_to_symbols.get_mut(&r) {
            assert!(!v.contains(&range));
            v.push(range);
        } else {
            let v = vec![range];
            self.reference_to_symbols.insert(r, v);
        }
    }

    async fn find_references(&self, uri: &Url, name: &str) -> Vec<Symbol> {
        let mut to_visit = vec![uri.clone()];
        let mut visited = HashSet::new();
        let mut res = Vec::new();
        let v = self.fd.get_component(uri).await;
        for f in &v {
            if !visited.contains(f) {
                to_visit.push(f.clone());
            }
        }

        while let Some(uri) = to_visit.pop() {
            if let Some(v) = self.reference_to_symbols.get(&Reference::new(&uri, name)) {
                res.extend(v.iter().map(|x| Symbol::new(uri.clone(), *x)));
            }

            visited.insert(uri);
        }
        res
    }

    fn invalidate(&mut self, uri: &Url) {
        let mut v = Vec::new();
        for k in self.reference_to_symbols.keys() {
            if k.uri == *uri {
                v.push(k.clone());
            }
        }

        for reference in v {
            self.reference_to_symbols.remove(&reference);
        }
    }

    fn rename(&mut self, uri: &Url, old_name: &str, new_name: &str) -> Result<(), String> {
        let old = self.reference_to_symbols.remove_entry(&Reference {
            name: old_name.to_string(),
            uri: uri.clone(),
        });

        match old {
            None => Err(format!("Renaming non-existant label: {old_name}")),
            Some((old_label, mut ranges)) => {
                for range in &mut ranges {
                    let new_name_len = u32::try_from(new_name.len()).map_err(|e| format!("{e}"))?;
                    range.end.character = range.start.character + new_name_len;
                }
                self.reference_to_symbols.insert(
                    Reference {
                        name: new_name.to_string(),
                        uri: old_label.uri,
                    },
                    ranges,
                );
                Ok(())
            }
        }
    }

    #[cfg(test)]
    fn size(&self) -> usize {
        self.reference_to_symbols.keys().count()
    }

    async fn dump(&self) {
        info!("===REFERENCES===");
        for (k, v) in &self.reference_to_symbols {
            info!("{} ({}): {}", k.name, k.uri, v.len());
        }
    }
}

pub struct ReferencesDepot {
    data: Mutex<Data>,
}

impl ReferencesDepot {
    pub fn new(fd: &FileDepot) -> ReferencesDepot {
        ReferencesDepot {
            data: Mutex::new(Data::new(fd)),
        }
    }

    pub async fn add_reference(&self, name: &str, uri: &Url, range: Range) {
        info!("ReferencesDepot::add_reference()");
        let mut data = self.data.lock().unwrap();
        data.add_reference(name, uri, range);
    }

    pub async fn find_references(&self, uri: &Url, name: &str) -> Vec<Symbol> {
        info!("ReferencesDepot::find_references()");
        {
            let x = self.data.lock().unwrap();
            x.clone()
        }
        .find_references(uri, name)
        .await
    }

    pub async fn invalidate(&self, uri: &Url) {
        info!("ReferencesDepot::invalidate()");
        let mut data = self.data.lock().unwrap();
        data.invalidate(uri);
    }

    pub async fn rename(&self, uri: &Url, old_name: &str, new_name: &str) {
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
    pub async fn size(&self) -> usize {
        let data = self.data.lock().unwrap();
        data.size()
    }

    pub async fn dump(&self) {
        {
            let x = self.data.lock().unwrap();
            x.clone()
        }
        .dump()
        .await;
    }
}

#[cfg(test)]
impl PartialEq for ReferencesDepot {
    fn eq(&self, other: &Self) -> bool {
        let me = self.data.lock().unwrap().clone();
        let other = other.data.lock().unwrap().clone();

        //TODO: make vectors sorted
        me.reference_to_symbols == other.reference_to_symbols
    }
}

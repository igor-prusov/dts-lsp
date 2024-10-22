use crate::{error, info, log_message, utils::is_header};
use std::collections::HashMap;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::Mutex;
use tower_lsp::lsp_types::{MessageType, TextEdit, Url};

#[derive(Default, Clone)]
struct FileEntry {
    text: Option<String>,
    includes: Vec<Url>,
    included_by: Vec<Url>,
}

#[derive(Clone)]
struct Data {
    root_dir: Option<Url>, // TODO: Maybe some type that allows only one assignment?
    includes_prefix: String,
    entries: HashMap<Url, FileEntry>,
}

#[derive(PartialEq)]
pub enum InsertResult {
    Ok,
    Exists,
    Modified,
}

struct MyTextEdit<'a>(&'a TextEdit);
impl<'a> MyTextEdit<'a> {
    fn new_text(&self) -> &str {
        &self.0.new_text
    }

    fn lines_range(&self) -> (usize, usize) {
        (
            self.0.range.start.line as usize,
            self.0.range.end.line as usize,
        )
    }

    fn chars_range(&self) -> (usize, usize) {
        (
            self.0.range.start.character as usize,
            self.0.range.end.character as usize,
        )
    }
}

impl Data {
    fn new() -> Data {
        Data {
            root_dir: None,
            includes_prefix: "include".to_string(),
            entries: HashMap::new(),
        }
    }

    fn apply_edits(&mut self, uri: &Url, edits: &Vec<TextEdit>) -> Result<(), String> {
        for edit in edits {
            let edit = MyTextEdit(edit);
            let Some(e) = self.entries.get_mut(uri) else {
                return Err("Failed to apply edits".to_string());
            };
            let mut new_text = String::new();
            if let Some(text) = &e.text {
                let (start_line, end_line) = edit.lines_range();
                let (start_char, end_char) = edit.chars_range();
                let multiline = start_line != end_line;

                for (n, line) in text.split_inclusive('\n').enumerate() {
                    let mut line = line.to_string();

                    #[allow(clippy::if_same_then_else)]
                    if n == start_line && !multiline {
                        line.replace_range(start_char..end_char, edit.new_text());
                    } else if n == start_line && multiline {
                        line.replace_range(start_char.., "");
                    } else if (start_line + 1..end_line).contains(&n) {
                        continue;
                    } else if n == end_line {
                        line.replace_range(..end_char, edit.new_text());
                    }

                    new_text.push_str(&line);
                }
                e.text = Some(new_text);
            }
        }
        Ok(())
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

    fn get_real_path(&self, rel_path: &str) -> Option<Url> {
        let Some(root) = &self.root_dir else {
            error!("Root dir is not set");
            return None;
        };

        let Ok(dst) = root.join(&(self.includes_prefix.clone() + "/")) else {
            error!("failed to join {root} and {}", self.includes_prefix);
            return None;
        };

        let Ok(dst) = dst.join(rel_path) else {
            error!("failed to join {root} and {}", rel_path);
            return None;
        };

        Some(dst)
    }

    fn add_include(&mut self, uri: &Url, include_uri: &Url) {
        let e = self.entries.entry(uri.clone()).or_default();
        e.includes.push(include_uri.clone());

        let e = self.entries.entry(include_uri.clone()).or_default();
        e.included_by.push(uri.clone());
    }

    fn get_component(&self, uri: &Url) -> Vec<Url> {
        // Process includes
        let mut to_visit = vec![uri.clone()];
        let mut visited = HashSet::new();
        let mut res: Vec<Url> = Vec::new();
        while let Some(uri) = to_visit.pop() {
            if let Some(e) = self.entries.get(&uri) {
                for f in &e.includes {
                    if !visited.contains(f) {
                        if !is_header(f) {
                            to_visit.push(f.clone());
                        }
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
                    if !is_header(f) {
                        to_visit.push(f.clone());
                    }
                    visited.insert(f.clone());
                    res.push(f.clone());
                }
            }
        }

        while let Some(uri) = to_visit.pop() {
            if let Some(e) = self.entries.get(&uri) {
                for f in e.includes.iter().chain(e.included_by.iter()) {
                    if !visited.contains(f) {
                        if !is_header(f) {
                            to_visit.push(f.clone());
                        }
                        visited.insert(f.clone());
                        res.push(f.clone());
                    }
                }
            }
            visited.insert(uri);
        }

        //TODO: Fix this ugly hack
        let res: HashSet<Url> = res.iter().cloned().collect();
        res.iter().cloned().collect()
    }

    fn dump(&self) {
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

    fn set_includes_prefix(&mut self, prefix: &str) {
        self.includes_prefix = prefix.to_string();
    }

    fn set_root_dir(&mut self, uri: &Url) {
        /* root_dir comes from LSP client and it's better to
         * verify that there is a trailing slash */
        let mut uri = uri.clone();
        if !uri.path().ends_with('/') {
            uri.set_path(&(uri.path().to_string() + "/"));
        }
        self.root_dir = Some(uri.clone());
    }

    #[cfg(any(test, feature = "walkdir"))]
    pub fn get_root_dir(&self) -> Option<Url> {
        self.root_dir.clone()
    }

    #[cfg(test)]
    fn size(&self) -> usize {
        self.entries.keys().count()
    }

    #[cfg(test)]
    fn n_with_text(&self) -> usize {
        self.entries
            .iter()
            .map(|x| usize::from(x.1.text.is_some()))
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

    pub fn insert(&self, uri: &Url, text: &str) -> InsertResult {
        let mut data = self.data.lock().unwrap();
        data.insert(uri, text)
    }

    pub fn dump(&self) {
        {
            let lock = self.data.lock().unwrap();
            lock.clone()
        }
        .dump();
    }

    pub fn get_text(&self, uri: &Url) -> Option<String> {
        self.data.lock().unwrap().get_text(uri)
    }

    pub fn exist(&self, uri: &Url) -> bool {
        self.data.lock().unwrap().exist(uri)
    }

    pub fn add_include(&self, uri: &Url, include_uri: &Url) {
        self.data.lock().unwrap().add_include(uri, include_uri);
    }

    pub fn get_component(&self, uri: &Url) -> Vec<Url> {
        self.data.lock().unwrap().get_component(uri)
    }

    pub fn apply_edits(&self, uri: &Url, edits: &Vec<TextEdit>) {
        if let Err(e) = {
            let mut data = self.data.lock().unwrap();
            data.apply_edits(uri, edits)
        } {
            error!("{}", e);
        }
    }

    pub fn get_real_path(&self, uri: &str) -> Option<Url> {
        self.data.lock().unwrap().get_real_path(uri)
    }

    // TODO: Allow per-workspace include prefixes
    #[allow(dead_code)]
    pub fn set_includes_prefix(&self, prefix: &str) {
        self.data.lock().unwrap().set_includes_prefix(prefix);
    }

    pub fn set_root_dir(&self, uri: &Url) {
        self.data.lock().unwrap().set_root_dir(uri);
    }

    #[cfg(any(test, feature = "walkdir"))]
    pub fn get_root_dir(&self) -> Option<Url> {
        self.data.lock().unwrap().get_root_dir()
    }

    #[cfg(test)]
    pub fn size(&self) -> usize {
        self.data.lock().unwrap().size()
    }

    #[cfg(test)]
    pub fn n_with_text(&self) -> usize {
        self.data.lock().unwrap().n_with_text()
    }
}

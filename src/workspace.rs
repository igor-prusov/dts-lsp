use crate::file_depot;
use crate::file_depot::FileDepot;
use crate::includes_depot::IncludesDepot;
use crate::labels_depot::LabelsDepot;
use crate::references_depot::ReferencesDepot;
use crate::utils::convert_range;
use crate::utils::extension_one_of;
use crate::utils::is_header;
use crate::{error, log_message, warn};
use std::fs::metadata;
use std::fs::read_dir;
use std::fs::read_to_string;
use tower_lsp::lsp_types::{MessageType, Url};
use tree_sitter::Parser;
use tree_sitter::Query;
use tree_sitter::QueryCursor;
use tree_sitter::Tree;

pub struct Workspace {
    pub fd: FileDepot,
    pub ld: LabelsDepot,
    pub rd: ReferencesDepot,
    pub id: IncludesDepot,
}

impl Workspace {
    pub fn new() -> Workspace {
        let fd = FileDepot::new();
        Workspace {
            ld: LabelsDepot::new(&fd),
            rd: ReferencesDepot::new(&fd),
            id: IncludesDepot::new(&fd),
            fd,
        }
    }

    pub fn process_labels(&self, tree: &Tree, uri: &Url, text: &str) {
        let mut cursor = QueryCursor::new();

        let q = Query::new(
            &tree_sitter_devicetree::language(),
            "(node label: (identifier)@id)",
        )
        .unwrap();
        let matches = cursor.matches(&q, tree.root_node(), text.as_bytes());
        let mut labels = Vec::new();
        for m in matches {
            let nodes = m.nodes_for_capture_index(0);
            for node in nodes {
                let label = node.utf8_text(text.as_bytes()).unwrap();
                let range = node.range();
                labels.push((label, uri, range));
            }
        }

        for (label, uri, range) in labels {
            self.ld.add_label(label, uri, convert_range(&range));
        }
    }

    pub fn process_includes(&self, tree: &Tree, uri: &Url, text: &str) -> Vec<Url> {
        let mut cursor = QueryCursor::new();
        let q = Query::new(
            &tree_sitter_devicetree::language(),
            "[
            (dtsi_include path: (string_literal)@id)
            (preproc_include path: (string_literal)@id)
            (preproc_include path: (system_lib_string)@id)
            ]",
        )
        .unwrap();
        let matches = cursor.matches(&q, tree.root_node(), text.as_bytes());
        let mut v = Vec::new();
        for m in matches {
            let nodes = m.nodes_for_capture_index(0);
            for node in nodes {
                let label = node.utf8_text(text.as_bytes()).unwrap();
                let mut needs_fixup = false;
                if label.ends_with('>') {
                    needs_fixup = true;
                }
                let label = label.trim_matches('"');
                let label = label.trim_matches('<');
                let label = label.trim_matches('>');
                let mut new_url = uri.join(label).unwrap();
                if needs_fixup {
                    new_url = self.fd.get_real_path(label).unwrap();
                }
                v.push(new_url.clone());
                self.fd.add_include(uri, &new_url);
            }
        }
        v
    }

    pub fn process_references(&self, tree: &Tree, uri: &Url, text: &str) {
        let mut cursor = QueryCursor::new();

        let q = Query::new(
            &tree_sitter_devicetree::language(),
            "(reference label: (identifier)@id)",
        )
        .unwrap();
        let matches = cursor.matches(&q, tree.root_node(), text.as_bytes());
        let mut references = Vec::new();
        for m in matches {
            let nodes = m.nodes_for_capture_index(0);
            for node in nodes {
                let label = node.utf8_text(text.as_bytes()).unwrap();
                let range = node.range();
                references.push((label, uri, range));
            }
        }

        for (label, uri, range) in references {
            self.rd.add_reference(label, uri, convert_range(&range));
        }
    }

    pub fn process_defines(&self, tree: &Tree, uri: &Url, text: &str) {
        let mut cursor = QueryCursor::new();

        let q = Query::new(
            &tree_sitter_devicetree::language(),
            "[
            (preproc_def name: (identifier)@name value: (preproc_arg)@id)
            (preproc_function_def name: (identifier)@name parameters: (preproc_params) value: (preproc_arg)@id)
            ]",
        )
        .unwrap();
        let matches = cursor.matches(&q, tree.root_node(), text.as_bytes());
        for m in matches {
            let nodes = m
                .nodes_for_capture_index(0)
                .zip(m.nodes_for_capture_index(1));
            for (name, value) in nodes {
                let def_name = name.utf8_text(text.as_bytes()).unwrap();
                let value = value.utf8_text(text.as_bytes()).unwrap();
                let value = value.trim_end().trim_start();
                self.id
                    .add_define(def_name, uri, convert_range(&name.range()), value);
            }
        }
    }

    fn handle_single_file(&self, uri: &Url, text: Option<String>) -> Vec<Url> {
        if !extension_one_of(uri, &["dts", "dtsi", "h"]) {
            return Vec::new();
        }

        let Ok(path) = uri.to_file_path() else {
            error!("Invalid url {}", uri);
            return Vec::new();
        };

        let text = match text.map_or(read_to_string(path), Ok) {
            Ok(x) => x,
            Err(e) => {
                warn!("can't read file {}: {}", uri, e.kind());
                return Vec::new();
            }
        };

        match self.fd.insert(uri, &text) {
            file_depot::InsertResult::Exists => return Vec::new(),
            file_depot::InsertResult::Modified => {
                self.ld.invalidate(uri);
                self.rd.invalidate(uri);
            }
            file_depot::InsertResult::Ok => (),
        };

        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_devicetree::language())
            .unwrap();
        let tree = parser.parse(&text, None).unwrap();

        self.process_defines(&tree, uri, &text);
        if is_header(uri) {
            return Vec::new();
        }

        self.process_labels(&tree, uri, &text);
        self.process_references(&tree, uri, &text);
        self.process_includes(&tree, uri, &text)
    }

    pub fn handle_file(&self, uri: &Url, text: Option<String>, process_includes: bool) {
        let mut includes = self.handle_single_file(uri, text);
        if process_includes {
            while let Some(new_url) = includes.pop() {
                let mut tmp = self.handle_single_file(&new_url, None);
                includes.append(&mut tmp);
            }
        }
    }

    pub fn open_neighbours(&self, uri: &Url) {
        let d = uri.join(".").unwrap();
        let Ok(path) = d.to_file_path() else {
            error!("Invalid url {}", d);
            return;
        };

        // Skip if client has opened a buffer for a file that has some
        // directories in its path that have not been created yet.
        let Ok(files) = read_dir(path) else {
            return;
        };

        for f in files {
            let p = f.unwrap().path();
            if !metadata(&p).unwrap().is_file() {
                continue;
            }
            let u = Url::from_file_path(p).unwrap();
            if self.fd.exist(&u) {
                continue;
            }
            self.handle_file(&u, None, false);
        }
    }
}

use tower_lsp::lsp_types::{Diagnostic, Url};
use tree_sitter::{Node, Tree};

use crate::{includes_depot::IncludesDepot, utils::convert_range};

pub struct DiagnosticExt {
    pub diag: Diagnostic,
    url: Url, // TODO: remove this
    label: String,
    parent: Option<String>,
}

fn process_node(node: &Node, diagnostics: &mut Vec<DiagnosticExt>, url: &Url, text: &str) {
    let range = convert_range(&node.range());
    let label = node.utf8_text(text.as_bytes()).unwrap();
    if node.is_missing() {
        let msg = format!("missing {}", node.grammar_name());
        diagnostics.push(DiagnosticExt {
            diag: Diagnostic::new_simple(range, msg),
            label: label.to_string(),
            url: url.clone(),
            parent: None,
        });
    } else if node.is_error() {
        // Ignore syntax errors if they are in tokens that will be replaced after preprocessing
        // TODO: We should implement preprocessor pass for proper error reporting, but for now
        // just trade error detection inside macros for less false-positive noise.
        let parent = node
            .parent()
            .and_then(|x| x.utf8_text(text.as_bytes()).ok())
            .map(std::string::ToString::to_string);
        diagnostics.push(DiagnosticExt {
            diag: Diagnostic::new_simple(range, "Syntax error".to_string()),
            label: label.to_string(),
            url: url.clone(),
            parent,
        });
    }
}

pub fn gather(url: &Url, tree: &Tree, text: &str) -> Vec<DiagnosticExt> {
    let mut diagnostics = Vec::new();
    let mut it = tree.walk();
    process_node(&it.node(), &mut diagnostics, url, text);
    let mut recurse = true;

    #[allow(clippy::if_same_then_else)]
    loop {
        if recurse && it.goto_first_child() {
            process_node(&it.node(), &mut diagnostics, url, text);
            recurse = true;
        } else if it.goto_next_sibling() {
            process_node(&it.node(), &mut diagnostics, url, text);
            recurse = true;
        } else if it.goto_parent() {
            recurse = false;
        } else {
            break;
        }
    }
    diagnostics
}

impl DiagnosticExt {
    pub fn verify(&self, id: &IncludesDepot) -> bool {
        if self.diag.message.starts_with("missing") {
            return true;
        }

        if id.find_define(&self.url, &self.label).is_some() {
            return false;
        }

        if let Some(parent) = &self.parent {
            if let Some((parent, _)) = parent.split_once('(') {
                if id.find_define(&self.url, parent).is_some() {
                    return false;
                }
            }
        }

        true
    }
}

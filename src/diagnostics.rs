use tower_lsp::lsp_types::Diagnostic;
use tree_sitter::{Node, Tree};

use crate::utils::convert_range;

fn process_node(node: &Node, diagnostics: &mut Vec<Diagnostic>) {
    if node.is_missing() {
        let range = convert_range(&node.range());
        let name = node.grammar_name();
        let text = format!("missing {name}");
        diagnostics.push(Diagnostic::new_simple(range, text));
    } else if node.is_error() {
        let range = convert_range(&node.range());
        diagnostics.push(Diagnostic::new_simple(range, "Syntax error".to_string()));
    }
}

pub fn gather(tree: &Tree) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    let mut it = tree.walk();
    process_node(&it.node(), &mut diagnostics);
    let mut recurse = true;

    #[allow(clippy::if_same_then_else)]
    loop {
        if recurse && it.goto_first_child() {
            process_node(&it.node(), &mut diagnostics);
            recurse = true;
        } else if it.goto_next_sibling() {
            process_node(&it.node(), &mut diagnostics);
            recurse = true;
        } else if it.goto_parent() {
            recurse = false;
        } else {
            break;
        }
    }
    diagnostics
}

use std::fs::metadata;
use std::fs::read_dir;
use std::fs::File;
use std::io::prelude::*;
use tower_lsp::jsonrpc::Result;
#[allow(clippy::wildcard_imports)]
use tower_lsp::lsp_types::*;
use tower_lsp::{LanguageServer, LspService, Server};
use tree_sitter::Parser;
use tree_sitter::Point;
use tree_sitter::Query;
use tree_sitter::QueryCursor;
use tree_sitter::Tree;

mod file_depot;
mod labels_depot;
mod logger;
mod references_depot;
mod utils;

use file_depot::FileDepot;
use labels_depot::LabelsDepot;
use logger::Logger;
use references_depot::ReferencesDepot;

struct Backend {
    data: Data,
    logger: Logger,
}

impl Backend {
    fn new(logger: Logger) -> Self {
        Backend {
            data: Data::new(&logger),
            logger,
        }
    }
    async fn process_labels(&self, tree: &Tree, uri: &Url, text: &str) {
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
            self.data.ld.add_label(label, uri, range).await;
        }
    }

    async fn process_includes(&self, tree: &Tree, uri: &Url, text: &str) -> Vec<Url> {
        let mut cursor = QueryCursor::new();
        let q = Query::new(
            &tree_sitter_devicetree::language(),
            "[
            (dtsi_include path: (string_literal)@id)
            (preproc_include path: (string_literal)@id)
            ]",
        )
        .unwrap();
        let matches = cursor.matches(&q, tree.root_node(), text.as_bytes());
        let mut v = Vec::new();
        let mut logs = Vec::new();
        let mut includes = Vec::new();
        for m in matches {
            let nodes = m.nodes_for_capture_index(0);
            for node in nodes {
                let label = node.utf8_text(text.as_bytes()).unwrap();
                let label = label.trim_matches('"');
                let range = node.range();
                let pos = range.start_point;
                let new_url = uri.join(label).unwrap();
                v.push(new_url.clone());
                includes.push((uri, new_url.clone()));
                logs.push(format!(
                    "INCLUDE<{}>: {}, {}",
                    node.kind(),
                    new_url,
                    pos.row
                ));
            }
        }
        for msg in logs {
            self.logger.log_message(MessageType::INFO, &msg).await;
        }
        for (uri, new_url) in includes {
            self.data.fd.add_include(uri, &new_url).await;
        }
        v
    }

    async fn process_references(&self, tree: &Tree, uri: &Url, text: &str) {
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
            self.logger
                .log_message(MessageType::INFO, format!("LABEL = {label}"))
                .await;
            self.data.rd.add_reference(label, uri, range).await;
        }
    }

    async fn handle_file(&self, uri: &Url, text: Option<String>) -> Vec<Url> {
        let text = if let Some(x) = text {
            x
        } else {
            let mut file = File::open(uri.path()).unwrap();
            let mut s = String::new();
            match file.read_to_string(&mut s) {
                Ok(_) => {}
                Err(e) => {
                    self.logger
                        .log_message(MessageType::WARNING, format!("{}: {}", uri, e.kind()))
                        .await;
                }
            };
            s
        };
        self.data.fd.insert(uri, text.clone()).await;

        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_devicetree::language())
            .unwrap();
        let tree = parser.parse(&text, None).unwrap();

        self.process_labels(&tree, uri, &text).await;
        self.process_references(&tree, uri, &text).await;
        self.process_includes(&tree, uri, &text).await
    }

    fn extension_one_of(url: &Url, exts: &[&str]) -> bool {
        let Some(url_ext) = std::path::Path::new(url.path()).extension() else {
            return false;
        };

        for ext in exts {
            if url_ext.eq_ignore_ascii_case(ext) {
                return true;
            }
        }
        false
    }

    async fn open_neighbours(&self, uri: &Url) {
        let d = uri.join(".").unwrap();
        let d = d.path();
        let files = read_dir(d).unwrap();
        for f in files {
            let p = f.unwrap().path();
            if !metadata(&p).unwrap().is_file() {
                continue;
            }
            let u = Url::from_file_path(p).unwrap();
            if !Self::extension_one_of(&u, &["dts", "dtsi"]) {
                continue;
            }
            if self.data.fd.exist(&u).await {
                continue;
            }
            self.handle_file(&u, None).await;
        }
    }
}

struct Data {
    fd: FileDepot,
    ld: LabelsDepot,
    rd: ReferencesDepot,
}

impl Data {
    fn new(logger: &logger::Logger) -> Data {
        let fd = FileDepot::new(logger.clone());
        Data {
            ld: LabelsDepot::new(logger.clone(), &fd),
            rd: ReferencesDepot::new(logger.clone(), &fd),
            fd,
        }
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, _: InitializeParams) -> Result<InitializeResult> {
        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                ..ServerCapabilities::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.logger
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = &params.text_document.uri;

        let msg = format!("Open file: {uri}");
        self.logger.log_message(MessageType::INFO, msg).await;

        let text = params.text_document.text.as_str();
        let mut includes = self.handle_file(uri, Some(text.to_string())).await;

        while let Some(new_url) = includes.pop() {
            let mut tmp = self.handle_file(&new_url, None).await;
            includes.append(&mut tmp);
        }

        self.data.fd.dump().await;
        self.data.ld.dump().await;

        self.open_neighbours(uri).await;
    }

    async fn goto_definition(
        &self,
        input: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        // TODO: Handle files, that look like arch/arc/boot/dts/skeleton.dtsi
        // i.e. there is dtsi file with labels, that are expected to be in files
        // including skeleton.dtsi
        let location = input.text_document_position_params.position;
        let location = Point::new(location.line as usize, location.character as usize);
        let uri = input.text_document_position_params.text_document.uri;
        let Some(text) = self.data.fd.get_text(&uri).await else {
            return Ok(None);
        };
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_devicetree::language())
            .unwrap();
        let tree = parser.parse(&text, None).unwrap();
        if let Some(node) = tree
            .root_node()
            .named_descendant_for_point_range(location, location)
        {
            let label = node.utf8_text(text.as_bytes()).unwrap();

            if let (Some(parent), Some(point)) =
                (node.parent(), self.data.ld.find_label(&uri, label).await)
            {
                if parent.kind() == "reference" {
                    let pos = Location::new(point.uri, point.range);
                    return Ok(Some(GotoDefinitionResponse::Scalar(pos)));
                }
            }
        }

        Ok(None)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let location = params.text_document_position.position;
        let location = Point::new(location.line as usize, location.character as usize);
        let uri = params.text_document_position.text_document.uri;

        let Some(text) = self.data.fd.get_text(&uri).await else {
            return Ok(None);
        };

        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_devicetree::language())
            .unwrap();
        let tree = parser.parse(&text, None).unwrap();
        if let Some(node) = tree
            .root_node()
            .named_descendant_for_point_range(location, location)
        {
            let label = node.utf8_text(text.as_bytes()).unwrap();

            if let (Some(parent), v) = (
                node.parent(),
                self.data.rd.find_references(&uri, label).await,
            ) {
                if parent.kind() == "node" {
                    let mut res = Vec::new();
                    for x in v {
                        res.push(Location::new(x.uri, x.range));
                    }
                    return Ok(Some(res));
                }
            }
        }
        Ok(None)
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let msg = format!("Close file: {}", params.text_document.uri);
        self.logger.log_message(MessageType::INFO, msg).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let msg = format!("Change file: {}", params.text_document.uri);
        self.logger.log_message(MessageType::INFO, msg).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let msg = format!("Save file: {}", params.text_document.uri);
        self.logger.log_message(MessageType::INFO, msg).await;
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| {
        let logger = Logger::Lsp(client);
        Backend::new(logger)
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}

#[cfg(test)]
mod tests {

    async fn be_single_file(file_text: &str) -> (Backend, Url) {
        let logger = Logger::Print;
        let be = Backend::new(logger.clone());
        let url = Url::parse("file:///tmp/fake_url").unwrap();
        let file_data = String::from(file_text);

        be.handle_file(&url, Some(file_data)).await;
        (be, url)
    }

    use super::*;
    #[tokio::test]
    async fn functional() {
        {
            let (be, _) = be_single_file("Bad file").await;

            assert_eq!(be.data.fd.size().await, 1);
            assert_eq!(be.data.ld.size().await, 0);
            assert_eq!(be.data.rd.size().await, 0);
        }
        {
            let (be, url) = be_single_file(
                "
            / {
                lbl: node{};
            };
            ",
            )
            .await;

            assert_eq!(be.data.fd.size().await, 1);
            assert_eq!(be.data.ld.size().await, 1);
            assert_eq!(be.data.rd.size().await, 0);
            assert!(be.data.ld.find_label(&url, "lbl").await.is_some());
            assert!(be.data.ld.find_label(&url, "label").await.is_none());
        }
    }
}

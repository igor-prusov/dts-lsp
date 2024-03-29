use std::fs::metadata;
use std::fs::read_dir;
use std::fs::File;
use std::io::prelude::*;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tree_sitter::Parser;
use tree_sitter::Point;
use tree_sitter::Query;
use tree_sitter::QueryCursor;
use tree_sitter::Tree;

mod file_depot;
mod labels_depot;

use file_depot::FileDepot;
use labels_depot::LabelsDepot;

struct Backend {
    client: Client,
    data: Data,
}

impl Backend {
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
                //self.data.ld.add_include(uri, &new_url).await;
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
            self.client.log_message(MessageType::INFO, &msg).await;
        }
        for (uri, new_url) in includes {
            self.data.ld.add_include(uri, &new_url).await;
        }
        v
    }

    async fn handle_file(&self, uri: &Url, text: Option<String>) -> Vec<Url> {
        let text = if let Some(x) = text {
            x
        } else {
            let mut file = File::open(uri.path()).unwrap();
            let mut s = String::new();
            file.read_to_string(&mut s).unwrap();
            s
        };
        self.data.fd.insert(uri, text.clone()).await;

        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_devicetree::language())
            .unwrap();
        let tree = parser.parse(&text, None).unwrap();

        self.process_labels(&tree, uri, &text).await;
        self.process_includes(&tree, uri, &text).await
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
}

impl Data {
    fn new(client: &Client) -> Data {
        Data {
            fd: FileDepot::new(client.clone()),
            ld: LabelsDepot::new(client.clone()),
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
                ..ServerCapabilities::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        self.client
            .log_message(MessageType::INFO, "server initialized!")
            .await;
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = &params.text_document.uri;

        let msg = format!("Open file: {}", uri);
        self.client.log_message(MessageType::INFO, msg).await;

        let text = params.text_document.text.as_str();
        let mut includes = self.handle_file(uri, Some(text.to_string())).await;

        while let Some(new_url) = includes.pop() {
            let mut tmp = self.handle_file(&new_url, None).await;
            includes.append(&mut tmp)
        }

        self.data.fd.dump().await;
        self.data.ld.dump().await;

        self.open_neighbours(uri).await;
    }

    async fn goto_definition(
        &self,
        input: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let location = input.text_document_position_params.position;
        let location = Point::new(location.line as usize, location.character as usize);
        //let text = self.data.get_text();
        let uri = input.text_document_position_params.text_document.uri;
        let text = match self.data.fd.get_text(&uri).await {
            Some(text) => text,
            None => return Ok(None),
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

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let msg = format!("Close file: {}", params.text_document.uri);
        self.client.log_message(MessageType::INFO, msg).await;
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let msg = format!("Change file: {}", params.text_document.uri);
        self.client.log_message(MessageType::INFO, msg).await;
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let msg = format!("Save file: {}", params.text_document.uri);
        self.client.log_message(MessageType::INFO, msg).await;
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| Backend {
        data: Data::new(&client),
        client,
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}

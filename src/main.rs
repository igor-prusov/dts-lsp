use std::collections::HashMap;
use std::fs::File;
use std::fs::OpenOptions;
use std::io::Write;
use std::sync::Mutex;
use tower_lsp::jsonrpc::Result;
use tower_lsp::lsp_types::Range;
use tower_lsp::lsp_types::*;
use tower_lsp::{Client, LanguageServer, LspService, Server};
use tree_sitter::Parser;
use tree_sitter::Point;
use tree_sitter::Query;
use tree_sitter::QueryCursor;
use tree_sitter_devicetree;

mod file_depot;

use file_depot::FileDepot;

struct Backend {
    client: Client,
    data: Data,
}

struct Logger {}

impl Logger {
    const PATH: &'static str = "/tmp/dts-lsp-log.txt";
    fn init() {
        let mut file = File::create(Self::PATH).unwrap();
        writeln!(file, "====START====").unwrap();
    }

    fn log(text: &str) {
        let mut file = OpenOptions::new()
            .write(true)
            .append(true)
            .open(Self::PATH)
            .unwrap();
        writeln!(file, "{}", text).unwrap();
    }
}

#[derive(Clone)]
struct Symbol {
    uri: Url,
    range: Range,
}

impl Symbol {
    fn new(uri: Url, range: tree_sitter::Range) -> Symbol {
        let range = Range::new(
            Position::new(
                range.start_point.row as u32,
                range.start_point.column as u32,
            ),
            Position::new(range.end_point.row as u32, range.end_point.column as u32),
        );

        Symbol { uri, range }
    }
}

struct Data {
    labels: Mutex<HashMap<String, Symbol>>,
    fd: FileDepot,
}

impl Data {
    fn insert_label(&self, key: String, uri: &Url, range: tree_sitter::Range) {
        let mut data = self.labels.lock().unwrap();
        data.insert(key, Symbol::new(uri.clone(), range));
    }

    fn find_label(&self, key: &str) -> Option<Symbol> {
        let data = self.labels.lock().unwrap();
        match data.get(key) {
            Some(x) => Some(x.clone()),
            None => None,
        }
    }

    /*
    fn set_text(&self, s: &str) {
        let mut data = self.text.lock().unwrap();
        *data = s.to_string();
    }

    fn get_text(&self) -> String {
        let s = self.text.lock().unwrap();
        s.clone()
    }
    */

    fn new() -> Data {
        Data {
            labels: Mutex::new(HashMap::new()),
            fd: FileDepot::new(),
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
        Logger::log(&msg);

        let text = params.text_document.text.as_str();
        self.data.fd.insert(uri, Some(text.to_string()));

        let mut parser = Parser::new();
        parser
            .set_language(tree_sitter_devicetree::language())
            .unwrap();
        let tree = parser.parse(text, None).unwrap();
        let mut cursor = QueryCursor::new();

        let q = Query::new(
            tree_sitter_devicetree::language(),
            "(node label: (identifier)@id)",
        )
        .unwrap();
        let matches = cursor.matches(&q, tree.root_node(), text.as_bytes());
        for m in matches {
            let nodes = m.nodes_for_capture_index(0);
            for node in nodes {
                let label = node.utf8_text(text.as_bytes()).unwrap();
                let range = node.range();
                self.data.insert_label(label.to_string(), uri, range);
                let pos = range.start_point;
                Logger::log(&format!("NODE<{}>: {:?}, {}", node.kind(), label, pos.row));
            }
        }

        let q = Query::new(
            tree_sitter_devicetree::language(),
            "(preproc_include path: (string_literal)@id)",
        )
        .unwrap();
        let matches = cursor.matches(&q, tree.root_node(), text.as_bytes());
        for m in matches {
            let nodes = m.nodes_for_capture_index(0);
            for node in nodes {
                let label = node.utf8_text(text.as_bytes()).unwrap();
                let label = label.trim_matches('"');
                let range = node.range();
                let pos = range.start_point;
                let new_url = uri.join(label).unwrap();
                self.data.fd.insert(&new_url, None);
                Logger::log(&format!(
                    "INCLUDE<{}>: {}, {}",
                    node.kind(),
                    new_url,
                    pos.row
                ));
            }
        }

        self.data.fd.dump();
    }

    async fn goto_definition(
        &self,
        input: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let location = input.text_document_position_params.position;
        let location = Point::new(location.line as usize, location.character as usize);
        //let text = self.data.get_text();
        let uri = input.text_document_position_params.text_document.uri;
        let text = match self.data.fd.get_text(&uri) {
            Some(text) => text,
            None => return Ok(None),
        };
        let mut parser = Parser::new();
        parser
            .set_language(tree_sitter_devicetree::language())
            .unwrap();
        let tree = parser.parse(&text, None).unwrap();
        let node = tree
            .root_node()
            .named_descendant_for_point_range(location, location);
        // TODO: check if node type is reference
        match node {
            Some(node) => {
                let label = node.utf8_text(text.as_bytes()).unwrap();
                Logger::log(&format!(
                    "definintion for node <{}>: {}",
                    node.kind(),
                    label
                ));

                if let Some(point) = self.data.find_label(label) {
                    let pos = Location::new(point.uri, point.range);
                    return Ok(Some(GotoDefinitionResponse::Scalar(pos)));
                }
            }
            None => Logger::log(&format!("Node not found!",)),
        }

        Ok(None)
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        let msg = format!("Close file: {}", params.text_document.uri);
        Logger::log(&msg);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let msg = format!("Change file: {}", params.text_document.uri);
        Logger::log(&msg);
    }
    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        let msg = format!("Save file: {}", params.text_document.uri);
        Logger::log(&msg);
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    Logger::init();

    let (service, socket) = LspService::new(|client| Backend {
        client,
        data: Data::new(),
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}

use logger::log_message;
use logger::Logger;
use std::collections::HashMap;
use tower_lsp::jsonrpc::Error;
use tower_lsp::jsonrpc::Result;
#[allow(clippy::wildcard_imports)]
use tower_lsp::lsp_types::*;
use tower_lsp::Client;
use tower_lsp::{LanguageServer, LspService, Server};
use tree_sitter::Parser;
use tree_sitter::Point;
use utils::convert_range;

mod file_depot;
mod includes_depot;
mod labels_depot;
mod logger;
mod references_depot;
mod utils;
mod workspace;

#[cfg(test)]
mod functional_tests;

use workspace::Workspace;

struct Backend {
    data: Workspace,
    client: Option<Client>,
    process_neighbours: bool,
}

impl Backend {
    fn new(client: Client) -> Self {
        Backend {
            data: Workspace::new(),
            process_neighbours: true,
            client: Some(client),
        }
    }

    async fn get_includes_path(&self) -> String {
        let default = ".".to_string();

        let cfg_item = vec![ConfigurationItem {
            scope_uri: None,
            section: Some("dts-lsp".to_string()),
        }];

        let cfg = match self.client.clone() {
            None => return default,
            Some(x) => x.configuration(cfg_item).await,
        };

        info!("got cfg: {:?}", cfg);

        if let Ok(cfg) = cfg {
            let cfg = &cfg[0];
            let cfg = cfg.get("bindings_includes");
            if let Some(cfg) = cfg {
                return cfg.to_string();
            }
        }

        default
    }
}

#[tower_lsp::async_trait]
impl LanguageServer for Backend {
    async fn initialize(&self, params: InitializeParams) -> Result<InitializeResult> {
        let uri = if let Some(x) = params.root_uri {
            x
        } else {
            warn!("Can't get rootUri, using current directory");
            utils::current_url()?
        };
        self.data.fd.set_root_dir(&uri);

        Ok(InitializeResult {
            capabilities: ServerCapabilities {
                text_document_sync: Some(TextDocumentSyncCapability::Kind(
                    TextDocumentSyncKind::FULL,
                )),
                definition_provider: Some(OneOf::Left(true)),
                references_provider: Some(OneOf::Left(true)),
                rename_provider: Some(OneOf::Right(RenameOptions {
                    prepare_provider: Some(true),
                    work_done_progress_options: WorkDoneProgressOptions {
                        work_done_progress: None,
                    },
                })),
                ..ServerCapabilities::default()
            },
            ..Default::default()
        })
    }

    async fn initialized(&self, _: InitializedParams) {
        let x = self.get_includes_path().await;
        info!("include_path: {x}");

        info!("server initialized!");
    }

    async fn shutdown(&self) -> Result<()> {
        Ok(())
    }

    async fn did_open(&self, params: DidOpenTextDocumentParams) {
        let uri = &params.text_document.uri;

        info!("Open file: {uri}");

        let text = params.text_document.text.as_str();
        let mut includes = self.data.handle_file(uri, Some(text.to_string()));

        while let Some(new_url) = includes.pop() {
            let mut tmp = self.data.handle_file(&new_url, None);
            includes.append(&mut tmp);
        }

        self.data.fd.dump();
        self.data.ld.dump();
        self.data.rd.dump();
        self.data.id.dump();

        if self.process_neighbours {
            self.data.open_neighbours(uri);
        }
    }

    async fn goto_definition(
        &self,
        input: GotoDefinitionParams,
    ) -> Result<Option<GotoDefinitionResponse>> {
        let location = input.text_document_position_params.position;
        let location = Point::new(location.line as usize, location.character as usize);
        let uri = input.text_document_position_params.text_document.uri;
        let Some(text) = self.data.fd.get_text(&uri) else {
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

            let parent_kind = node.parent().map(|x| x.kind());
            let node_kind = node.kind();

            return match (node_kind, parent_kind) {
                ("identifier", Some("reference")) => {
                    let labels = self.data.ld.find_label(&uri, label);
                    let res: Vec<Location> = labels
                        .clone()
                        .into_iter()
                        .map(|x| Location::new(x.uri, x.range))
                        .collect();

                    match res.len() {
                        0 => Ok(None),
                        1 => Ok(Some(GotoDefinitionResponse::Scalar(res[0].clone()))),
                        _ => Ok(Some(GotoDefinitionResponse::Array(res))),
                    }
                }
                ("identifier", _) => match self.data.id.find_define(&uri, label) {
                    None => Ok(None),
                    Some(x) => {
                        let res = Location::new(x.uri, x.range);
                        Ok(Some(GotoDefinitionResponse::Scalar(res)))
                    }
                },
                _ => Ok(None),
            };
        }

        Ok(None)
    }

    async fn references(&self, params: ReferenceParams) -> Result<Option<Vec<Location>>> {
        let location = params.text_document_position.position;
        let location = Point::new(location.line as usize, location.character as usize);
        let uri = params.text_document_position.text_document.uri;

        let Some(text) = self.data.fd.get_text(&uri) else {
            warn!("No text found for file {uri}");
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

            if let (Some(parent), v) = (node.parent(), self.data.rd.find_references(&uri, label)) {
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

    async fn prepare_rename(
        &self,
        params: TextDocumentPositionParams,
    ) -> Result<Option<PrepareRenameResponse>> {
        let location = params.position;
        let location = Point::new(location.line as usize, location.character as usize);
        let uri = params.text_document.uri;
        let Some(text) = self.data.fd.get_text(&uri) else {
            warn!("No text found for file {uri}");
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
            let name = node.utf8_text(text.as_bytes()).unwrap();
            let range = node.range();

            let labels = self.data.ld.find_label(&uri, name);
            let references = self.data.rd.find_references(&uri, name);

            if labels.len() + references.len() > 0 {
                return Ok(Some(PrepareRenameResponse::Range(convert_range(&range))));
            }
        }

        Err(Error::new(tower_lsp::jsonrpc::ErrorCode::InvalidParams))
    }

    async fn rename(&self, params: RenameParams) -> Result<Option<WorkspaceEdit>> {
        let location = params.text_document_position.position;
        let location = Point::new(location.line as usize, location.character as usize);
        let uri = params.text_document_position.text_document.uri;
        let Some(text) = self.data.fd.get_text(&uri) else {
            warn!("No text found for file {uri}");
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
            let name = node.utf8_text(text.as_bytes()).unwrap();
            let mut result: HashMap<Url, Vec<TextEdit>> = HashMap::new();

            let labels = self.data.ld.find_label(&uri, name);
            let references = self.data.rd.find_references(&uri, name);

            for label in &labels {
                self.data.ld.rename(&label.uri, name, &params.new_name);
            }

            for reference in &references {
                self.data.rd.rename(&reference.uri, name, &params.new_name);
            }

            // TODO: check that labels in single file are ordered from bottom to top
            for symbol in labels.iter().chain(references.iter()) {
                let e = result.entry(symbol.uri.clone()).or_default();
                e.push(TextEdit::new(symbol.range, params.new_name.clone()));
            }

            for (uri, edits) in &result {
                self.data.fd.apply_edits(uri, edits);
            }

            if !result.is_empty() {
                return Ok(Some(WorkspaceEdit {
                    changes: Some(result),
                    document_changes: None,
                    change_annotations: None,
                }));
            }
        }

        Err(Error::new(tower_lsp::jsonrpc::ErrorCode::InvalidParams))
    }

    async fn did_close(&self, params: DidCloseTextDocumentParams) {
        info!("Close file: {}", params.text_document.uri);
    }

    async fn did_change(&self, params: DidChangeTextDocumentParams) {
        let uri = &params.text_document.uri;

        info!("Change file: {uri}");

        let text = &params.content_changes[0].text;
        let mut includes = self.data.handle_file(uri, Some(text.to_string()));

        while let Some(new_url) = includes.pop() {
            let mut tmp = self.data.handle_file(&new_url, None);
            includes.append(&mut tmp);
        }
    }

    async fn did_save(&self, params: DidSaveTextDocumentParams) {
        info!("Save file: {}", params.text_document.uri);
    }
}

#[tokio::main]
async fn main() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| {
        let handle = tokio::runtime::Handle::current();
        Logger::set(Logger::Lsp(handle, client.clone()));
        Backend::new(client)
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}

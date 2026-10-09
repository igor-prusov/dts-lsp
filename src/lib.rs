#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::must_use_candidate)]

pub mod backend;
pub mod bench_utils;
pub mod config;
pub mod diagnostics;
pub mod file_depot;
pub mod includes_depot;
pub mod labels_depot;
pub mod logger;
pub mod references_depot;
pub mod utils;
pub mod workspace;

#[cfg(test)]
mod functional_tests;

use backend::Backend;
use logger::Logger;
use tower_lsp::{LspService, Server};

pub async fn run() {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(|client| {
        let handle = tokio::runtime::Handle::current();
        Logger::set(Logger::Lsp(handle.clone(), client.clone()));
        Backend::new(handle, client, config::get())
    });
    Server::new(stdin, stdout, socket).serve(service).await;
}

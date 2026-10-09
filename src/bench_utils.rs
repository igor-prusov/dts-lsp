use crate::backend::Backend;
use crate::config::Config;
use crate::logger::Logger;
use crate::workspace::Workspace;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::runtime::Handle;
use tower_lsp::lsp_types::{Position, Range, Url};

pub fn corpus_root() -> Url {
    let path = std::env::var("DTS_LSP_CORPUS_DIR").map_or_else(
        |_| PathBuf::from("target/bench-corpus/linux"),
        PathBuf::from,
    );
    let path = path.canonicalize().unwrap_or_else(|_| {
        panic!(
            "Benchmark corpus not found at {}. Run benches/setup.sh first",
            path.display()
        )
    });
    Url::from_file_path(path).unwrap()
}

pub fn make_backend_ext(
    handle: &Handle,
    root: &Url,
    full_scan: bool,
    process_neighbours: bool,
) -> Backend {
    Logger::set(Logger::Silent);
    let config: &'static Config = Box::leak(Box::new(Config {
        process_neighbours,
        full_scan,
        ..Default::default()
    }));
    let be = Backend {
        data: Workspace::new(handle.clone(), None, config),
        client: None,
        config,
    };
    be.data.fd.set_root_dir(root);
    be
}

pub fn make_backend(handle: &Handle, root: &Url) -> Backend {
    make_backend_ext(handle, root, true, true)
}

pub async fn scanned_backend(handle: &Handle, root: &Url) -> Backend {
    let be = make_backend(handle, root);
    be.data.full_scan().await;
    be
}

pub fn find_hot_label(be: &Backend) -> (String, Url, Position) {
    let mut by_name: HashMap<String, Vec<(Url, Range)>> = HashMap::new();
    for (name, uri, range) in be.data.ld.all() {
        by_name.entry(name).or_default().push((uri, range));
    }

    let mut candidates: Vec<_> = by_name.into_iter().collect();
    candidates.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then_with(|| a.0.cmp(&b.0)));
    for (_, occurrences) in &mut candidates {
        occurrences.sort_by(|a, b| a.0.cmp(&b.0));
    }

    let mut best: Option<(String, Url, Position, usize)> = None;
    for (name, occurrences) in candidates.into_iter().take(50) {
        let (uri, range) = occurrences[0].clone();
        let total = be.data.ld.find_label(&uri, &name).len()
            + be.data.rd.find_references(&uri, &name).len();
        if best.as_ref().is_none_or(|b| total > b.3) {
            best = Some((name, uri, range.start, total));
        }
    }

    let (name, uri, pos, total) = best.expect("no labels found in corpus");
    eprintln!("hot label: '{name}' ({total} symbols in component, file {uri})");
    (name, uri, pos)
}

#[cfg(feature = "walkdir")]
pub fn sample_dts_file(root: &Url) -> Url {
    let path = root.to_file_path().unwrap();
    let mut files: Vec<PathBuf> = walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(std::result::Result::ok)
        .map(|x| x.path().to_path_buf())
        .filter(|x| x.extension().is_some_and(|e| e == "dts"))
        .collect();
    files.sort();
    let file = files.first().expect("no .dts files in corpus");
    Url::from_file_path(file).unwrap()
}

pub fn alternative_name(name: &str) -> String {
    name.chars()
        .map(|c| if c == 'x' { 'y' } else { 'x' })
        .collect()
}

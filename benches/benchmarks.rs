use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use dts_lsp::bench_utils::{
    alternative_name, corpus_root, find_hot_label, make_backend, make_backend_ext, sample_dts_file,
    scanned_backend,
};
use std::cell::Cell;
use std::fs::read_to_string;
#[allow(clippy::wildcard_imports)]
use tower_lsp::lsp_types::*;
use tower_lsp::LanguageServer;

fn bench_full_scan(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let root = corpus_root();
    let handle = rt.handle().clone();

    let mut g = c.benchmark_group("workspace");
    g.sample_size(10);
    g.bench_function("full_scan_cold", |b| {
        b.to_async(&rt).iter_batched(
            || make_backend(&handle, &root),
            |be| async move {
                be.data.full_scan().await;
            },
            BatchSize::LargeInput,
        );
    });
    g.finish();
}

fn bench_open_neighbours(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let root = corpus_root();
    let handle = rt.handle().clone();
    let file = sample_dts_file(&root);
    let text = read_to_string(file.to_file_path().unwrap()).unwrap();

    let mut g = c.benchmark_group("workspace");
    g.sample_size(10);
    g.bench_function("open_file_and_neighbours", |b| {
        b.to_async(&rt).iter_batched(
            || {
                let be = make_backend_ext(&handle, &root, false, true);
                let params = DidOpenTextDocumentParams {
                    text_document: TextDocumentItem::new(
                        file.clone(),
                        "dts".to_owned(),
                        1,
                        text.clone(),
                    ),
                };
                (be, params)
            },
            |(be, params)| async move {
                be.did_open(params).await;
            },
            BatchSize::LargeInput,
        );
    });
    g.finish();
}

fn bench_hot_label(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let root = corpus_root();
    let handle = rt.handle().clone();
    let be = rt.block_on(scanned_backend(&handle, &root));
    let (name, uri, pos) = find_hot_label(&be);

    let tdpp = TextDocumentPositionParams {
        position: pos,
        text_document: TextDocumentIdentifier::new(uri.clone()),
    };

    let mut g = c.benchmark_group("hot_label");

    g.bench_function("prepare_rename", |b| {
        b.to_async(&rt).iter(|| be.prepare_rename(tdpp.clone()));
    });

    g.bench_function("references", |b| {
        let params = ReferenceParams {
            context: ReferenceContext {
                include_declaration: false,
            },
            text_document_position: tdpp.clone(),
            work_done_progress_params: WorkDoneProgressParams::default(),
            partial_result_params: PartialResultParams::default(),
        };
        b.to_async(&rt).iter(|| be.references(params.clone()));
    });

    let alt = alternative_name(&name);
    let toggle = Cell::new(false);
    g.bench_function("rename", |b| {
        b.to_async(&rt).iter(|| {
            let t = toggle.get();
            toggle.set(!t);
            let new_name = if t { name.clone() } else { alt.clone() };
            be.rename(RenameParams {
                new_name,
                text_document_position: tdpp.clone(),
                work_done_progress_params: WorkDoneProgressParams::default(),
            })
        });
    });

    g.finish();
}

fn bench_did_change(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();
    let root = corpus_root();
    let handle = rt.handle().clone();
    let be = rt.block_on(scanned_backend(&handle, &root));
    let (_, uri, _) = find_hot_label(&be);

    let text_a = be.data.fd.get_text(&uri).unwrap();
    let text_b = format!("{text_a}\n// bench change\n");

    let toggle = Cell::new(false);
    let mut g = c.benchmark_group("workspace");
    g.bench_function("did_change_churn", |b| {
        b.to_async(&rt).iter(|| {
            let t = toggle.get();
            toggle.set(!t);
            let text = if t { text_a.clone() } else { text_b.clone() };
            be.did_change(DidChangeTextDocumentParams {
                text_document: VersionedTextDocumentIdentifier::new(uri.clone(), 2),
                content_changes: vec![TextDocumentContentChangeEvent {
                    range: None,
                    range_length: None,
                    text,
                }],
            })
        });
    });
    g.finish();
}

criterion_group!(
    benches,
    bench_full_scan,
    bench_open_neighbours,
    bench_hot_label,
    bench_did_change
);
criterion_main!(benches);

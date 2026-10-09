# Benchmarking Plan for dts-lsp

Goal: catch performance degradations critical on real projects like the Linux kernel by benchmarking processing of a big tree of DTS files (full scan, rename, references, edits).

## 1. Corpus: realistic DTS tree
- Use a pinned snapshot of the Linux kernel's `arch/arm64/boot/dts` + `arch/arm/boot/dts` (~4-6k `.dts/.dtsi` files, deep include chains, heavy label/rename usage — exactly the real-world worst case).
- Pin to a specific kernel tag (e.g. `v6.12`) for reproducibility.
- Options: vendor a stripped tarball in `benches/corpus/`, or fetch+extract in a setup script (`benches/setup.sh`) with a hash check. Prefer fetch-on-demand to keep the repo small; cache in `target/bench-corpus/`.

## 2. Harness
- New bench module reusing the existing mock-backend pattern from `src/functional_tests.rs` (`make_backend`, `mock_initialize`, `mock_open`, `mock_rename`) — no real LSP client needed, measures pure processing cost.
- Two layers:
  - **Criterion benches** (`benches/` + `cargo bench`, `[dev-dependencies] criterion`) for fine-grained ops with statistical comparison (`--baseline`).
  - **End-to-end timing** of `dts-lsp` binary init on the corpus via a small bench binary or script (wall time + peak RSS via `/usr/bin/time -v`).

## 3. Scenarios to measure
1. **Cold full scan** — `mock_initialize` on the corpus root → measures `full_scan()` / `handle_files()` (parse + 4 tree-sitter queries per file + include resolution). This is the big-O hot spot (`workspace.rs:309`).
2. **Open file + neighbours** — `mock_open` on a `.dts` deep in an include chain → measures incremental parse + `open_neighbours`.
3. **Rename hot label** — pick a label with thousands of references (e.g. a common SoC label) → `prepare_rename` + `rename`, including edit generation across many files.
4. **Find references** — same hot label.
5. **did_change churn** — modify a widely-included `.dtsi`, measure re-invalidation/re-parse cascade (`ld/rd/id.invalidate`).
6. **Scaling check** — run cold scan on corpus subsets (500 / 2k / full) to detect non-linear behavior (likely suspects: lock contention in depots, per-file `Query::new` allocations at `workspace.rs:57`, `resolve_include` hitting disk per include).

## 4. Regression detection
- Store baseline results as JSON (Criterion's `target/criterion` or custom) in CI.
- CI job (`bench.yml`, on-demand or nightly — too heavy for every PR): run benches, fail if any scenario regresses > threshold (e.g. 20%), post comparison table as PR comment/artifact.
- Locally: `cargo bench -- --baseline master` for quick before/after.

## 5. Implementation order
1. `benches/setup.sh` — fetch/extract pinned kernel DTS subset.
2. Extract mock helpers from `functional_tests.rs` into a shared `test_utils` module (feature-gated or moved to a lib target).
3. `benches/benchmarks.rs` with Criterion: cold scan → rename → references → change.
4. Baseline run on master, commit results.
5. CI job with regression threshold + artifact upload.

## Usage (implemented)

```sh
# One-time corpus setup (Linux v6.12 DTS subset, ~4700 files)
./benches/setup.sh

# Run benchmarks
cargo bench --bench benchmarks

# Compare against saved baseline
cargo bench --bench benchmarks -- --baseline master
python3 scripts/compare_bench.py   # exits 1 if any scenario regresses > 20%

# Refresh baseline
cargo bench --bench benchmarks -- --save-baseline master
```

Override corpus location with `DTS_LSP_CORPUS_DIR`, kernel tag with `DTS_LSP_KERNEL_TAG`.

Scenarios (benches/benchmarks.rs):
- `workspace/full_scan_cold` — cold full scan of the corpus
- `workspace/open_file_and_neighbours` — did_open + neighbour processing
- `workspace/did_change_churn` — did_change on a file defining the hot label
- `hot_label/prepare_rename`, `hot_label/references`, `hot_label/rename` — ops on the label with most component symbols (deterministically selected)

CI: `.github/workflows/bench.yml` — nightly/dispatch compares against cached baseline and fails on >20% regression; pushes to master refresh the baseline.

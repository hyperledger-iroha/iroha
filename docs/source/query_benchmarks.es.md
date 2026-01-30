---
lang: es
direction: ltr
source: docs/source/query_benchmarks.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e930be751a4959cc3e6adf1e370e9dec710704cf0888ba54fbb4c4bb3a504079
source_last_modified: "2026-01-03T18:08:01.854245+00:00"
translation_last_reviewed: 2026-01-30
---

# Query Benchmark Suite

This page documents the micro-benchmarks added for Iroha queries, how they are
constructed, and how to interpret the results. The focus is on iterator-based
execution and client-path builder overhead, independent of networking.

## Where the benches live

- Core query benches: `crates/iroha_core/benches/queries.rs`
- Client-path builder benches (mocked executor): `crates/iroha_core/benches/queries_client.rs`

Both are Criterion benches with a manual entrypoint (`fn main`) to comply with
workspace lints.

## Methodology

- The benches construct an in-memory `State` and `World` directly (no network),
  then use `ValidQuery::execute` to obtain iterators over items.
- Result counts are consumed (with `count()` or by collecting then sorting) to
  ensure side effects are not optimized away; `criterion::black_box` prevents
  dead-code elimination.
- For client-path overhead, a `MockExec` implements `QueryExecutor` and feeds
  batches to `QueryBuilder`/`QueryBuilderExt` without network I/O, isolating the
  builder cost and typed downcasting path.

## Datasets

- Accounts: `build_state_with_accounts(n)` creates a single domain and `n`
  accounts (authority-only ownership).
- Assets: `build_state_with_assets(n_accounts, assets_per_account)` creates the
  above plus one or two assets per account.
- Domains: `build_state_with_domains(n)` creates `n` standalone domains.
- Asset Definitions: `build_state_with_asset_definitions(n)` creates a domain,
  an owner account, and `n` asset definitions.

## What’s measured

Core benches (`queries.rs`):
- FindAccounts
  - Iterate and count (1k, 10k)
  - Sort by ID (10k)
  - Simulated pagination over a full result vector (page = 100)
- FindAssets
  - Iterate and count (~10k assets)
  - Filter by a specific account
  - Filter by quantity threshold
- FindDomains
  - Iterate and count (5k)
  - Sort by ID (10k)
- FindAssetDefinitions
  - Iterate and count (10k)

Client-path benches (`queries_client.rs`):
- `QueryBuilder` + `execute_all()` with mocked `QueryExecutor`
  - 1k total, fetch_size = 100
  - 10k total, fetch_size = 500

These mimic end-user paths in the client SDK without network effects.

## Running

Build benches:

```
cargo build -p iroha_core --benches
```

Run specific benches:

```
# All query benches in iroha_core
cargo bench -p iroha_core --bench queries

# Client-path builder benches (mock executor)
cargo bench -p iroha_core --bench queries_client
```

Note: Criterion manages output under `target/criterion`. For stable comparisons,
fix CPU governor and isolate noisy background processes. Use multiple runs and
inspect the report variance.

## Interpreting Results

- Iterator cost scales with the number of items scanned; most benches expose
  linear behavior. Sorting adds `O(n log n)` cost; values are sensitive to key
  size and comparison complexity.
- Filtering on the iterator side demonstrates upper bounds for server-side
  filters. In practice, server-side predicates should prefer pre-filtering to
  reduce downstream processing.
- Pagination simulation over full vectors is a proxy to client-side chunking.
  Real systems should rely on server-side pagination (fetch_size) to avoid
  loading entire datasets.
- Client-path builder benches isolate the overhead of constructing and
  downcasting typed batches from `QueryOutputBatchBoxTuple`.

## Next Steps

- Add server-path benches for query handlers (hot predicates, sort keys).
- Add selection (projection) once server-side projections are reintroduced.
- Expand to triggers and blocks with synthetic state builders.

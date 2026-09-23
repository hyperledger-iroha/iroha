# Combined-source file-budget gate — 2026-09-23

On the existing `optimizations` checkout, `python3 scripts/check_source_file_budget.py`
initially failed with 279 findings across 12,804 checked files (168 configured
exceptions). Splitting the reserve tests into a code-adjacent file reduced the
current result to 278 findings across 12,805 checked files. The latest local diagnostic is
`target/first-release-source-budget-20260923.log`; it is not a release receipt.
The current in-progress merge contributes many grown and new files, so the
result cannot qualify an immutable candidate. No baseline or exception was
raised to make this check pass.

`crates/iroha_core/src/privacy_state.rs` still grew from its 14,920-line
baseline to 15,783 lines. The new reserve regressions now live in
`crates/iroha_core/src/smartcontracts/isi/asset/privacy_public_reserve_tests.rs`;
`core_numeric_mutation_tests.rs` is 2,949 lines and no longer appears in the
findings. The production privacy helpers need scoped extraction and a rerun.

The file-budget gate remains **OPEN** alongside full formatting, strict
Clippy, workspace, SDK and release validation. The findings must be resolved
against the final combined source, not waived by treating a current checkout
as a qualified candidate.

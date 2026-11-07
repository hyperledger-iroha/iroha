# Panic recovery source closure — 2026-11-05

The finite panic audit now seals the State lifecycle restoration helper called
by executor dispatch, along with its literal test module. A method call does
not create a Rust textual module edge, so both files are explicitly listed in
`CORE_RECOVERY_SUPPORT_PATHS`. The helper restores the previous lifecycle
origin, refuses capture after a panic and resumes unwinding; ordinary panic
reporting remains enabled. This repair changes coverage, not production
recovery behavior.

Two parametrized regression cases prove that the helper is inventoried without
an executor module declaration and that a same-count source change to either
the helper or its tests fails the guard. The refreshed inventory contains
1,394 files, including 1,348 Rust sources, with 111 `catch_unwind`, 30
`spawn_blocking`, 485 task-spawn and 10 upgrade-task sites. The two added catch
sites belong to the helper and its unwind test. Its SHA-256 is
`c457fbebef9e32a5d554fc4db1ce850fcbb281f8db6baa3cece0894bec65cd71`.

The independent source review checked changed control flow and new literal
source edges, including calls through existing recoverable worker wrappers;
unchanged lexical counts alone were not accepted. Its 13:12:35–13:13:04 UTC
capture was stable and reported no source-closure failures. Exact reviewed
hashes, diffs and source copies are retained externally.

## Validation

All 191 tests passed in 93.64 seconds with
`python -m pytest -q scripts/tests/panic_recovery_boundaries_test.py`, including
the complete guard in `test_panic_recovery_boundary_guard`. The final receipt
binds all 1,397 recorded inputs before and after the run; the maps are identical.

External artifact root:
`/Users/takemiyamakoto/Library/Caches/iroha-script-tests/runs/panic-final-20261105/`.

| Evidence | External artifact | SHA-256 |
| --- | --- | --- |
| Independent source review | [source-review.json](/Users/takemiyamakoto/Library/Caches/iroha-script-tests/runs/panic-final-20261105/independent-source-review-20261105T130315Z/source-review.json) | `4fd85b8dc337803755e6f859bd9e584a58dd6001fcddf741c75e0551c16cc68f` |
| Final test receipt | [receipt.json](/Users/takemiyamakoto/Library/Caches/iroha-script-tests/runs/panic-final-20261105/final-validation/receipt.json) | `e34cd6f864537df344b3b94962e2ee1e14f4e48bbfa1307d925614270a8e4a16` |

The retained [November 4 evidence](panic_recovery_takeover_20261104/README.md)
and [push/JWT recheck](panic_recovery_takeover_20261104/rechecks/20261104T051527Z-push-jwt/README.md)
remain unchanged. The previous inventory bytes are retained externally at
`final-validation/inventory.before`, SHA-256
`639627b563ae375bd5fcc183fd3fdeea0a7c10c480010a8406138e0a3c052786`.
No generated logs are added to the repository.

This establishes source closure only for the configured Torii, daemon,
shared-build, panic-hook, Core ZK and explicit Core executor/recovery support
scope. It does not
establish whole-Core or whole-State panic coverage. Other State worker
boundaries remain outside this scope, and no native execution, network,
release, settlement or current-State qualification follows from these checks.

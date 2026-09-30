# Panic boundary inventory review — 2026-09-30

The [previous exact inventory](panic_recovery_boundaries.inventory.txt) has SHA-256 `f97ce67ef76e88f1e8e6c1f12f3edd168a97dbcc542e1fe6949030234f1d3306`. It recorded 972 source and manifest files, 54 catch sites, 21 blocking spawn sites, 396 task spawn sites and 10 upgrade sites.

The current inventory records 973 files and SHA-256 `f6426f55748e54a3a64873968a4226f747d4fada5a301f3313a674d1850a48f3`. Catch, blocking spawn and upgrade counts are unchanged. Task spawn count becomes exactly 397: the sole added site is the test-only `mint_seed_consumption_races_a_launcher_that_removes_the_emptied_path` thread in `irohad::taira_runtime_signer`. Its 16 rounds retain a 30-second observer deadline, join every thread, and check erased-descriptor custody when the launcher removes the one-shot pathname.

The seven reviewed source rows are:

- `iroha_torii/src/routing.rs` and its new `routing/pipeline_preflight_fixture_tests.rs` leaf: a `cfg(test)` module owns two served-JSON/DTO fixture checks; no recovery sites change.
- `iroha_torii/src/tests/lib_runtime_handlers/part_5.rs` and `irohad/src/musubi_publication_service/finality.rs`: tests acknowledge the canonical result-bearing proposal constructor; recovery sites remain unchanged.
- `irohad/src/sccp_attestor.rs` and `sccp_attestor/keeper.rs`: async construction uses the existing recoverable blocking worker and joined result. The default current-thread, multithread and disabled-keeper regressions remain. A committed temporary reproducer that deliberately invoked the private synchronous constructor on an async worker was removed as its own comment required; it was not an executable-owner regression.
- `irohad/src/taira_runtime_signer.rs`: erased-record descriptor checks and the bounded launcher race above.

The current guard retains its closed source/manifest inventory, exact call counts, source-custody checks, semantic recovery requirements and negative controls. This review adds no allowance for future sites or unreviewed source changes and does not establish node or network qualification.

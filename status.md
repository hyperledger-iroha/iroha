# Status

Last updated: 2026-03-17

- Normalized the first-release ABI surface to v1 only across the core IVM/runtime path and the public ABI API: removed experimental syscall-policy scaffolding, made `IVM::load_program` reject non-v1 ABI headers, made pointer-ABI validation fail closed for non-v1 annotations, collapsed runtime ABI queries/endpoints/telemetry to singular `abi_version = 1` payloads, and updated the canonical runtime-upgrade/syscall docs to describe `added_*` fields as reserved-and-empty in this release.
- Extended the v1-only ABI cleanup through the runtime-upgrade SDK surface: Python, JavaScript, and Swift clients now reject runtime-upgrade manifests unless `abi_version = 1` and `added_syscalls`/`added_pointer_types` are empty, examples/tests were rewritten to match, and the remaining runtime-upgrade/nexus/threat-model docs were swept to remove stale multi-version ABI wording.
- Fixed `integration_tests/tests/sorting.rs` asset-definition sorting checks to filter results by the exact IDs registered in-test instead of `id().name().starts_with("xor_")`, matching current opaque `aid:*` asset-definition ID behavior.
- Verified previously failing integration tests now pass:
  - `correct_sorting_of_entities`
  - `metadata_sorting_descending`

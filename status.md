# Status

Last updated: 2026-03-17

- Fixed `integration_tests/tests/sorting.rs` asset-definition sorting checks to filter results by the exact IDs registered in-test instead of `id().name().starts_with("xor_")`, matching current opaque `aid:*` asset-definition ID behavior.
- Verified previously failing integration tests now pass:
  - `correct_sorting_of_entities`
  - `metadata_sorting_descending`

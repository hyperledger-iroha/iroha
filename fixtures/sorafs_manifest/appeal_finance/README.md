# Appeal-finance `CancelAssetLock` V1 fixtures

These fixtures freeze the first-release two-field cancellation instruction:

```text
CancelAssetLock {
    escrow_id,
    expected_remaining_amount,
}
```

`cancel_asset_lock_v1.{json,to}` is the canonical positive vector. The
`negative/` directory contains:

- the retired one-field layout, which both JSON and Norito decoders reject;
- a noncanonical quantity spelling, which the JSON decoder rejects;
- a canonical frame with trailing bytes, which the Norito decoder rejects;
- a zero expected amount, which is structurally codec-valid but rejected by
  native ledger execution before custody is changed.

Regenerate the payloads from the typed Rust model, then reseal the signed
reference-SDK inventory:

```sh
cargo run -p iroha_data_model --features test-fixtures \
  --bin cancel_asset_lock_fixtures
cargo run -p sorafs_manifest --bin generate_por_fixtures
python3 scripts/check_sorafs_reference_sdk_fixtures.py
```

Use `--check` with `cancel_asset_lock_fixtures` for a read-only byte-parity
check. Publication uses same-directory synchronized temporary files and atomic
rename, rejects symlinked/non-regular/hardlinked targets and parent traversal,
and requires the generated directory to contain only the closed path set plus
this README. Old development state carrying the one-field layout must be
discarded and reseeded; V1 has no compatibility decoder or migration.

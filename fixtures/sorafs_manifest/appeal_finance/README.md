# Appeal-finance `CancelAssetLock` V1 fixtures

These fixtures freeze the first-release two-field cancellation instruction:

```text
CancelAssetLock {
    escrow_id,
    expected_remaining_amount,
}
```

`EscrowId` remains a nominal Rust type and retains its distinct `IntoSchema`
identity. Its V1 codec is nevertheless transparent: the binary field is the
same `0x20 <32-byte hash>` sequence as `Hash`, and JSON is one scalar
`hash:...` literal. The redundant nested `0x21 0x20 <hash>` binary form and a
one-element JSON array are retired and rejected; there is no compatibility
decoder. With the transparent field, `cancel_asset_lock_v1.to` is exactly 85
bytes.

`cancel_asset_lock_v1.{json,to}` is the canonical positive vector. The
`negative/` directory contains:

- the retired one-field layout, which both JSON and Norito decoders reject;
- the retired nested `EscrowId` binary layout, whose redundant wrapper makes an
  86-byte frame, which Norito decoders reject;
- a noncanonical quantity spelling, which the JSON decoder rejects;
- a zero expected amount, which the Rust structural codec admits so reference
  validation can classify it as `SFS-VAL-001`; strict external SDK codecs reject
  zero at their public boundaries, and native ledger execution rejects it before
  custody is changed.

Strict decoder tests also append trailing bytes to the canonical frame and
require rejection; the closed eight-file payload set uses the retired nested
identifier as its dedicated 86-byte binary negative.

Rust, JavaScript/TypeScript, Python, Kotlin/JVM, mirrored Java Android, Swift,
and C# tests consume the same files at their public SDK boundaries. The
generated files are mandatory: an absent fixture is a failing test, not a
capability skip.

Regenerate the payloads from the typed Rust model, then reseal the signed
reference-SDK inventory:

```sh
cargo run --locked -p iroha_data_model --features test-fixtures,dev-tools \
  --bin cancel_asset_lock_fixtures
cargo run --locked -p sorafs_manifest --features dev-tools --bin generate_por_fixtures -- --write
python3 scripts/check_sorafs_reference_sdk_fixtures.py
```

Use `--check` with `cancel_asset_lock_fixtures` for a read-only byte-parity
check. Both generators also accept a separate `--output-dir PATH` argument
after Cargo's `--`; the aggregate generator interprets it as the
`sorafs_manifest` root, while the cancellation generator interprets it as the
`appeal_finance` directory. `ci/check_sorafs_fixtures.sh` copies the complete
manifest tree into two isolated temporary roots, regenerates both passes there,
validates each signed inventory, and requires byte-identical checked-in,
first-pass, and second-pass manifests.

Cancellation publication uses same-directory synchronized temporary files and
atomic rename, rejects symlinked/non-regular/hardlinked targets and parent
traversal, and requires the generated directory to contain only the closed
path set plus this README. Old development state carrying the one-field layout
must be discarded and reseeded; V1 has no compatibility decoder or migration.

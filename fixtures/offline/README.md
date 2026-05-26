# Offline Fixtures

Canonical Offline interop vectors generated from `iroha_data_model` types.

There is one first-release offline protocol. Any remaining `version` fields in
these fixtures are fixed schema, proof, storage, or transport-frame markers,
not alternate offline protocol variants.

Regenerate with:

```bash
cargo run -p iroha_data_model --features test-fixtures,transparent_api --bin offline_vectors
```

Verify with:

```bash
cargo run -p iroha_data_model --features test-fixtures,transparent_api --bin offline_vectors -- --check
```

# Offline V2 Fixtures

Canonical Offline V2 interop vectors generated from `iroha_data_model` types.

Regenerate with:

```bash
cargo run -p iroha_data_model --features test-fixtures,transparent_api --bin offline_v2_vectors
```

Verify with:

```bash
cargo run -p iroha_data_model --features test-fixtures,transparent_api --bin offline_v2_vectors -- --check
```

Sample governance DAG node wrapping a PoR proof.

Generated via:

```
cargo run -p sorafs_manifest --bin generate_por_fixtures
```

- `node_v1.to` — canonical Norito encoding.
- `node_v1.json` — summary including human-readable CIDs and signature metadata.

`node_v1` carries a deterministic Dilithium3/ML-DSA publisher signature so
reference validators can exercise non-Ed25519 governance key verification.

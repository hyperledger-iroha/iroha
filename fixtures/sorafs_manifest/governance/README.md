Deterministic governance DAG fixtures wrapping a PoR proof.

Generated via:

```
cargo run -p sorafs_manifest --bin generate_por_fixtures
```

- `node_v1.to` — canonical Norito encoding.
- `node_v1.json` — summary including exact 32-byte CIDs in hexadecimal and signature metadata.
- `dag_block_0_v1.to` / `.json` — signed root `GovernanceDagBlockV1`.
- `dag_block_1_v1.to` / `.json` — signed child block linked to block 0.
- `dag_head_v1.to` / `.json` — signed `GovernanceDagHeadV1` binding the two-block chain.
- `dag_head_validation_outcome_v1.json` — canonical successful
  `ValidationOutcomeV1` for the signed head-chain at `generated_at=123`.
  Its diagnostic input paths are the exact checked-in basenames
  `dag_head_v1.to`, `dag_block_0_v1.to`, and `dag_block_1_v1.to`.

`node_v1` carries a deterministic Dilithium3/ML-DSA publisher signature so
reference validators can exercise non-Ed25519 governance key verification. The
DAG nodes, blocks, and head use one deterministic Ed25519 fixture key and peer
identity. SDK parity tests
validate both the accepted root-to-head chain and rejected CID/reordering
vectors against these exact Norito bytes.

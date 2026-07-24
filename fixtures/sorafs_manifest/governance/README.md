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
- `dag_block_bad_signature_v1.to` / `.json` and its validation outcome —
  canonically encoded block bytes with a corrupted Ed25519 block signature;
  validation returns `SFS-SIG-006`.
- `dag_head_bad_signature_v1.to` / `.json` and its validation outcome —
  canonically encoded head bytes with a corrupted Ed25519 head signature;
  validation returns `SFS-SIG-007`.
- `dag_block_1_bad_predecessor_v1.to` / `.json`,
  `dag_head_bad_predecessor_v1.to` / `.json`, and the head-chain validation
  outcome — individually valid, consistently signed child/head bytes whose
  child references the wrong predecessor; validation returns `SFS-GOV-006`.
- `dag_block_trailing_bytes_v1.to` and its validation outcome — the canonical
  root block with one forbidden trailing byte; validation returns
  `SFS-NORITO-001`.

`node_v1` carries a deterministic Dilithium3/ML-DSA publisher signature so
reference validators can exercise non-Ed25519 governance key verification. The
DAG nodes, blocks, and head use one deterministic Ed25519 fixture key and peer
identity. SDK parity tests
validate the accepted root-to-head chain and exact CID, ordering, predecessor,
signature, and noncanonical-byte failures against these shared Norito bytes and
`ValidationOutcomeV1` JSON outcomes.

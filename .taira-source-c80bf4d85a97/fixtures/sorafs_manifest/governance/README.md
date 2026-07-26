Deterministic governance DAG fixtures wrapping a PoR proof.

Generated via:

```
cargo run -p sorafs_manifest --bin generate_por_fixtures
```

- `node_v1.to` — canonical Norito encoding.
- `node_v1.json` — summary including exact 32-byte CIDs in hexadecimal and signature metadata.
- `dag_block_0_v1.to` / `.json` — signed root `GovernanceDagBlockV1`.
- `dag_block_validation_outcome_v1.json` — canonical successful
  `ValidationOutcomeV1` for `dag_block_0_v1.to` at `generated_at=123`.
- `dag_block_cid_mismatch_validation_outcome_v1.json` — canonical
  `SFS-GOV-004` outcome when the root block is checked against the exact
  32-byte `0x7f` test CID.
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
- `dag_head_reordered_validation_outcome_v1.json` — canonical
  `SFS-GOV-006` outcome when the two valid blocks are supplied newest-to-root
  instead of root-to-head.
- `sdk_validation_inventory_v1.json` — schema-closed, path-sorted SHA-256 and
  byte-length inventory for all nine Governance DAG SDK Norito vectors, all
  eight canonical JSON payload sidecars, and all eight exact
  `ValidationOutcomeV1` goldens (25 signed artifacts plus the inventory
  itself). Every payload row declares `encoding: norito|json`; JSON sidecars
  use sorted two-space pretty JSON without a trailing newline. The inventory
  is signed under the same explicitly test-only Ed25519 key used by the
  deterministic Governance DAG fixtures. Its public-key fingerprint is pinned
  by `scripts/check_sorafs_governance_sdk_fixtures.py`; it is not a production
  release key or a substitute for the later globally signed multi-domain fixture
  inventory.

`node_v1` carries a deterministic Dilithium3/ML-DSA publisher signature so
reference validators can exercise non-Ed25519 governance key verification. The
DAG nodes, blocks, and head use one deterministic Ed25519 fixture key and peer
identity. SDK parity tests
compare the complete accepted block/head and exact CID, ordering, predecessor,
signature, and noncanonical-byte `ValidationOutcomeV1` objects against these
shared goldens. Verify the inventory without Cargo or network access with:

```sh
python3 scripts/check_sorafs_governance_sdk_fixtures.py
python3 -m pytest -q \
  scripts/tests/check_sorafs_governance_sdk_fixtures_test.py
```

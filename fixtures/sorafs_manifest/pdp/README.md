# SoraFS PDP Fixtures

Deterministic Proof-of-Data Possession fixtures for SF-13.

- `commitment_v1.to` / `.json` encode the canonical `PdpCommitmentV1`.
- `challenge_v1.to` / `.json` encode the canonical `PdpChallengeV1`.
- `proof_v1.to` / `.json` encode the canonical `PdpProofV1` bound to the challenge.
- `negative/duplicate_hot_leaf_challenge_v1.*` encodes a challenge rejected for duplicate hot-leaf indices.
- `negative/missing_signature_proof_v1.*` encodes a proof rejected for a missing provider signature.
- `negative/missing_segment_path_proof_v1.*` encodes a proof rejected for a missing segment Merkle path.
- `negative/missing_hot_leaf_path_proof_v1.*` encodes a proof rejected for a missing hot-leaf Merkle path.
- `negative/late_proof_v1.*` encodes a structurally valid proof rejected for missing the challenge deadline.
- `negative/wrong_provider_proof_v1.*` encodes a structurally valid proof rejected for provider mismatch.
- `negative/wrong_manifest_proof_v1.*` encodes a structurally valid proof rejected for manifest mismatch.
- `negative/wrong_path_proof_v1.*` encodes a structurally valid proof rejected for witness coverage mismatch.

Regenerate after PDP schema changes:

```sh
cargo run -p sorafs_manifest --bin generate_pdp_fixtures
```

Validate the positive bundle:

```sh
sorafs-validate pdp --commitment fixtures/sorafs_manifest/pdp/commitment_v1.to --challenge fixtures/sorafs_manifest/pdp/challenge_v1.to --proof fixtures/sorafs_manifest/pdp/proof_v1.to
sorafs-validate bundle --bundle fixtures/sorafs_manifest
```

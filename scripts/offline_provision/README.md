# Offline Provisioning Fixtures

`cargo xtask offline-provision` reads a JSON spec and emits deterministic
Android Provisioned proofs (the manifests signed by inspector kiosks). Each
entry produces Norito + JSON artefacts under the requested output directory so
SDKs, kiosks, and regression tests can share the same canonical payloads.

```
cargo xtask offline-provision \
    --spec scripts/offline_provision/spec.example.json \
    --output fixtures/offline_provision
```

Spec layout:

| Field | Description |
|-------|-------------|
| `inspector.private_key` | Default Ed25519 key used to sign manifests (overridden per-proof via `inspector_key` or `--inspector-key`). |
| `inspector.manifest_schema` / `.manifest_version` | Default schema label/version applied when individual proofs omit the fields. |
| `proofs[].label` | Directory name for the generated artefacts. |
| `proofs[].manifest_issued_at_ms` | Timestamp (ms) when the kiosk recorded the manifest. |
| `proofs[].counter` | Monotonic counter enforced per `{manifest_schema}::{device_id}` scope. |
| `proofs[].device_manifest` / `.device_manifest_file` | Metadata describing the inspected device. Must include `android.provisioned.device_id`. |
| `proofs[].challenge` / `.challenge_file` | Receipt challenge preimage fields (invoice, receiver, asset, amount, issued_at_ms, nonce). If `issued_at_ms` is omitted, `manifest_issued_at_ms` is used. |
| `proofs[].manifest_schema` / `.manifest_version` | Optional overrides for individual proofs. |
| `proofs[].inspector_key` | Optional per-proof key override. |

The tool writes:

- `<label>/proof.json` — Norito JSON form of `AndroidProvisionedProof`
- `<label>/proof.norito` — Norito-encoded bytes consumed by SDKs/POS tooling
- `provision_fixtures.manifest.json` — index covering every generated proof

`scripts/offline_topup/spec.example.json` references these proofs via the
`provisioned_proof_file` field so the generated allowance embeds the exact
inspector manifest as its `attestation_report`. Run `cargo xtask offline-provision`
before invoking `cargo xtask offline-topup` when exercising the provisioned flow.

See `spec.example.json` for a fully-populated template.

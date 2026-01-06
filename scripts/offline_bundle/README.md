# Offline bundle fixture generator

`scripts/offline_bundle/run.sh` wraps `cargo xtask offline-bundle` and emits
deterministic offline bundle fixtures (JSON + Norito) alongside the FASTPQ
witness requests for the sum/counter/replay circuits. The helper consumes a
single spec file and writes one directory per bundle under the requested output
root (defaults to `artifacts/offline_bundle`). Each directory contains:

- `bundle.json` / `bundle.norito` – the `OfflineToOnlineTransfer` payload
- `proof_sum_request.json`, `proof_counter_request.json`,
  `proof_replay_request.json` (optional) – witness payloads that can be fed
  directly into the prover APIs
- `bundle_summary.json` – per-bundle manifest with the Poseidon root and
  generated filenames for downstream automation

## Usage

```bash
scripts/offline_bundle/run.sh --spec scripts/offline_bundle/spec.example.json \
  --output artifacts/offline_bundle
```

`--spec` must point at a JSON document describing one or more bundles. Paths
inside the spec (for example the certificate references) are resolved relative
to the spec file. `--output` defaults to `artifacts/offline_bundle`.

## Spec format

The top-level document contains a `bundles` array. Each bundle entry uses the
following structure:

```json
{
  "label": "human-readable name; becomes the output directory",
  "bundle_id_hex": "32-byte hash in hex",
  "receiver": "account_id",
  "deposit_account": "account_id",
  "controller": "account_id (optional, defaults to first receipt sender)",
  "status": "settled|archived (optional)",
  "recorded_at_ms": 1730314876000,
  "recorded_at_height": 640001,
  "archived_at_height": null,
  "balance_proof": {
    "initial_asset": "asset_id",
    "initial_amount": "decimal string",
    "initial_commitment_hex": "hex",
    "resulting_commitment_hex": "hex",
    "claimed_delta": "decimal string",
    "zk_proof_hex": null
  },
  "receipts": [
    {
      "tx_id_hex": "hex",
      "from": "account_id",
      "to": "account_id",
      "asset": "asset_id",
      "amount": "decimal string",
      "issued_at_ms": 1730314876000,
      "invoice_id": "string",
      "platform_proof": {
        "kind": "apple_app_attest" | "android_marker_key" | "android_provisioned",
        "payload": {
          "...": "variant-specific fields"
        }
      },
      "sender_certificate": {
        "certificate_path": "../../fixtures/offline_allowance/ios-demo/certificate.json"
      },
      "sender_signature_hex": "ed25519 signature hex"
    }
  ],
  "aggregate_proof": {
    "proof_sum_hex": null,
    "proof_counter_hex": null,
    "proof_replay_hex": null,
    "metadata": {
      "fastpq.parameter_set": "fastpq-offline-v1",
      "fastpq.circuit.sum": "fastpq/offline_sum/v1",
      "fastpq.circuit.counter": "fastpq/offline_counter/v1",
      "fastpq.circuit.replay": "fastpq/offline_replay/v1"
    }
  },
  "counter_checkpoint": 40,
  "replay_log_head_hex": "hex",
  "replay_log_tail_hex": "hex"
}
```

All receipts in a bundle must reference the same certificate. The example spec
(in this directory) demonstrates how to point the generator at the allowance
fixtures emitted by `scripts/offline_topup`. The metadata block inside
`aggregate_proof` is optional; when present it is copied verbatim into the
`AggregateProofEnvelope` to help downstream tooling tag the proof payloads. Use
`fastpq.parameter_set` plus the `fastpq.circuit.*` keys to advertise the FASTPQ
parameter set and circuit identifiers used for the bundle proofs.

Platform proof payload notes:
- App Attest `key_id` must be canonical standard base64.
- Android marker `marker_public_key` is hex-encoded SEC1 P-256 bytes (65 bytes), and
  `marker_signature_hex` must be a raw 64-byte signature when present.

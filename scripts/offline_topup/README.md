# Offline Top-up Helper

This helper turns an allowance specification into canonical Norito fixtures and,
optionally, registers the allowance on-ledger. It backs roadmap item **OA1** and
is the canonical way to mint deterministic `OfflineWalletCertificate`s + sample
`RegisterOfflineAllowance` envelopes for SDK and Torii tests.

```
$ scripts/offline_topup/run.sh \
    --spec scripts/offline_topup/spec.example.json \
    --output artifacts/offline_demo \
    --register-config configs/offline-controller.toml \
    --register-mode blocking
```

## Spec schema (JSON)

```jsonc
{
  "operator": {
    "private_key": "ed25519:..."        // optional, per-entry overrides take precedence
  },
  "allowances": [
    {
      "label": "retail-demo",           // folder name, used in logs
      "controller": "ed0120…@wonderland",    // canonical multihash account id
      "allowance_asset": "xor#sora#ed0120…@sora",
      "amount": "250.00",
      "issued_at_ms": 1730314876000,
      "expires_at_ms": 1745900000000,
      "policy": {
        "max_balance": "500",
        "max_tx_value": "125",
        "expires_at_ms": 1745900000000
      },
      "spend_public_key": "ed0120...",       // multihash or `algo:hex`
      "metadata": {
        "ios.app_attest.team_id": "ABCD1234",
        "ios.app_attest.bundle_id": "tech.iroha.retail",
        "ios.app_attest.environment": "production"
      },
      "attestation_report_hex": "deadbeef",
      "blinding_hex": "ffeedd...optional",
      "operator_key": "ed25519:..."     // optional per entry override
    }
  ]
}
```

Controller and asset IDs must use the canonical multihash format (`ed0120…@domain`), just like the
account strings that appear on-chain. Spend keys accept either that multihash literal or the
`algo:hex` helper format used elsewhere in the spec.

Relative paths inside the spec (e.g., `attestation_report_file`, `metadata_file`)
are resolved relative to the spec file. All numeric strings use the same format
as `Numeric::from_str` (plain decimal with optional fractional part).

## Command-line flags

| Flag | Description |
|------|-------------|
| `--spec <path>` | Required; JSON spec as described above. |
| `--output <dir>` | Root directory for generated fixtures (default `artifacts/offline_topup`). |
| `--operator-key <algo:hex>` | Overrides `operator.private_key` and `allowance.operator_key`. Useful for CI secrets. |
| `--register-config <path>` | Optional client TOML; when provided the tool submits `RegisterOfflineAllowance` using that config. |
| `--register-mode <blocking|immediate>` | Choose whether submission waits for confirmation (`blocking`, default) or fire-and-forgets. |

Asset definitions intended for offline allowances must set metadata
`offline.enabled = true`. The ledger derives a deterministic escrow account for the asset
definition and records it in `settlement.offline.escrow_accounts` (creating the account if
needed). If `settlement.offline.escrow_required=true`, registration will reject missing escrow
bindings, so ensure the asset definition metadata is in place before submitting allowances.

Each allowance entry emits:

- `certificate.json` / `certificate.norito` — canonical encodings for `OfflineWalletCertificate`.
- `register_instruction.json` / `register_instruction.norito` — canonical `RegisterOfflineAllowance`.
- `summary.json` — helper metadata (`certificate_id`, commitment hex, metadata keys, file names, submission status).
- Optional on-ledger registration when `--register-config` is set.

The helper enforces the same invariants as the ledger (positive allowance
amounts, policy bounds, expiry windows) and now derives commitments using the
same Pedersen scheme enforced on-ledger:
`C = amount·G + H(asset_id, blinding)`. The `blinding_hex` seed is hashed with
the asset identifier to derive the randomizer so fixtures and ledger verification
stay in lockstep.

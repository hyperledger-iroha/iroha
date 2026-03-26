# Offline POS Provisioning Helper

This helper turns a provisioning specification into signed manifests that POS
clients can pin. Each manifest bundles the backend root keys a POS fleet must
trust along with rotation timing hints so devices know when to refresh the
bundle. Roadmap item **OA12.a** tracks this deliverable.

```
$ scripts/offline_pos_provision/run.sh \
    --spec scripts/offline_pos_provision/spec.example.json \
    --output artifacts/offline_pos_demo
```

For audit-friendly rotation drills, wrap the helper with
`scripts/offline_pos_provision/rotation_drill.sh --notes "merchant dress rehearsal"`
so every run records a timestamped entry in `<output>/rotation_drill.log`.

## Spec schema (JSON)

```jsonc
{
  "operator": {
    "account": "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
    "private_key": "ed25519:..."        // optional, --operator-key takes precedence
  },
  "manifests": [
    {
      "label": "retail-pos",            // folder name under --output
      "manifest_id": "pos-retail-v1",
      "sequence": 1,
      "published_at_ms": 1730314876000,
      "valid_from_ms": 1730314876000,
      "valid_until_ms": 1745900000000,
      "rotation_hint_ms": 1736000000000,  // optional refresh hint
      "operator_account": "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB", // optional override per manifest
      "operator_key": "ed25519:...",        // optional override per manifest
      "metadata": { "jurisdiction": "EU" }, // optional metadata inline or via metadata_file
      "roots": [
        {
          "label": "torii-admission",
          "role": "offline_admission_signer",
          "public_key": "ed0120...",       // or `public_key_file`
          "valid_from_ms": 1730314876000,
          "valid_until_ms": 1745900000000,
          "metadata": { "channel": "production" }
        }
      ]
    }
  ]
}
```

All account identifiers must use canonical Katakana i105 account IDs. Public keys
accept either the multihash literal (`ed0120...`) or the `algo:hex` helper used
elsewhere in the repo.

### Revocation bundles

Operators can optionally add a `revocation_bundles` section when they want to
ship deny-lists for POS devices that cannot reach Torii. Each entry mirrors the
manifest structure (`label`, `bundle_id`, `sequence`, `published_at_ms`,
optional operator overrides/metadata) and references either an inline
`revocations` array or a `revocations_file` containing entries with:

```jsonc
[
  {
    "verdict_id_hex": "0000000000000000000000000000000000000000000000000000000000000001",
    "issuer": "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB",
    "revoked_at_ms": 1730314876000,
    "reason": "device_compromised",
    "note": "Retail device reported stolen",
    "metadata": { "ticket": "INC-4012" }
  }
]
```

Reason slugs match the ledger (`unspecified`, `device_compromised`,
`device_lost_or_stolen`, `policy_violation`, `issuer_request`). The helper
writes `revocations.json` / `revocations.norito` bundles alongside
`summary.json`, and `pos_provision_fixtures.manifest.json` now records both the
manifest list and the revocation bundle outputs so CI/diff tooling can track
artefacts.

## Outputs

For each manifest entry the tool writes:

- `manifest.json` / `manifest.norito` — canonical encodings for
  `OfflinePosProvisionManifest`.
- `summary.json` — helper metadata (`manifest_id`, operator account, backend
  root list, on-disk file names) consumed by SDK tests.

The root output directory also gains a
`pos_provision_fixtures.manifest.json` manifest with relative paths to every
emitted artefact so CI can diff bundles deterministically (including
`revocation_bundles` whenever they are present).

Use `--operator-key ed25519:<hex>` when specs live in version control while
keys are injected via CI secrets.

### POS policy verification (OA11.1b)

`cargo xtask offline-pos-verify` now accepts provisioning manifests and allowance
summaries alongside a policy definition so POS clients can pin backend roots,
enforce verdict TTL/nonce metadata, and emit audit entries whenever a manifest
rotation occurs.

Sample policy definitions live next to the helper
(`scripts/offline_pos_provision/policy.example.json`) and cover:

- allowed operator accounts and manifest identifiers.
- pinned backend roots (`label` + optional `role`) with signature checks.
- grace periods for manifest validity and verdict refresh deadlines.
- whether verdict IDs/attestation nonces are required in allowance summaries.

Example verification run:

```
cargo xtask offline-pos-verify \
    --manifest artifacts/offline_pos_demo/retail-pos/manifest.json \
    --allowance-summary fixtures/offline_allowance/android-hms-demo/summary.json \
    --policy scripts/offline_pos_provision/policy.example.json \
    --audit-log artifacts/offline_pos_demo/audit.log
```

Add `--bundle <revocations.json>` and `--api-snapshot` to verify the optional
deny-list artefacts at the same time. Policy failures exit with a descriptive
error so CI can block releases when manifests or allowance metadata drift from
the agreed guards.

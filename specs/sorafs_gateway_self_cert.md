---
title: SoraFS Gateway Self-Certification Kit
summary: Operator workflow for signed gateway attestations bound to a verified aggregate release manifest (SF-5a).
---

# SoraFS Gateway Self-Certification Kit

`scripts/sorafs_gateway_self_cert.sh` verifies the candidate’s canonical
aggregate release manifest before it starts the gateway harness. The manifest
must be accompanied by the governed raw Ed25519 signature/public-key tuple and
must verify through an explicitly SHA256-pinned `sorafs-validate` binary. The
harness then produces the gateway report, signed Norito attestation, and
human-readable summary.

## Required runtime inputs

The wrapper has no fixture or identity defaults. Supply every item either as a
flag or through a runtime `key=value` config:

- `signing_key`: runtime Ed25519 private key used only for the gateway
  attestation.
- `signer`: admitted operator account recorded in the attestation.
- `gateway`: explicit regional gateway base URL; there is no fixture/default
  target.
- `release_manifest`: canonical aggregate `release_manifest.json`.
- `release_manifest_signature`: exactly 64 raw Ed25519 signature bytes.
- `release_manifest_public_key`: exactly 32 raw Ed25519 public-key bytes.
- `trusted_signing_fingerprint`: reviewed SHA256 of that raw public key.
- `release_manifest_verifier`: reviewed `sorafs-validate` executable.
- `trusted_release_manifest_verifier_sha256`: reviewed SHA256 of that exact
  executable.

Private keys, HSM credentials, gateway bearer tokens, and other runtime secrets
must not be committed. Start from
`fixtures/documentation/sorafs_gateway_self_cert.conf`, copy it into protected runtime
storage, and replace every placeholder.

## Run

```bash
scripts/sorafs_gateway_self_cert.sh \
  --config /run/sorafs-release/gateway-self-cert.conf
```

Command-line flags override config values. Unknown config keys and retired
signature-bundle fields are rejected. Supply the required gateway in the config
or with `--gateway`; use `--workspace` for the repository root and `--out` for a
new evidence directory.

Verification occurs before `cargo xtask sorafs-gateway-attest`. A bad signer
fingerprint, changed verifier digest, malformed key/signature, missing input, or
native verification failure prevents the harness from running.

## Output artifacts

The output directory contains:

| File | Description |
|------|-------------|
| `release_manifest.verify.json` | Payload-free receipt containing the manifest hash, signer fingerprint, verifier protocol/path/SHA256, and `signature_verified=true`. |
| `sorafs_gateway_report.json` | Canonical run report and scenario metrics. |
| `sorafs_gateway_attestation.to` | Signed Norito attestation envelope. |
| `sorafs_gateway_attestation.txt` | Human-readable change-ticket summary. |

Archive those files together with the aggregate release manifest, raw public
key, and raw signature. Do not archive the gateway attestation private key or
signer/HSM session material.

## Verify the gateway attestation

```bash
cargo xtask sorafs-gateway-attest --verify \
  artifacts/.../sorafs_gateway_attestation.to
```

The gateway-attestation signature is independent of the release-manifest
signature. Both receipts are required for a production self-cert packet.

## Output-path safety

The wrapper rejects symlinked/non-regular inputs, symlinked output directories,
symlinked parent components, and pre-existing release verification receipts. It
does not overwrite release authenticity evidence. Keep the output directory on
operator-controlled storage.

## Catalog-promotion evidence

The self-certification wrapper does not accept local denylist bundles or
generate policy-diff evidence. Gateway enforcement is bound only to the
threshold-approved, predecessor-linked catalog verified by
`scripts/check_sorafs_gateway_compliance_rollout_evidence.py`.

Collect the canonical `catalog_promotion` artifact separately. It must bind the
promoted and predecessor digests, contiguous sequence, bounded entry/change
inventories, unique threshold signers, and acknowledgements from at least two
gateways with distinct region and administration identities. Attach that
payload-free artifact and the exact observed `451` probe artifacts to the
governance packet; never attach source catalogs, denied payloads, tokens, or
credentials.

## Troubleshooting

- A release verification failure is a release blocker, not a reason to bypass
  self-certification. Re-acquire the governed public artifacts and re-confirm
  the reviewed signer/verifier digests.
- If a gateway scenario fails, review the harness output and follow
  `specs/sorafs_gateway_refusal_guidance.md`.
- Re-run independently for each regional gateway administrator and keep the
  receipts distinct.

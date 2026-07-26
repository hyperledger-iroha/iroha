---
title: SoraFS CI Templates & Release Hooks
summary: Reference pipelines for deterministic SoraFS artifacts, governed Ed25519 release authentication, and separate provenance attestations.
---

# SoraFS CI Templates & Release Hooks

Use `sorafs_cli` in ordinary build jobs to pack payloads, build content
manifests, verify CAR contents, and submit manifests. Do not expose release
signing keys to those jobs. Release authenticity is applied later to the
canonical aggregate `release_manifest.json` through an external Ed25519 signer
and a pinned native validator.

## Artifact preparation

A minimal provider-neutral job is:

```bash
sorafs_cli car pack \
  --input=payload.bin \
  --car-out=artifacts/payload.car \
  --plan-out=artifacts/chunk_plan.json \
  --summary-out=artifacts/car_summary.json

sorafs_cli manifest build \
  --summary=artifacts/car_summary.json \
  --manifest-out=artifacts/manifest.to \
  --manifest-json-out=artifacts/manifest.json

sorafs_cli proof verify \
  --manifest=artifacts/manifest.to \
  --car=artifacts/payload.car \
  --chunk-plan=artifacts/chunk_plan.json \
  --summary-out=artifacts/proof.json
```

Upload the CAR, chunk plan, content manifest, and proof summary as unsigned
candidate inputs. The release pipeline must reproduce them before promotion.

## Governed release authentication

After deterministic packaging, SBOM generation, vulnerability scanning, and
aggregate-manifest generation succeed, move only the canonical aggregate
manifest into the protected signing job:

```bash
scripts/release_sorafs_cli.sh \
  --manifest artifacts/release/release_manifest.json \
  --external-signer /run/sorafs-release/ed25519-sign \
  --signing-public-key /run/sorafs-release/release.ed25519.pub \
  --trusted-signing-fingerprint "$REVIEWED_SIGNER_SHA256" \
  --release-manifest-verifier /opt/iroha/bin/sorafs-validate \
  --trusted-release-manifest-verifier-sha256 "$REVIEWED_VERIFIER_SHA256"
```

The external signer contract is deliberately small: it receives the manifest
path and a new signature-output path and writes exactly 64 raw Ed25519 signature
bytes. A PKCS#11/HSM adapter should implement that contract. The wrapper copies
the governed 32-byte raw public key, checks its reviewed SHA256 fingerprint, and
verifies immutable snapshots with the exact `sorafs-validate` binary whose
SHA256 was supplied.

There are no fixture, key, fingerprint, or verifier defaults. Missing inputs,
unsafe path aliases, malformed key/signature sizes, fingerprint drift, verifier
digest drift, and native verification failure all block the release.

Archive these public artifacts together:

- `release_manifest.json`
- `release_manifest.ed25519.sig`
- `release_manifest.ed25519.pub`
- `release_manifest.verify.json`
- the reviewed signer fingerprint and native-verifier SHA256

Private keys, HSM credentials, bearer tokens, and signer sessions are
runtime-only and must never enter the artifact packet.

## Provenance is separate

GitHub OIDC/cosign or an equivalent provider can attest build provenance after
the release candidate is fixed. Verify that provenance against the pinned
workflow identity and issuer. A provenance bundle does not authenticate the
aggregate release manifest and cannot replace the governed Ed25519 signature or
native verification receipt.

## Gateway self-certification

`scripts/sorafs_gateway_self_cert.sh` requires the same aggregate manifest,
signature, public key, signer fingerprint, verifier path, and verifier SHA256
before the gateway harness starts. Populate a runtime copy of
`docs/examples/sorafs_gateway_self_cert.conf`; do not run the checked-in template
without replacing every placeholder.

## Fixture policy

The committed `fixtures/sorafs_manifest/ci_sample` directory is for deterministic
content-manifest, CAR, chunk-plan, proof, and negative-vector tests. It is not
release-signing evidence. Release jobs must not substitute those fixtures for a
production manifest, signer key, trusted fingerprint, native verifier, or
verification receipt.

Regenerate deterministic fixtures with the content commands above and compare
the resulting Norito/JSON bytes twice. Generate the release manifest only from
the final candidate inventory, and sign it only in the protected release job.

## Failure handling

- Stop promotion on any non-zero content verification, release-signing, or
  native-verification result.
- Treat a changed signer fingerprint or native-verifier digest as a governance
  event requiring fresh review.
- Preserve failed public evidence for investigation, but never archive runtime
  credentials or sensitive signer output.
- Keep rollback candidates and their raw Ed25519/native verification receipts
  available until every package channel has confirmed the new release.

---
lang: he
direction: rtl
source: docs/source/release_dual_track_automation_plan.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: b80dcaae8fcb8de805faa6100a5c2131070d2a0c6534ed568dcba83d0dbc1ea3
source_last_modified: "2026-01-04T10:50:53.642758+00:00"
translation_last_reviewed: 2026-01-30
---

# Dual-Track Release Automation Plan

This document records the actual build and release-automation boundary for the
Iroha 2 and Iroha 3 dual-track runbook. The generic dual-track helpers can stage
artifacts for S3, SoraFS, SoraNet, or another reviewed URI, but they are not the
canonical SoraFS CLI release workflow.

## Scope

1. Build profile-specific binary bundles and container-image archives.
2. Produce checksums and release manifests.
3. Obtain and verify detached Ed25519 signatures without accepting private keys.
4. Generate and validate publication plans.
5. Archive SBOM, provenance, registry, and promotion evidence supplied by the
   hosted release environment.

## Current tooling

- `scripts/build_release_bundle.sh` builds `iroha2`/`iroha3` binary bundles.
- `scripts/build_release_image.sh` saves the corresponding container image.
- `scripts/run_release_pipeline.py` coordinates changelog generation through
  `git-cliff`, both builders, `ci/dual_profile_matrix.sh`,
  `scripts/generate_release_manifest.py`,
  `scripts/release_manifest_signing.py`, and `scripts/publish_plan.py`.
- `ci/dual_profile_smoke.sh` and `ci/dual_profile_matrix.sh` provide the local
  profile smoke and bundle-comparison gates.
- `.github/workflows/workspace_release.yml` is the workspace release gate.
- `.github/workflows/sorafs-cli-release.yml` is the separate canonical SoraFS
  CLI/reference-validator release workflow. It does not invoke the generic
  dual-track pipeline.

No checked-in workflow currently invokes `scripts/run_release_pipeline.py`.
Hosted execution, artifact upload, and promotion therefore remain release-
operator actions and must not be claimed from a local dry run.

## Private-key-free Ed25519 signing contract

The bundle and image builders accept signing only through this complete option
set:

```text
--external-signer <reviewed-executable>
--signing-public-key <raw-32-byte-ed25519-public-key>
--trusted-signing-fingerprint <reviewed-lowercase-sha256>
```

The pipeline also requires the aggregate verifier contract:

```text
--release-manifest-verifier <reviewed-sorafs-validate-executable>
--trusted-release-manifest-verifier-sha256 <reviewed-lowercase-sha256>
```

The external signer receives the artifact path and a new signature-output path.
It must use the runtime PKCS#11/HSM session to write exactly 64 raw Ed25519
signature bytes. Private keys, PINs, bearer tokens, and provider configuration
remain outside the repository and artifact tree.

The builders pin the SHA-256 fingerprint of the exact raw public-key bytes,
reject unsafe or replaceable signing inputs, verify the detached Ed25519
signature before and after installation, and emit the public key as generated
Ed25519 SPKI PEM for independent verification. The per-artifact manifest records
`signature_algorithm=ed25519`, `public_key_format=pem-spki-ed25519`, and
`signer_fingerprint_sha256`.

After all rollout evidence has been attached, the pipeline applies the same
external-signer contract to the final aggregate `release_manifest.json`.
`scripts/release_manifest_signing.py` writes
`release_manifest.json.sig` as exactly 64 raw signature bytes and
`release_manifest.json.pub` as exactly 32 raw public-key bytes. It rejects
malformed, unsafe, symlinked, or hardlinked inputs, pins the reviewed raw-key
fingerprint, snapshots the exact reviewed verifier executable, checks its
SHA-256 and identity, invokes `sorafs-validate release-manifest`, and rechecks
the manifest, key, signature, and verifier identities after native execution.
There is no OpenSSL, PEM, RSA, or in-process fallback for this aggregate
contract. The publish-plan generator and validator reverify the aggregate
signature and record its digest, `public_key_format=raw-ed25519-32`,
fingerprint, verification mode, native-verifier path, and native-verifier
SHA-256. Production generation and validation require the independently
reviewed fingerprint and verifier path/digest again; neither value copied from
the plan is a trust anchor.

Unsigned local artifacts and plans are permitted only when
`--development-allow-unsigned-publish-plan` (pipeline) or
`--development-allow-unsigned-manifest` (publish-plan helper) is selected
explicitly. That mode is test/development-only and cannot be promotion
evidence.

## Evidence state

| Capability | Local source state | Evidence required for promotion |
|------------|--------------------|---------------------------------|
| Dual-profile bundle/image build | Implemented by the two builders and pipeline | Hosted Linux build and smoke records |
| Ed25519 signature validation | Implemented for artifacts and the final aggregate manifest with negative tests | HSM/PKCS#11 ceremony, reviewed fingerprint, rotation/revocation record |
| Checksums and manifests | Deterministic aggregate generation, signing, and publish-plan binding are implemented locally | Independent replay and signed publication inventory |
| SBOM and vulnerability scan | Not supplied by this generic local pipeline | Hosted SBOM plus zero critical/high scanner result |
| Provenance | Not supplied by this generic local pipeline | OIDC/cosign attestation and verification receipt |
| Registry/S3/SoraFS publication | Publication plan generation is implemented | Authorized upload, registry, and gateway receipts |
| Rollback/yank | Operator-controlled | Rehearsal record and retained previous signed release |

The SoraFS CLI workflow supplies its own SBOM, vulnerability, provenance, and
keyless cosign gates. Those results do not automatically certify artifacts made
by this generic dual-track pipeline.

## External dependencies

- Reviewed PKCS#11/HSM Ed25519 signer wrapper and runtime-only credentials.
- Out-of-band approval of the raw public-key fingerprint.
- Packaged `sorafs-validate` candidate plus independent approval of its exact
  executable path and lowercase SHA-256 digest.
- OIDC/cosign identity and transparency-log availability for provenance.
- Registry, bucket, or SoraFS publication authorization.
- Hosted build, install, scan, publication, rollback, and yank receipts.

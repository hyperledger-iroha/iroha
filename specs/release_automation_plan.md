# Iroha 3 Release Automation Plan

This document records the actual build and release-automation boundary for the
canonical Iroha 3 release runbook. The generic release helpers can stage
artifacts for S3, SoraFS, SoraNet, or another reviewed URI, but they are not the
canonical SoraFS CLI release workflow.

## Scope

1. Build canonical Iroha 3 binary bundles and container-image archives.
2. Produce checksums and release manifests.
3. Obtain and verify detached Ed25519 signatures without accepting private keys.
4. Generate and validate publication plans.
5. Archive SBOM, provenance, registry, and promotion evidence supplied by the
   hosted release environment.

## Current tooling

- `scripts/build_release_bundle.sh` builds canonical `iroha3` binary bundles.
- `scripts/build_release_image.sh` saves the corresponding container image.
- `scripts/run_release_pipeline.py` coordinates changelog generation through
  `git-cliff`, both builders, `ci/release_bundle_inventory.sh`,
  `scripts/generate_release_manifest.py`,
  `scripts/release_manifest_signing.py`, and `scripts/publish_plan.py`.
- `ci/release_bundle_smoke.sh` and `ci/release_bundle_inventory.sh` provide the
  local smoke and bundle-inventory gates.
- `.github/workflows/workspace_release.yml` is the workspace release gate.
- `.github/workflows/sorafs-cli-release.yml` is the separate canonical SoraFS
  CLI/reference-validator release workflow. It does not invoke the generic
  Iroha 3 release pipeline.

No checked-in workflow currently invokes `scripts/run_release_pipeline.py`.
Hosted execution, artifact upload, and promotion therefore remain release-
operator actions and must not be claimed from a local dry run.

## Private-key-free Ed25519 signing contract

The bundle and image builders do not sign artifacts. They emit the candidate
bytes, checksum sidecars, and unsigned per-build metadata consumed by the
aggregate manifest. The retired per-artifact OpenSSL/PEM signature format and
its builder CLI options are rejected.

After all candidates and rollout evidence have been attached, the pipeline
accepts this complete aggregate-signing option set:

```text
--external-signer <reviewed-executable>
--signing-public-key <raw-32-byte-ed25519-public-key>
--trusted-signing-fingerprint <reviewed-lowercase-sha256>
```

It also requires the verifier contract:

```text
--release-manifest-verifier <reviewed-sorafs-validate-executable>
--trusted-release-manifest-verifier-sha256 <reviewed-lowercase-sha256>
```

The external signer receives an owner-private snapshot of the final canonical
`release_manifest.json` and a new signature-output path. The
`authenticated_external_signer` provider must use its authenticated isolated
service with exact `software` backend to write exactly 64 raw Ed25519 signature
bytes. Private keys, bearer tokens, and provider configuration remain outside
the repository and artifact tree. `scripts/release_manifest_signing.py` writes
`release_manifest.json.sig` as exactly 64 raw signature bytes and
`release_manifest.json.pub` as exactly 32 raw public-key bytes. It rejects
malformed or noncanonical signatures, incompatible keys, unsafe permissions,
symlinks, hard links, and untrusted fingerprints; snapshots the exact reviewed
signer, manifest, and verifier; checks verifier SHA-256 and identity; invokes
`sorafs-validate release-manifest`; and rechecks every input after native
execution. There is no OpenSSL, PEM, RSA, or in-process fallback. The
publish-plan generator and validator reverify the aggregate signature and
record its digest, `public_key_format=raw-ed25519-32`, fingerprint, verification
mode, native-verifier path, and native-verifier SHA-256. Production generation
and validation require the independently reviewed fingerprint and verifier
path/digest again; neither value copied from the plan is a trust anchor.

Unsigned local artifacts and plans are permitted only when
`--development-allow-unsigned-publish-plan` (pipeline) or
`--development-allow-unsigned-manifest` (publish-plan helper) is selected
explicitly. That mode is test/development-only and cannot be promotion
evidence.

## Evidence state

| Capability | Local source state | Evidence required for promotion |
|------------|--------------------|---------------------------------|
| Iroha 3 bundle/image build | Implemented by the two builders and pipeline | Hosted Linux build and smoke records |
| Ed25519 signature validation | Implemented once for the final aggregate manifest with strict positive/negative tests | Independently administered external software-signing ceremony, reviewed fingerprint, rotation/revocation record |
| Checksums and manifests | Deterministic aggregate generation, signing, and publish-plan binding are implemented locally | Independent replay and signed publication inventory |
| SBOM and vulnerability scan | Not supplied by this generic local pipeline | Hosted SBOM plus zero critical/high scanner result |
| Provenance | Not supplied by this generic local pipeline | OIDC/cosign attestation and verification receipt |
| Registry/S3/SoraFS publication | Publication plan generation is implemented | Authorized upload, registry, and gateway receipts |
| Rollback/yank | Operator-controlled | Rehearsal record and retained previous signed release |

The SoraFS CLI workflow supplies its own SBOM, vulnerability, provenance, and
keyless cosign gates. Those results do not automatically certify artifacts made
by this generic Iroha 3 release pipeline.

## External dependencies

- Reviewed `authenticated_external_signer` Ed25519 adapter, exact `software`
  backend, independently administered runtime-only credentials, and
  `software-key-qualified` verification receipt.
- Out-of-band approval of the raw public-key fingerprint.
- Packaged `sorafs-validate` candidate plus independent approval of its exact
  executable path and lowercase SHA-256 digest.
- OIDC/cosign identity and transparency-log availability for provenance.
- Registry, bucket, or SoraFS publication authorization.
- Hosted build, install, scan, publication, rollback, and yank receipts.

The provider contract is intentionally custody-neutral. Iroha exposes no
HSM-specific adapter or qualification label; deployment-owned custody does not
alter the signed provider identity or promotion evidence.

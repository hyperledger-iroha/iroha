---
lang: az
direction: ltr
source: docs/source/release_artifact_selection.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: d3ea92fbfd7a44cd789ecf187e0edc0dcb33969d45836dd55af706424c66656b
source_last_modified: "2025-12-29T18:16:36.024185+00:00"
translation_last_reviewed: 2026-02-07
---

# Iroha Release Artifact Selection

This note clarifies which artifacts (bundles and container images) operators should deploy for each release profile.

## Profiles

- **iroha2 (Self-hosted networks)** — single-lane configuration matching `defaults/genesis.json` and `defaults/client.toml`.
- **iroha3 (SORA Nexus)** — Nexus multi-lane configuration using `defaults/nexus/*` templates.

## Bundles (Binaries)

Bundles are produced via `scripts/build_release_bundle.sh` with `--profile` set to `iroha2` or `iroha3`.

Each tarball contains:

- `bin/` — `irohad`, `iroha`, and `kagami` built with the deploy profile.
- `config/` — profile-specific genesis/client configuration (single vs. nexus). Nexus bundles include `config.toml` with lane and DA parameters.
- `PROFILE.toml` — metadata describing profile, config, version, commit, OS/arch, and enabled feature set.
- Metadata artefacts written alongside the tarball:
  - `<profile>-<version>-<os>.tar.zst`
  - `<profile>-<version>-<os>.tar.zst.sha256`
  - `<profile>-<version>-<os>.tar.zst.sig` containing a raw 64-byte Ed25519
    signature when the complete external-signer option set is supplied
  - `<profile>-<version>-<os>.tar.zst.pub` containing generated Ed25519 SPKI PEM
  - `<profile>-<version>-manifest.json` capturing the tarball path, hash,
    `signature_algorithm=ed25519`, `public_key_format=pem-spki-ed25519`, and
    the reviewed SHA-256 fingerprint of the exact raw public-key bytes

## Container Images

Container images are produced via `scripts/build_release_image.sh` with the same profile/config arguments.

Outputs:

- `<profile>-<version>-<os>-image.tar`
- `<profile>-<version>-<os>-image.tar.sha256`
- Ed25519 signature/public key (`*.sig`/`*.pub`) for promotable artifacts
- `<profile>-<version>-image.json` recording tag, image ID, hash, and signature metadata

## Aggregate release inventory

The final release directory also contains `release_manifest.json`,
`release_manifest.json.sig` (exactly 64 raw Ed25519 signature bytes), and
`release_manifest.json.pub` (exactly 32 raw Ed25519 public-key bytes). The
pipeline appends rollout evidence before signing this aggregate inventory.
Verify it against the independently reviewed raw-key fingerprint and pinned
native verifier before trusting its artifact paths or generating a production
publish plan:

```bash
RELEASE_MANIFEST_VERIFIER=/opt/iroha/bin/sorafs-validate
TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256=<reviewed-lowercase-sha256>

python3 scripts/release_manifest_signing.py verify \
  --manifest release_manifest.json \
  --signature release_manifest.json.sig \
  --public-key release_manifest.json.pub \
  --trusted-signing-fingerprint "$TRUSTED_SIGNING_FINGERPRINT" \
  --release-manifest-verifier "$RELEASE_MANIFEST_VERIFIER" \
  --trusted-release-manifest-verifier-sha256 \
    "$TRUSTED_RELEASE_MANIFEST_VERIFIER_SHA256"
```

The wrapper checks and snapshots the exact verifier executable, invokes
`sorafs-validate release-manifest`, and rechecks the manifest, raw key,
signature, verifier digest, and file identities after native execution.
`scripts/publish_plan.py generate` also requires the signed-manifest paths,
independently reviewed signing fingerprint, native-verifier path, and reviewed
verifier SHA-256. Validation requires the independent fingerprint and verifier
pins again; values recorded in the plan are metadata, not trust anchors. The
unsigned escape hatch,
`--development-allow-unsigned-manifest`, is test/development-only and never
valid for promotion.

## Selecting the correct artefact

1. Determine the deployment surface:
   - **SORA Nexus / multi-lane** -> use the `iroha3` bundle and image.
   - **Self-hosted single-lane** -> use the `iroha2` artefacts.
   - When in doubt, run `scripts/select_release_profile.py --network <alias>` or `--chain-id <id>`; the helper maps networks to the correct profile per `release/network_profiles.toml`.
2. Download the desired tarball and accompanying checksum, signature, public
   key, and manifest. Promoted artifacts must include all of them; unsigned
   local builds are development-only. Validate the checksum, compare the
   manifest's `signer_fingerprint_sha256` with the independently reviewed
   release-ticket fingerprint, confirm the generated public key is Ed25519,
   and verify the detached signature before unpacking:
   ```bash
   ARTIFACT=iroha3-<version>-linux.tar.zst
   MANIFEST=iroha3-<version>-manifest.json
   TRUSTED_SIGNING_FINGERPRINT=<reviewed-lowercase-sha256>

   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   test "$(jq -r '.artifacts[0].signature_algorithm' "$MANIFEST")" = ed25519
   test "$(jq -r '.artifacts[0].public_key_format' "$MANIFEST")" = pem-spki-ed25519
   test "$(jq -r '.artifacts[0].signer_fingerprint_sha256' "$MANIFEST")" \
     = "$TRUSTED_SIGNING_FINGERPRINT"
   ACTUAL_SIGNING_FINGERPRINT="$(
     openssl pkey -pubin -in "$ARTIFACT.pub" -outform DER |
       python3 -c 'import hashlib,sys; d=sys.stdin.buffer.read(); p=bytes.fromhex("302a300506032b6570032100"); assert len(d)==44 and d.startswith(p); print(hashlib.sha256(d[len(p):]).hexdigest())'
   )"
   test "$ACTUAL_SIGNING_FINGERPRINT" = "$TRUSTED_SIGNING_FINGERPRINT"
   openssl pkeyutl -verify -pubin -rawin \
     -inkey "$ARTIFACT.pub" \
     -in "$ARTIFACT" \
     -sigfile "$ARTIFACT.sig"
   ```
   The public-key fingerprint must come from the reviewed release ticket or
   another authenticated channel; a fingerprint copied only from the
   downloaded manifest does not establish trust.
3. Extract the bundle (`tar --use-compress-program=zstd -xf <tar>`) and place `bin/` in the deployment PATH. Apply local configuration overrides where necessary.
4. Load the container image with `docker load -i <profile>-<version>-<os>-image.tar` if using containerised deployments. Verify the hash/signature as above before loading.

## Validator host platform

First-release production voting-validator artifacts target Linux. macOS arm64
is also a source-bound Sumeragi v2 release-evidence host, but is not the
published production deployment artifact. Windows and other non-Unix builds
are restricted development or non-voting-observer surfaces: they do not
implement the complete crash-safe validator-storage contract, their complete
observer application path is not release-certified, and they must fail if
configured as a voting validator. Compile success alone is not validator
certification.

## Nexus configuration checklist

- `config/config.toml` must include `[nexus]`, `[nexus.lane_catalog]`, `[nexus.dataspace_catalog]`, and `[nexus.da]` sections.
- Confirm lane routing rules match governance expectations (`nexus.routing_policy`).
- Validate DA thresholds (`nexus.da`) and fusion parameters (`nexus.fusion`) align with council-approved settings.

## Single-lane configuration checklist

- `config/config.d` (if present) should contain only single-lane overrides—no `[nexus]` sections.
- Ensure `config/client.toml` references the intended Torii endpoint and peer list.
- Genesis should retain the canonical domains/assets for the self-hosted network.

## Tooling quick reference

- `scripts/build_release_bundle.sh --help`
- `scripts/build_release_image.sh --help`
- `scripts/run_release_pipeline.py --help`
- `scripts/release_manifest_signing.py --help`
- `scripts/publish_plan.py --help`
- `scripts/select_release_profile.py --list`
- `docs/source/sora_nexus_operator_onboarding.md` — end-to-end onboarding flow for Sora Nexus data-space operators once artefacts are selected.

The builders accept no private key. Reference signing requires a reviewed
PKCS#11/HSM wrapper through `--external-signer`, a raw 32-byte Ed25519 public
key through `--signing-public-key`, and its independently approved lowercase
SHA-256 fingerprint through `--trusted-signing-fingerprint`. Aggregate
production signing and publish-plan validation additionally require the
packaged `sorafs-validate` candidate and its independently approved exact
executable SHA-256. OIDC/cosign provenance, hosted scan results, publication
receipts, and rollback/yank evidence remain external promotion inputs.

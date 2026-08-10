# Iroha Release Artifact Selection

This note clarifies which artifacts (bundles and container images) operators should deploy for each release profile.

## Profiles

- **iroha2 (Self-hosted networks)** — single-lane configuration matching `defaults/genesis.json` and `defaults/client.toml`.
- **iroha3 (SORA Nexus)** — Nexus multi-lane configuration using `defaults/nexus/*` templates.

## Bundles (Binaries)

Bundles are produced via `scripts/build_release_bundle.sh` with `--profile` set to `iroha2` or `iroha3`.

Each tarball contains:

- `bin/` — `iroha3d`, `iroha`, and `kagami` built with the deploy profile.
- `config/` — profile-specific genesis/client configuration (single vs. nexus). Nexus bundles include `config.toml` with lane and DA parameters.
- `PROFILE.toml` — metadata describing profile, config, version, commit, OS/arch, and enabled feature set.
- Metadata artefacts written alongside the tarball:
  - `<profile>-<version>-<os>-<arch>.tar.zst`
  - `<profile>-<version>-<os>-<arch>.tar.zst.sha256`
  - `<profile>-<version>-<os>-<arch>-manifest.json` capturing the target
    triple, tarball path, and hash

## Container Images

Container images are produced via `scripts/build_release_image.sh` for the
explicit `linux/amd64` and `linux/arm64` platform matrix. The production
builder accepts only the `single` and `nexus` configurations, requires
reviewed prebuilt binaries and digest-pinned builder/runtime images, and builds
the closed context with network access disabled. Taira is intentionally not a
release-pipeline configuration.

Outputs:

- `<profile>-<version>-linux-<arch>-image.oci.tar`
- `<profile>-<version>-linux-<arch>-image.oci.tar.sha256`
- `<profile>-<version>-linux-<arch>-image.json` recording the exact platform,
  OCI graph, base-image digests, tool digests, and archive hash

The builders deliberately expose no signing interface. The retired
per-artifact OpenSSL/PEM signature format is not part of V1.

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
2. Download the desired tarball, checksum, per-build metadata manifest, and the
   signed aggregate-manifest tuple. Verify the aggregate tuple first using the
   command above. Then bind the artifact bytes to both manifests before
   unpacking:
   ```bash
   ARTIFACT=iroha3-<version>-linux-x86_64.tar.zst
   MANIFEST=iroha3-<version>-linux-x86_64-manifest.json

   sha256sum -c iroha3-<version>-linux-x86_64.tar.zst.sha256
   EXPECTED_SHA256="$(
     jq -er --arg artifact "$ARTIFACT" \
       '.artifacts[] | select(.path == $artifact) | .sha256' \
       release_manifest.json
   )"
   test "$(jq -er '.artifacts[0].sha256' "$MANIFEST")" = "$EXPECTED_SHA256"
   printf '%s  %s\n' "$EXPECTED_SHA256" "$ARTIFACT" | sha256sum -c -
   ```
   The aggregate public-key fingerprint must come from the reviewed release
   ticket or another authenticated channel; a fingerprint copied only from the
   downloaded manifest does not establish trust.
3. Extract the bundle (`tar --use-compress-program=zstd -xf <tar>`) and place `bin/` in the deployment PATH. Apply local configuration overrides where necessary.
4. Load the container image with
   `docker load -i <profile>-<version>-linux-<arch>-image.oci.tar` if using
   containerised deployments. Verify its hash and authenticated
   aggregate-manifest binding as above before loading.

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
- `specs/sora_nexus_operator_onboarding.md` — end-to-end onboarding flow for Sora Nexus data-space operators once artefacts are selected.

The builders accept no signing or private-key option. Aggregate production
signing uses the `authenticated_external_signer` provider through
`--external-signer` with the exact `software` backend, a raw 32-byte Ed25519
public key through `--signing-public-key`, and its independently approved
lowercase SHA-256 fingerprint through
`--trusted-signing-fingerprint`. Signing and publish-plan validation also
require the packaged `sorafs-validate` candidate and its independently approved
exact executable SHA-256. A verified V1 release is `software-key-qualified`.
OIDC/cosign provenance, hosted scan results, publication receipts, and
rollback/yank evidence remain external promotion inputs. A later HSM adapter
requires new HSM-backed deployment evidence and promotion signatures.

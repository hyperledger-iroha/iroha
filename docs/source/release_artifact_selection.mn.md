---
lang: mn
direction: ltr
source: docs/source/release_artifact_selection.md
status: complete
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
  - `<profile>-<version>-<os>.tar.zst.sig` and `.pub` (when `--signing-key` supplied)
  - `<profile>-<version>-manifest.json` capturing the tarball path, hash, and signature details

## Container Images

Container images are produced via `scripts/build_release_image.sh` with the same profile/config arguments.

Outputs:

- `<profile>-<version>-<os>-image.tar`
- `<profile>-<version>-<os>-image.tar.sha256`
- Optional signature/public key (`*.sig`/`*.pub`)
- `<profile>-<version>-image.json` recording tag, image ID, hash, and signature metadata

## Selecting the correct artefact

1. Determine the deployment surface:
   - **SORA Nexus / multi-lane** -> use the `iroha3` bundle and image.
   - **Self-hosted single-lane** -> use the `iroha2` artefacts.
   - When in doubt, run `scripts/select_release_profile.py --network <alias>` or `--chain-id <id>`; the helper maps networks to the correct profile per `release/network_profiles.toml`.
2. Download the desired tarball and accompanying manifest files. Validate the SHA256 hash and signature before unpacking:
   ```bash
   sha256sum -c iroha3-<version>-linux.tar.zst.sha256
   openssl dgst -sha256 -verify iroha3-<version>-linux.tar.zst.pub        -signature iroha3-<version>-linux.tar.zst.sig        iroha3-<version>-linux.tar.zst
   ```
3. Extract the bundle (`tar --use-compress-program=zstd -xf <tar>`) and place `bin/` in the deployment PATH. Apply local configuration overrides where necessary.
4. Load the container image with `docker load -i <profile>-<version>-<os>-image.tar` if using containerised deployments. Verify the hash/signature as above before loading.

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
- `scripts/select_release_profile.py --list`
- `docs/source/sora_nexus_operator_onboarding.md` — end-to-end onboarding flow for Sora Nexus data-space operators once artefacts are selected.

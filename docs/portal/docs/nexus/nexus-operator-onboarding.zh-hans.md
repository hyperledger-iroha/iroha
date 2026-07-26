---
lang: zh-hans
direction: ltr
source: docs/portal/docs/nexus/nexus-operator-onboarding.md
status: needs-review
generator: scripts/sync_docs_i18n.py
source_hash: c958f1044ce6ae35dde36629b55aa3880c3926e60349cb06e80efdd8a3f9211c
source_last_modified: "2025-12-31T15:58:47.310713+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-operator-onboarding
title: Sora Nexus data-space operator onboarding
description: Mirror of `docs/source/sora_nexus_operator_onboarding.md`, tracking the end-to-end release checklist for Nexus operators.
---

:::note Canonical Source
This page mirrors `docs/source/sora_nexus_operator_onboarding.md`. Keep both copies aligned until the localized editions arrive in the portal.
:::

# Sora Nexus Data-Space Operator Onboarding

This guide captures the end-to-end flow Sora Nexus data-space operators must follow once a release is announced. It complements the dual-track runbook (`docs/source/release_dual_track_runbook.md`) and the artefact selection note (`docs/source/release_artifact_selection.md`) by describing how to align downloaded bundles/images, manifests, and configuration templates with the global lane expectations before bringing a node online.

## Audience & prerequisites
- You have been approved by the Nexus Program and received your data-space assignment (lane index, data-space ID/alias, and routing policy requirements).
- You can access the signed release artefacts published by Release Engineering
  (tarballs, images, checksum sidecars, per-build metadata manifests, the signed
  aggregate `release_manifest.json`, its raw 64-byte Ed25519 signature and
  32-byte public key, and provenance bundles).
- You obtained the reviewed SHA-256 fingerprint of the exact raw 32-byte
  release-signing public key through an authenticated channel independent of
  the downloaded manifest.
- You obtained the packaged `sorafs-validate` candidate by direct path and its
  independently reviewed exact executable SHA-256.
- You have generated or received production key material for your validator/observer role (Ed25519 node identity; BLS consensus key + PoP for validators; plus any confidential feature toggles).
- You can reach the existing Sora Nexus peers that will bootstrap your node.

## Step 1 — Confirm the release profile
1. Identify the network alias or chain ID you were given.
2. Run `scripts/select_release_profile.py --network <alias>` (or `--chain-id <id>`) on a checkout of this repository. The helper consults `release/network_profiles.toml` and prints the profile to deploy. For Sora Nexus the response must be `iroha3`. For any other value, stop and contact Release Engineering.
3. Note the version tag the release announcement referenced (e.g. `iroha3-v3.2.0`); you will use it to fetch artefacts and manifests.

## Step 2 — Retrieve and validate artefacts
1. Download the `iroha3` bundle (`<profile>-<version>-<os>.tar.zst`) and its
   companion `.sha256` and `<profile>-<version>-manifest.json` files. Download
   `<profile>-<version>-image.json` as well when deploying a container. Also
   download `release_manifest.json`, `release_manifest.json.sig`, and
   `release_manifest.json.pub`.
2. Verify the final aggregate inventory before trusting any path or hash it
   contains:
   ```bash
   TRUSTED_SIGNING_FINGERPRINT=<reviewed-lowercase-sha256>
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
   The aggregate `.pub` is exactly 32 raw Ed25519 bytes; it is not PEM.
   The wrapper pins the verifier digest and identity, invokes
   `sorafs-validate release-manifest`, then rechecks the manifest, key,
   signature, and verifier. Production publication-plan generation and
   validation re-run this check, require the independently reviewed signing and
   verifier pins, and bind themselves to the exact aggregate-manifest SHA-256.
   An inventory or plan marked `development-unsigned` is not promotable.
3. Validate the checksum and bind the per-build metadata to the already
   authenticated aggregate manifest before unpacking:
   ```bash
   ARTIFACT=iroha3-<version>-linux.tar.zst
   MANIFEST=iroha3-<version>-manifest.json

   sha256sum -c "$ARTIFACT.sha256"
   EXPECTED_SHA256="$(
     jq -er --arg artifact "$ARTIFACT" \
       '.artifacts[] | select(.path == $artifact) | .sha256' \
       release_manifest.json
   )"
   test "$(jq -er '.artifacts[0].sha256' "$MANIFEST")" = "$EXPECTED_SHA256"
   printf '%s  %s\n' "$EXPECTED_SHA256" "$ARTIFACT" | sha256sum -c -
   ```
   The aggregate fingerprint supplied in step 2 remains the independent trust
   anchor. Builders emit no per-artifact key or signature sidecars.
4. Inspect `PROFILE.toml` inside the tarball and the JSON manifests to confirm:
   - `profile = "iroha3"`
   - The `version`, `commit`, and `built_at` fields match the release announcement.
   - The OS/architecture match your deployment target.
5. If you use the container image, repeat the checksum and authenticated
   aggregate-manifest binding for `<profile>-<version>-<os>-image.tar`, then
   confirm the image ID recorded in `<profile>-<version>-image.json`.

## Step 3 — Stage configuration from templates
1. Extract the bundle and copy `config/` to the location where the node will read its configuration.
2. Treat the files under `config/` as templates:
   - Replace `public_key`/`private_key` with your production Ed25519 keys. Remove private keys from disk if the node will source them from an HSM; update the config to point at the HSM connector instead.
   - Adjust `trusted_peers`, `network.address`, and `torii.address` so they reflect your reachable interfaces and the bootstrap peers you were assigned.
   - Update `client.toml` with the operator-facing Torii endpoint (including TLS configuration if applicable) and the credentials you provision for operational tooling.
3. Keep the chain ID provided in the bundle unless Governance explicitly instructs otherwise—the global lane expects a single canonical chain identifier.
4. Plan to start the node with the Sora profile flag: `irohad --sora --config <path>`. The configuration loader will reject SoraFS or multi-lane settings when the flag is absent.

## Step 4 — Align data-space metadata and routing
1. Edit `config/config.toml` so the `[nexus]` section matches the data-space catalogue the Nexus Council provided:
   - `lane_count` must equal the total lanes enabled in the current epoch.
   - Every entry in `[[nexus.lane_catalog]]` and `[[nexus.dataspace_catalog]]` must contain a unique `index`/`id` and the agreed aliases. Do not delete the existing global entries; add your delegated aliases if the council assigned additional data-spaces.
   - Ensure each dataspace entry includes `fault_tolerance (f)`; lane-relay committees are sized at `3f+1`.
2. Update `[[nexus.routing_policy.rules]]` to capture the policy you were given. The default template routes governance instructions to lane `1` and contract deployments to lane `2`; append or modify rules so traffic destined for your data-space is forwarded to the correct lane and alias. Coordinate with Release Engineering before changing rule order.
3. Review `[nexus.da]`, `[nexus.da.audit]`, and `[nexus.da.recovery]` thresholds. Operators are expected to keep the council-approved values; only adjust them if an updated policy was ratified.
4. Record the final configuration in your operations tracker. The dual-track release runbook requires attaching the effective `config.toml` (with secrets redacted) to the onboarding ticket.

## Step 5 — Pre-flight validation
1. Run the built-in configuration validator before joining the network:
   ```bash
   ./bin/irohad --sora --config config/config.toml --trace-config
   ```
   This prints the resolved configuration and fails early if catalogue/routing entries are inconsistent or if genesis and config disagree.
2. If you deploy containers, run the same command inside the image after loading it with `docker load -i <profile>-<version>-<os>-image.tar` (remember to include `--sora`).
3. Check logs and `--trace-config` output for lane/data-space validation warnings. If any appear, revisit Step 4 so the effective catalog, aliases, and routing rules match the council-approved topology.
4. Execute your local smoke procedure (e.g., submit a `FindNetworkStatus` query with `iroha_cli`, confirm telemetry endpoints expose `nexus_lane_state_total`, and verify streaming keys are rotated or imported as required).

## Step 6 — Cutover and hand-off
1. Store the verified per-build manifests and checksum sidecars plus
   `release_manifest.json`, `release_manifest.json.sig`, and
   `release_manifest.json.pub` in the release ticket so auditors can reproduce
   your checks.
2. Notify Nexus Operations that the node is ready to be introduced; include:
   - Node identity (peer ID, hostnames, Torii endpoint).
   - Effective lane/data-space catalogue and routing policy values.
   - Hashes of the binaries/images you verified.
3. Coordinate the final peer admission (gossip seeds and lane assignment) with `@nexus-core`. Do not join the network until you receive approval; Sora Nexus enforces deterministic lane occupancy and requires an updated admissions manifest.
4. After the node is live, update your runbooks with any overrides you introduced and note the release tag so the next iteration can start from this baseline.
5. Attach the external PKCS#11/HSM signing-ceremony record, OIDC/cosign
   provenance verification, vulnerability-scan result, registry/publication
   receipt, and rollback/yank rehearsal. These hosted records remain open until
   Release Engineering supplies them; local verification cannot synthesize
   them.

## Reference checklist
- [ ] Release profile validated as `iroha3`.
- [ ] Aggregate signature plus bundle/image hashes verified.
- [ ] Keys, peer addresses, and Torii endpoints updated to production values.
- [ ] Nexus lane/dataspace catalogue and routing policy match council assignment.
- [ ] Configuration validator (`irohad --sora --config … --trace-config`) passes without warnings.
- [ ] Aggregate signature, manifests, and checksums archived in the onboarding ticket and Ops notified.

For broader context on Nexus migration phases and telemetry expectations, review [Nexus transition notes](./nexus-transition-notes).

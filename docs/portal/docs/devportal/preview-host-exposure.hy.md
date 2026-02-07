---
lang: hy
direction: ltr
source: docs/portal/docs/devportal/preview-host-exposure.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 82939c8aa73add3f3817490ab1a24bef5388c2fc5a00d19d76e4be6a3fa9c559
source_last_modified: "2025-12-29T18:16:35.111267+00:00"
translation_last_reviewed: 2026-02-07
id: preview-host-exposure
title: Preview host exposure guide
sidebar_label: Preview host exposure
description: Publish and verify the beta preview host before sending invites.
---

The DOCS‑SORA roadmap requires every public preview to ride on the same
checksum‑verified bundle that reviewers exercise locally. Use this runbook
after reviewer onboarding (and the invite approval ticket) are complete to put
the beta preview host online.

## Prerequisites

- Reviewer onboarding wave approved and logged in the preview tracker.
- Latest portal build present under `docs/portal/build/` and checksum
  verified (`build/checksums.sha256`).
- SoraFS preview credentials (Torii URL, authority, private key, submitted
  epoch) stored either in environment variables or a JSON config such as
  [`docs/examples/sorafs_preview_publish.json`](../../../examples/sorafs_preview_publish.json).
- DNS change ticket opened with the desired hostname (`docs-preview.sora.link`,
  `docs.iroha.tech`, etc.) plus on-call contacts.

## Step 1 – Build and verify the bundle

```bash
cd docs/portal
export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
npm ci
npm run build
./scripts/preview_verify.sh --build-dir build
```

The verify script refuses to continue when the checksum manifest is missing or
tampered with, keeping every preview artefact audited.

## Step 2 – Package the SoraFS artefacts

Convert the static site into a deterministic CAR/manifest pair. `ARTIFACT_DIR`
defaults to `docs/portal/artifacts/`.

```bash
./scripts/sorafs-pin-release.sh \
  --alias docs-preview.sora \
  --alias-namespace docs \
  --alias-name preview \
  --pin-label docs-preview \
  --skip-submit

node scripts/generate-preview-descriptor.mjs \
  --manifest artifacts/checksums.sha256 \
  --archive artifacts/sorafs/portal.tar.gz \
  --out artifacts/sorafs/preview-descriptor.json
```

Attach the generated `portal.car`, `portal.manifest.*`, descriptor, and checksum
manifest to the preview wave ticket.

## Step 3 – Publish the preview alias

Re-run the pin helper **without** `--skip-submit` once you are ready to expose
the host. Supply either the JSON config or explicit CLI flags:

```bash
./scripts/sorafs-pin-release.sh \
  --alias docs-preview.sora \
  --alias-namespace docs \
  --alias-name preview \
  --pin-label docs-preview \
  --config ~/secrets/sorafs_preview_publish.json
```

The command writes `portal.pin.report.json`,
`portal.manifest.submit.summary.json`, and `portal.submit.response.json`, which
must ship with the invite evidence bundle.

## Step 4 – Generate the DNS cutover plan

```bash
node scripts/generate-dns-cutover-plan.mjs \
  --dns-hostname docs.iroha.tech \
  --dns-zone sora.link \
  --dns-change-ticket DOCS-SORA-Preview \
  --dns-cutover-window "2026-03-05 18:00Z" \
  --dns-ops-contact "pagerduty:sre-docs" \
  --manifest artifacts/sorafs/portal.manifest.to \
  --cache-purge-endpoint https://cache.api/purge \
  --cache-purge-auth-env CACHE_PURGE_TOKEN \
  --out artifacts/sorafs/portal.dns-cutover.json
```

Share the resulting JSON with Ops so the DNS switch references the exact
manifest digest. When reusing an earlier descriptor as the rollback source,
append `--previous-dns-plan path/to/previous.json`.

## Step 5 – Probe the deployed host

```bash
npm run probe:portal -- \
  --base-url=https://docs-preview.sora.link \
  --expect-release="$DOCS_RELEASE_TAG"
```

The probe confirms the served release tag, CSP headers, and signature metadata.
Repeat the command from two regions (or attach curl output) so auditors can see
that the edge cache is warm.

## Evidence bundle

Include the following artefacts in the preview wave ticket and refer to them in
the invite email:

| Artefact | Purpose |
|----------|---------|
| `build/checksums.sha256` | Proves the bundle matches the CI build. |
| `artifacts/sorafs/portal.tar.gz` + `portal.manifest.to` | Canonical SoraFS payload + manifest. |
| `portal.pin.report.json`, `portal.manifest.submit.summary.json`, `portal.submit.response.json` | Shows the manifest submission + alias binding succeeded. |
| `artifacts/sorafs/portal.dns-cutover.json` | DNS metadata (ticket, window, contacts), route promotion (`Sora-Route-Binding`) summary, the `route_plan` pointer (plan JSON + header templates), cache purge info, and rollback instructions for Ops. |
| `artifacts/sorafs/preview-descriptor.json` | Signed descriptor tying the archive + checksum together. |
| `probe` output | Confirms the live host advertises the expected release tag. |

Once the host is live, follow the [preview invite playbook](./public-preview-invite.md)
to distribute the link, log invites, and monitor telemetry.

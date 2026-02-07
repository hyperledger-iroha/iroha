---
id: observability
lang: zh-hans
direction: ltr
source: docs/portal/docs/devportal/observability.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Portal Observability & Analytics
sidebar_label: Observability
description: Telemetry, release tagging, and verification automation for the developer portal.
---

The DOCS-SORA roadmap requires analytics, synthetic probes, and broken-link
automation for every preview build. This note documents the plumbing that now
ships with the portal so operators can wire monitoring without leaking visitor
data.

## Release tagging

- Set `DOCS_RELEASE_TAG=<identifier>` (falls back to `GIT_COMMIT` or `dev`) when
  building the portal. The value is injected into `<meta name="sora-release">`
  so probes and dashboards can distinguish deployments.
- `npm run build` emits `build/release.json` (written by
  `scripts/write-checksums.mjs`) describing the tag, timestamp, and optional
  `DOCS_RELEASE_SOURCE`. The same file is bundled into preview artefacts and
  referenced by the link checker report.

## Privacy-preserving analytics

- Configure `DOCS_ANALYTICS_ENDPOINT=<https://collector.example/ingest>` to
  enable the lightweight tracker. Payloads contain `{ event, path, locale,
  release, ts }` with no referrer or IP metadata, and `navigator.sendBeacon`
  is used whenever possible to avoid blocking navigations.
- Control sampling with `DOCS_ANALYTICS_SAMPLE_RATE` (0–1). The tracker stores
  the last-sent path and never emits duplicate events for the same navigation.
- The implementation lives in `src/components/AnalyticsTracker.jsx` and is
  mounted globally through `src/theme/Root.js`.

## Synthetic probes

- `npm run probe:portal` issues GET requests against common routes
  (`/`, `/norito/overview`, `/reference/torii-swagger`, etc.) and verifies the
  `sora-release` meta tag matches `--expect-release` (or
  `DOCS_RELEASE_TAG`). Example:

```bash
PORTAL_BASE_URL="https://docs.staging.sora" \
DOCS_RELEASE_TAG="preview-42" \
npm run probe:portal -- --expect-release=preview-42
```

Failures are reported per path, making it easy to gate CD on probe success.

## Broken-link automation

- `npm run check:links` scans `build/sitemap.xml`, ensures every entry maps to a
  local file (checking `index.html` fallbacks), and writes
  `build/link-report.json` containing the release metadata, totals, failures,
  and the SHA-256 fingerprint of `checksums.sha256` (exposed as `manifest.id`)
  so every report can be tied back to the artefact manifest.
- The script exits non-zero when a page is missing, so CI can block releases on
  stale or broken routes. Reports cite the candidate paths that were attempted,
  which helps trace routing regressions back to the docs tree.

## Grafana dashboard & alerts

- `dashboards/grafana/docs_portal.json` publishes the **Docs Portal Publishing**
  Grafana board. It ships the following panels:
  - *Gateway Refusals (5m)* uses `torii_sorafs_gateway_refusals_total` scoped by
    `profile`/`reason` so SREs can detect bad policy pushes or token failures.
  - *Alias Cache Refresh Outcomes* and *Alias Proof Age p90* track
    `torii_sorafs_alias_cache_*` to prove fresh proofs exist before a DNS cut
    over.
  - *Pin Registry Manifest Counts* plus the *Active Alias Count* stat mirror the
    pin-registry backlog and total aliases so governance can audit each release.
  - *Gateway TLS Expiry (hours)* highlights when the publishing gateway’s TLS
    cert approaches expiry (alert threshold at 72 h).
  - *Replication SLA Outcomes* and *Replication Backlog* keep an eye on
    `torii_sorafs_replication_*` telemetry to ensure all replicas meet the GA
    bar after publishing.
- Use the built-in template variables (`profile`, `reason`) to focus on the
  `docs.sora` publishing profile or investigate spikes across all gateways.
- PagerDuty routing uses the dashboard panels as evidence: alerts named
  `DocsPortal/GatewayRefusals`, `DocsPortal/AliasCache`, and
  `DocsPortal/TLSExpiry` fire when the corresponding series breach their
  thresholds. Link the alert’s runbook to this page so on-call engineers can
  replay the exact Prometheus queries.

## Putting it together

1. During `npm run build`, set the release/analytics environment variables and
   let the post-build step emit `checksums.sha256`, `release.json`, and
   `link-report.json`.
2. Run `npm run probe:portal` against the preview hostname with
   `--expect-release` wired to the same tag. Save the stdout for the publishing
   checklist.
3. Run `npm run check:links` to fail fast on broken sitemap entries and archive
   the generated JSON report together with the preview artefacts. CI drops the
   latest report at `artifacts/docs_portal/link-report.json` so governance can
   download the evidence bundle straight from the build logs.
4. Forward the analytics endpoint to your privacy-preserving collector (Plausible,
   self-hosted OTEL ingest, etc.) and ensure sampling rates are documented per
   release so dashboards interpret counts correctly.
5. CI already wires these steps through the preview/deploy workflows
   (`.github/workflows/docs-portal-preview.yml`,
   `.github/workflows/docs-portal-deploy.yml`), so local dry runs only need to
   cover secret-specific behaviour.

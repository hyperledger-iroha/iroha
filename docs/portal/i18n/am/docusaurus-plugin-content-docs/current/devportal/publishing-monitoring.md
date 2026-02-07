---
id: publishing-monitoring
lang: am
direction: ltr
source: docs/portal/docs/devportal/publishing-monitoring.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Publishing & Monitoring
sidebar_label: Publishing & Monitoring
description: Capture the end-to-end monitoring flow for SoraFS portal releases so DOCS-3c has deterministic probes, telemetry, and evidence bundles.
---

Roadmap item **DOCS-3c** requires more than a packaging checklist: after every
SoraFS publish we must continuously prove that the developer portal, Try it
proxy, and gateway bindings remain healthy. This page documents the monitoring
surface that accompanies the [deployment guide](./deploy-guide.md) so CI and on
call engineers can exercise the same checks that Ops uses to enforce the SLO.

## Pipeline recap

1. **Build and sign** – follow the [deployment guide](./deploy-guide.md) to run
   `npm run build`, `scripts/preview_wave_preflight.sh`, and the Sigstore +
   manifest submission steps. The preflight script emits `preflight-summary.json`
   so every preview carries build/link/probe metadata.
2. **Pin and verify** – `sorafs_cli manifest submit`, `cargo xtask soradns-verify-binding`,
   and the DNS cutover plan provide deterministic artefacts for governance.
3. **Archive evidence** – store the CAR summary, Sigstore bundle, alias proof,
   probe output, and `docs_portal.json` dashboard snapshots under
   `artifacts/sorafs/<tag>/`.

## Monitoring channels

### 1. Publishing monitors (`scripts/monitor-publishing.mjs`)

The new `npm run monitor:publishing` command wraps the portal probe, Try it
proxy probe, and binding verifier into a single CI-friendly check. Provide a
JSON config (checked into CI secrets or `configs/docs_monitor.json`) and run:

```bash
cd docs/portal
npm run monitor:publishing -- \
  --config ../../configs/docs_monitor.json \
  --json-out ../../artifacts/docs_monitor/$(date -u +%Y%m%dT%H%M%SZ).json \
  --evidence-dir ../../artifacts/sorafs/preview-2026-02-14/monitoring
```

Add `--prom-out ../../artifacts/docs_monitor/monitor.prom` (and optionally
`--prom-job docs-preview`) to emit Prometheus text-format metrics suitable for
Pushgateway uploads or direct Prometheus scrapes in staging/production. The
metrics mirror the JSON summary so SLO dashboards and alert rules can track
portal, Try it, binding, and DNS health without parsing the evidence bundle.

Example config with required knobs and multiple bindings:

```json
{
  "portal": {
    "baseUrl": "https://docs-preview.sora.link",
    "paths": ["/", "/devportal/try-it", "/reference/torii-swagger"],
    "expectRelease": "preview-2026-02-14",
    "checkSecurity": true,
    "expectedSecurity": {
      "csp": "default-src 'self'; connect-src https://tryit-preview.sora",
      "permissionsPolicy": "fullscreen=()",
      "referrerPolicy": "strict-origin-when-cross-origin"
    }
  },
  "tryIt": {
    "proxyUrl": "https://tryit-preview.sora",
    "samplePath": "/proxy/v1/accounts/ih58.../assets?limit=1",
    "method": "GET",
    "timeoutMs": 7000,
    "token": "${TRYIT_BEARER}",
    "metricsUrl": "https://tryit-preview.sora/metrics"
  },
  "bindings": [
    {
      "label": "portal",
      "bindingPath": "../../artifacts/sorafs/portal.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/portal.manifest.json"
    },
    {
      "label": "openapi",
      "bindingPath": "../../artifacts/sorafs/openapi.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/openapi.manifest.json"
    },
    {
      "label": "portal-sbom",
      "bindingPath": "../../artifacts/sorafs/portal-sbom.gateway.binding.json",
      "alias": "docs-preview.sora.link",
      "hostname": "docs-preview.sora.link",
      "proofStatus": "ok",
      "manifestJson": "../../artifacts/sorafs/portal-sbom.manifest.json"
    }
  ],

  "dns": [
    {
      "label": "docs-preview CNAME",
      "hostname": "docs-preview.sora.link",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    },
    {
      "label": "docs-preview canonical",
      "hostname": "igjssx53t4ayu3d5qus5o6xtp2f5dvka5rewr6xgscpmh3x4io4q.gw.sora.id",
      "recordType": "CNAME",
      "expectedRecords": ["docs-preview.sora.link.gw.sora.name"]
    }
  ]
}
```

The monitor writes a JSON summary (S3/SoraFS friendly) and exits non‑zero when
any probe fails, making it suitable for Cron jobs, Buildkite steps, or
Alertmanager webhooks. Passing `--evidence-dir` persists `summary.json`,
`portal.json`, `tryit.json`, and `binding.json` alongside a `checksums.sha256`
manifest so governance reviewers can diff the monitor results without having to
re-run the probes.

> **TLS guardrail:** `monitorPortal` rejects `http://` base URLs unless you set
> `allowInsecureHttp: true` in the config. Keep production/staging probes on
> HTTPS; the opt-in exists solely for local previews.

Each binding entry runs `cargo xtask soradns-verify-binding` against the captured
`portal.gateway.binding.json` bundle (and optional `manifestJson`) so alias,
proof status, and content CID stay aligned with the published evidence. The
optional `hostname` guard confirms the alias-derived canonical host matches the
gateway host you intend to promote, preventing DNS cutovers that drift from the
recorded binding.

The optional `dns` block wires DOCS-7’s SoraDNS rollout into the same monitor.
Each entry resolves a hostname/record-type pair (for example the
`docs-preview.sora.link` → `docs-preview.sora.link.gw.sora.name` CNAME) and
confirms the answers match `expectedRecords` or `expectedIncludes`. The second
entry in the snippet above hard-codes the canonical hashed hostname produced by
`cargo xtask soradns-hosts --name docs-preview.sora.link`; the monitor now proves
both the human-friendly alias and the canonical hash (`igjssx53…gw.sora.id`)
resolve to the pinned pretty host. This makes DNS promotion evidence automatic:
the monitor will fail if either host drifts, even when the HTTP bindings still
staple the right manifest.

### 2. OpenAPI version manifest guard

DOCS-2b’s “signed OpenAPI manifest” requirement now ships an automated guard:
`ci/check_openapi_spec.sh` calls `npm run check:openapi-versions`, which invokes
`scripts/verify-openapi-versions.mjs` to cross-check
`docs/portal/static/openapi/versions.json` with the actual Torii specs and
manifests. The guard verifies that:

- Every version listed in `versions.json` has a matching directory under
  `static/openapi/versions/`.
- Each entry’s `bytes` and `sha256` fields match the on-disk spec file.
- The `latest` alias mirrors the `current` entry (digest/size/signature metadata)
  so the default download cannot drift.
- Signed entries reference a manifest whose `artifact.path` points back to the
  same spec and whose signature/public key hex values match the manifest.

Run the guard locally whenever you mirror a new spec:

```bash
cd docs/portal
npm run check:openapi-versions
```

Failure messages include the stale-file hint (`npm run sync-openapi -- --latest`)
so portal contributors know how to refresh the snapshots. Keeping the guard in
CI prevents portal releases where the signed manifest and the published digest
fall out of sync.

### 2. Dashboards & alerts

- **`dashboards/grafana/docs_portal.json`** – primary board for DOCS-3c. Panels
  track `torii_sorafs_gateway_refusals_total`, replication SLA misses, Try it
  proxy errors, and probe latency (`docs.preview.integrity` overlay). Export the
  board after every release and attach it to the operations ticket.
- **Try it proxy alerts** – Alertmanager rule `TryItProxyErrors` fires on
  sustained `probe_success{job="tryit-proxy"}` drops or
  `tryit_proxy_requests_total{status="error"}` spikes.
- **Gateway SLO** – `DocsPortal/GatewayRefusals` ensures alias bindings continue
  to advertise the pinned manifest digest; escalations link to the
  `cargo xtask soradns-verify-binding` CLI transcript captured during publish.

### 3. Evidence trail

Each monitoring run should append:

- `monitor-publishing` evidence bundle (`summary.json`, per-section files, and
  `checksums.sha256`).
- Grafana screenshots for the `docs_portal` board over the release window.
- Try it proxy change/rollback transcripts (`npm run manage:tryit-proxy` logs).
- Alias verification output from `cargo xtask soradns-verify-binding`.

Store these under `artifacts/sorafs/<tag>/monitoring/` and link them in the
release issue so the audit trail survives after CI logs expire.

## Operational checklist

1. Run the deployment guide through Step 7.
2. Execute `npm run monitor:publishing` with production configuration; archive
   the JSON output.
3. Capture Grafana panels (`docs_portal`, `TryItProxyErrors`,
   `DocsPortal/GatewayRefusals`) and attach them to the release ticket.
4. Schedule recurring monitors (recommended: every 15 minutes) pointing at the
   production URLs with the same config to satisfy the DOCS-3c SLO gate.
5. During incidents, re-run the monitor command with `--json-out` to record
   before/after evidence and attach it to the postmortem.

Following this loop closes DOCS-3c: the portal build flow, publishing pipeline,
and monitoring stack now live in a single playbook with reproducible commands,
sample configs, and telemetry hooks.

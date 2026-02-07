---
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/portal-publish-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: dd5fff7302924f71ca19593cbbcc29352c00f286ab5bc555d4654e2dc43c3daa
source_last_modified: "2026-01-22T16:26:46.525444+00:00"
translation_last_reviewed: 2026-02-07
id: portal-publish-plan
title: Docs Portal → SoraFS Publish Plan
sidebar_label: Portal Publish Plan
description: Step-by-step checklist for shipping the docs portal, OpenAPI, and SBOM bundles via SoraFS.
---

:::note Canonical Source
Mirrors `docs/source/sorafs/portal_publish_plan.md`. Update both copies when the workflow changes.
:::

Roadmap item DOCS-7 requires every docs artefact (portal build, OpenAPI spec,
SBOMs) to flow through the SoraFS manifest pipeline and serve via `docs.sora`
with `Sora-Proof` headers. This checklist stitches the existing helpers together
so Docs/DevRel, Storage, and Ops can run the release without hunting through
multiple runbooks.

## 1. Build & Package Payloads

Run the packaging helper (skip options are available for dry-runs):

```bash
./ci/package_docs_portal_sorafs.sh \
  --out artifacts/devportal/sorafs/$(date -u +%Y%m%dT%H%M%SZ) \
  --sign \
  --sigstore-provider=github-actions \
  --sigstore-audience=sorafs-devportal \
  --proof
```

- `--skip-build` reuses `docs/portal/build` if CI already produced it.
- Add `--skip-sbom` when `syft` is unavailable (e.g., air-gapped rehearsal).
- The script runs the portal tests, emits CAR + manifest pairs for `portal`,
  `openapi`, `portal-sbom`, and `openapi-sbom`, verifies each CAR when
  `--proof` is set, and drops Sigstore bundles when `--sign` is set.
- Output structure:

```json
{
  "generated_at": "2026-02-19T13:00:12Z",
  "output_dir": "artifacts/devportal/sorafs/20260219T130012Z",
  "artifacts": [
    {
      "name": "portal",
      "car": ".../portal.car",
      "plan": ".../portal.plan.json",
      "car_summary": ".../portal.car.json",
      "manifest": ".../portal.manifest.to",
      "manifest_json": ".../portal.manifest.json",
      "proof": ".../portal.proof.json",
      "bundle": ".../portal.manifest.bundle.json",
      "signature": ".../portal.manifest.sig"
    }
  ]
}
```

Keep the entire folder (or symlink via `artifacts/devportal/sorafs/latest`) so
governance reviewers can trace build artifacts.

## 2. Pin Manifests + Aliases

Use `sorafs_cli manifest submit` to push manifests into Torii and bind aliases.
Set `${SUBMITTED_EPOCH}` to the latest consensus epoch (from
`curl -s "${TORII_URL}/v1/status" | jq '.sumeragi.epoch'` or your dashboard).

```bash
OUT="artifacts/devportal/sorafs/20260219T130012Z"
TORII_URL="https://torii.stg.sora.net/"
AUTHORITY="ih58..."
KEY_FILE="secrets/docs-admin.key"
ALIAS_PROOF="secrets/docs.alias.proof"
SUBMITTED_EPOCH="$(curl -s ${TORII_URL}/v1/status | jq '.sumeragi.epoch')"

cargo run -p sorafs_orchestrator --bin sorafs_cli -- \
  manifest submit \
  --manifest="${OUT}/portal.manifest.to" \
  --chunk-plan="${OUT}/portal.plan.json" \
  --torii-url="${TORII_URL}" \
  --submitted-epoch="${SUBMITTED_EPOCH}" \
  --authority="${AUTHORITY}" \
  --private-key-file "${KEY_FILE}" \
  --alias-namespace docs \
  --alias-name portal \
  --alias-proof "${ALIAS_PROOF}" \
  --summary-out "${OUT}/portal.manifest.submit.json" \
  --response-out "${OUT}/portal.manifest.response.json"
```

- Repeat for `openapi.manifest.to` and the SBOM manifests (omit alias flags for
  SBOM bundles unless governance assigns a namespace).
- Alternative: `iroha app sorafs pin register` works with the digest from the submit
  summary if the binary is already installed.
- Verify registry state with
  `iroha app sorafs pin list --alias docs:portal --format json | jq`.
- Dashboards to watch: `sorafs_pin_registry.json` (`torii_sorafs_replication_*`
  metrics).

## 3. Gateway Headers & Proofs

Generate the HTTP header block + binding metadata:

```bash
iroha app sorafs gateway route-plan \
  --manifest-json "${OUT}/portal.manifest.json" \
  --hostname docs.sora \
  --alias docs:portal \
  --route-label docs-portal-20260219 \
  --proof-status ok \
  --headers-out "${OUT}/portal.gateway.headers.txt" \
  --out "${OUT}/portal.gateway.plan.json"
```

- The template includes `Sora-Name`, `Sora-CID`, `Sora-Proof`, and
  `Sora-Proof-Status` headers plus the default CSP/HSTS/Permissions-Policy.
- Use `--rollback-manifest-json` to render a paired rollback header set.

Before exposing traffic, run:

```bash
./ci/check_sorafs_gateway_probe.sh -- \
  --gateway "https://docs.sora/.well-known/sorafs/manifest" \
  --report-json artifacts/sorafs_gateway_probe/docs.json

scripts/sorafs_gateway_self_cert.sh \
  --manifest "${OUT}/portal.manifest.json" \
  --headers "${OUT}/portal.gateway.headers.txt" \
  --output artifacts/sorafs_gateway_self_cert/docs
```

- The probe enforces GAR signature freshness, alias policy, and TLS cert
  fingerprints.
- The self-cert harness downloads the manifest with `sorafs_fetch` and stores
  CAR replay logs; keep the outputs for audit evidence.

## 4. DNS & Telemetry Guardrails

1. Refresh the DNS skeleton so governance can prove the binding:

   ```bash
   scripts/sns_zonefile_skeleton.py \
     --manifest "${OUT}/portal.manifest.json" \
     --out artifacts/sorafs/portal.dns-cutover.json
   ```

2. Monitor during rollout:

   - `torii_sorafs_alias_cache_refresh_total`
   - `torii_sorafs_gateway_refusals_total{profile="docs"}`
   - `torii_sorafs_fetch_duration_ms` / `_failures_total`

   Dashboards: `sorafs_gateway_observability.json`,
   `sorafs_fetch_observability.json`, and the pin registry board.

3. Smoke the alert rules (`scripts/telemetry/test_sorafs_fetch_alerts.sh`) and
   capture logs/screenshots for the release archive.

## 5. Evidence Bundle

Include the following in the release ticket or governance package:

- `artifacts/devportal/sorafs/<stamp>/` (CARs, manifests, SBOMs, proofs,
  Sigstore bundles, submit summaries).
- Gateway probe + self-cert outputs
  (`artifacts/sorafs_gateway_probe/<stamp>/`,
  `artifacts/sorafs_gateway_self_cert/<stamp>/`).
- DNS skeleton + header templates (`portal.gateway.headers.txt`,
  `portal.gateway.plan.json`, `portal.dns-cutover.json`).
- Dashboard screenshots + alert acknowledgements.
- `status.md` update referencing the manifest digest and alias binding time.

Following this checklist delivers DOCS-7: the portal/OpenAPI/SBOM payloads are
packaged deterministically, pinned with aliases, guarded by `Sora-Proof`
headers, and monitored end-to-end through the existing observability stack.

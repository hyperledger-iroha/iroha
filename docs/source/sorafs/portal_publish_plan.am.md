---
lang: am
direction: ltr
source: docs/source/sorafs/portal_publish_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 84a23df97e771e110dc304ed09155e85d110deca8ba557518055c3669e6b9c27
source_last_modified: "2026-01-22T16:26:46.592059+00:00"
translation_last_reviewed: 2026-02-07
title: Docs Portal → SoraFS Publish Plan (DOCS-7)
summary: Checklist for packaging the developer portal, OpenAPI, and SBOM bundles as SoraFS manifests, pinning them with aliases, and rolling the docs gateway + DNS bindings.
---

# 1. Purpose & Scope

Roadmap item **DOCS-7 – “Publish via SoraFS gateway”** requires every public
docs artefact (portal build, OpenAPI spec, SBOMs) to travel through the SoraFS
pin registry and serve behind `docs.sora` with proof-carrying headers. This note
binds the tooling that already exists in-tree into one reproducible flow so Ops,
Docs/DevRel, and Storage can rehearse the cutover before the Q2 2026 gate.

The plan assumes:

- The packaging helpers under `ci/` and `docs/portal/scripts/` are available.
- Governance has issued the `docs:portal` alias proof bundle (binary Norito).
- SoraDNS entries for `docs.sora` already point at the gateway anycast pool.

See `docs/portal/docs/devportal/deploy-guide.md` for the long-form tutorial;
this file stays focused on the multi-team runbook tied to DOCS-7.

# 2. Build & Package Release Payloads

1. **Kick off the packaging script**

   ```bash
   ./ci/package_docs_portal_sorafs.sh \
     --out artifacts/devportal/sorafs/$(date -u +%Y%m%dT%H%M%SZ) \
     --sign \
     --sigstore-provider=github-actions \
     --sigstore-audience=sorafs-devportal \
     --proof
   ```

   - Runs `npm ci && npm run build` inside `docs/portal/`, mirrors `npm run sync-openapi`
     and `npm run test:*` so OpenAPI/Norito snippets stay fresh.
   - Calls `syft` twice to emit CycloneDX SBOMs for the built portal directory and
     `static/openapi/torii.json`.
   - Uses `sorafs_cli car pack` + `sorafs_cli manifest build` for the portal, OpenAPI,
     portal SBOM, and OpenAPI SBOM payloads (default chunker profile
     `sorafs.sf1@1.0.0`, pin policy min 5 replicas, warm storage, 14‑epoch retention).
   - When `--proof` is supplied, `sorafs_cli proof verify` adds a `${label}.proof.json`
     bundle for each CAR/manifest pair.
   - When `--sign` is supplied, `sorafs_cli manifest sign` emits Sigstore bundles
     (`.manifest.bundle.json` / `.manifest.sig`), using the token pointed at
     `${SIGSTORE_ID_TOKEN}` by default.

2. **Inspect the output summary**

   - The script writes a `package_summary.json` with one entry per artefact:

     ```json
     {
       "generated_at": "2026-02-19T13:00:12Z",
       "output_dir": "/abs/path/artifacts/devportal/sorafs/20260219T130012Z",
       "artifacts": [
         {
           "name": "portal",
           "car": "artifacts/devportal/sorafs/.../portal.car",
           "plan": "artifacts/devportal/sorafs/.../portal.plan.json",
           "car_summary": "artifacts/devportal/sorafs/.../portal.car.json",
           "manifest": "artifacts/devportal/sorafs/.../portal.manifest.to",
           "manifest_json": "artifacts/devportal/sorafs/.../portal.manifest.json",
           "proof": "artifacts/devportal/sorafs/.../portal.proof.json",
           "bundle": "artifacts/devportal/sorafs/.../portal.manifest.bundle.json",
           "signature": "artifacts/devportal/sorafs/.../portal.manifest.sig"
         }
       ]
     }
     ```

   - Archive the whole directory (or symlink via
     `artifacts/devportal/sorafs/latest`) so governance reviewers can trace the
     payload digests back to build logs and Sigstore bundles.

# 3. Submit & Pin Manifests with Aliases

1. **Submit manifests via `sorafs_cli` (preferred)**

   ```bash
   OUT="artifacts/devportal/sorafs/20260219T130012Z"
   TORII_URL="https://torii.stg.sora.net/"
   AUTHORITY="soraカタカナ..."
   KEY_FILE="secrets/docs-admin.key"
   ALIAS_PROOF="secrets/docs.alias.proof"

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

   - Passing `--chunk-plan` lets the CLI derive the SHA3‑256 chunk digest and
     refuse mismatches (no manual digest juggling).
   - Repeat for `openapi.manifest.to`, `portal.sbom.manifest.to`, and
     `openapi.sbom.manifest.to`. SBOMs typically do **not** receive aliases,
     so omit the alias flags unless governance requests a dedicated namespace
     (e.g. `docs:portal-sbom`).
   - Set `${SUBMITTED_EPOCH}` to the current consensus epoch (obtainable from
     `curl -s "${TORII_URL}/v1/status" | jq '.sumeragi.epoch'` or your ops
     dashboard) so the registry can track when the manifest entered the ledger.

2. **Alternative: `iroha app sorafs pin register`**

   - When the deployment host carries the compiled CLI (`cargo install --path .`),
     the same manifest can be pinned via:

     ```bash
     iroha app sorafs pin register \
     --manifest "${OUT}/portal.manifest.to" \
     --chunk-digest "$(jq -r '.chunk_digest_sha3_hex' "${OUT}/portal.manifest.submit.json")" \
     --submitted-epoch "${SUBMITTED_EPOCH}" \
     --alias-namespace docs \
     --alias-name portal \
     --alias-proof "${ALIAS_PROOF}"
     ```

   - This wrapper hits the same `/v1/sorafs/pin/register` endpoint but
     relies on a precomputed digest.

3. **Verify registry state**

   - `iroha app sorafs pin list --alias docs:portal --format json | jq`
     should show the new manifest digest, pin policy, and replication orders.
   - Dashboards: check `dashboards/grafana/sorafs_pin_registry.json` for alias
     counts and `torii_sorafs_replication_backlog_total` (should stay near zero).

# 4. Gateway Headers & `Sora-Proof`

1. **Render the route binding + header template**

   ```bash
   OUT="artifacts/devportal/sorafs/20260219T130012Z"
   iroha app sorafs gateway route-plan \
     --manifest-json "${OUT}/portal.manifest.json" \
     --hostname docs.sora \
     --alias docs:portal \
     --route-label docs-portal-20260219 \
     --proof-status ok \
     --headers-out "${OUT}/portal.gateway.headers.txt" \
     --out "${OUT}/portal.gateway.plan.json"
   ```

   - The template contains the `Sora-Name`, `Sora-CID`, `Sora-Proof`, and
     `Sora-Proof-Status` headers (plus CSP/HSTS/Permissions-Policy). Commit the
     `*.headers.txt` file into the release evidence bundle so a third party can
     recreate the binding.

2. **Update the gateway / CDN config**

   - Feed the generated header block into the Gateway automation per
     `docs/source/sorafs_gateway_tls_automation.md`. For manual cutovers, the
     headers can be pasted into the CDN config or templated via Terraform.
   - If a rollback manifest already exists, pass `--rollback-manifest-json`
     so the command writes both the primary and rollback headers.

3. **Probe & self-cert before opening traffic**

   ```bash
   ./ci/check_sorafs_gateway_probe.sh -- \
     --gateway "https://docs.sora/.well-known/sorafs/manifest" \
     --report-json artifacts/sorafs_gateway_probe/docs.json

   scripts/sorafs_gateway_self_cert.sh \
     --manifest "${OUT}/portal.manifest.json" \
     --headers "${OUT}/portal.gateway.headers.txt" \
     --output artifacts/sorafs_gateway_self_cert/docs
   ```

   - The probe enforces GAR signature freshness, alias proof policy, and TLS
     fingerprints (`dashboards/grafana/sorafs_gateway_observability.json` will
     show `torii_sorafs_gateway_refusals_total` spikes if anything drifts).
   - The self-cert harness mirrors the production fetch path with `sorafs_fetch`,
     storing CAR replay logs under `artifacts/sorafs_gateway_self_cert/`.

# 5. DNS & Monitoring Guardrails

1. **DNS binding evidence**

   - Run `scripts/sns_zonefile_skeleton.py --manifest "${OUT}/portal.manifest.json"` to
     refresh the `docs.sora` TXT stanza (includes latest `Sora-Proof` and
     `Sora-Proof-Status` hashes). Attach the JSON output to
     `artifacts/sorafs/portal.dns-cutover.json` alongside the existing plan.

2. **Telemetry**

   - During the first 30 minutes post-promotion watch:
     - `torii_sorafs_alias_cache_refresh_total` (alias policy health)
     - `torii_sorafs_gateway_refusals_total{profile="docs"}` (gateway policy)
     - `torii_sorafs_fetch_duration_ms` / `torii_sorafs_fetch_failures_total`
       (end-to-end fetch telemetry on the staging fetcher)
   - Dashboards to pin: `sorafs_gateway_observability.json`,
     `sorafs_fetch_observability.json`, and the generic pin registry board.

3. **Chaos & alerts**

   - Trigger `scripts/telemetry/run_sorafs_gateway_probe.sh --scenario dns-cutover`
     so PagerDuty + Alertmanager evidence is archived under
     `artifacts/sorafs_gateway_probe/<stamp>/`.
   - Run `scripts/telemetry/test_sorafs_fetch_alerts.sh` to exercise the
     fetch alert bundle (`dashboards/alerts/sorafs_fetch_rules.yml`).

# 6. Evidence & Reporting

- Archive the following into the release bundle (Git annex or SoraFS):
  - `artifacts/devportal/sorafs/<stamp>/` (CARs, manifests, SBOMs, proofs,
    Sigstore bundles, package summary, manifest submit summaries).
  - `artifacts/sorafs_gateway_probe/<stamp>/` and
    `artifacts/sorafs_gateway_self_cert/<stamp>/`.
  - DNS skeleton (`portal.dns-cutover.json`) and gateway header templates.
  - Dashboard screenshots demonstrating healthy metrics and alert resets.
- Update `status.md` (“Docs/DevRel” + “Storage”) with the manifest digests,
  alias binding timestamp, and probe links.
- Mirror this plan into the portal (`docs/portal/docs/sorafs/portal-publish-plan.md`)
  when updating the Docusaurus site so the public docs share the same steps.

Following these steps satisfies the DOCS-7 deliverable: every artefact now ships
through the canonical Norito/SoraFS pipeline, manifests are aliased and
observable, and both DNS + telemetry capture the signed evidence required
for production sign-off.

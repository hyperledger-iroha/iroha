---
lang: ru
direction: ltr
source: docs/source/sorafs/chunker_registry_rollout_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e4d52f449ae561d75e4723d9573ad6e099fda92ccae1f0fe683baacbbcae494c
source_last_modified: "2026-01-03T18:07:58.428331+00:00"
translation_last_reviewed: 2026-01-30
---

# SoraFS Registry Rollout Checklist

This checklist captures the steps required to promote a new chunker profile or
provider admission bundle from review to production after the governance
charter has been ratified.

> **Scope:** Applies to all releases that modify
> `sorafs_manifest::chunker_registry`, provider admission envelopes, or the
> canonical fixture bundles (`fixtures/sorafs_chunker/*`).

## 1. Pre-flight Validation

1. Regenerate fixtures and verify determinism:
   ```bash
   cargo run --locked -p sorafs_chunker --bin export_vectors
   cargo test -p sorafs_chunker --offline vectors
   ci/check_sorafs_fixtures.sh
   ```
2. Confirm determinism hashes in
   `docs/source/sorafs/reports/sf1_determinism.md` (or the relevant profile
   report) match the regenerated artifacts.
3. Ensure `sorafs_manifest::chunker_registry` compiles with
   `ensure_charter_compliance()` by running:
   ```bash
   cargo test -p sorafs_manifest --lib chunker_registry::tests::ensure_charter_compliance
   ```
4. Update the proposal dossier:
   - `docs/source/sorafs/proposals/<profile>.json`
   - Council minutes entry under `docs/source/sorafs/council_minutes_*.md`
   - Determinism report

## 2. Governance Sign-off

1. Present the Tooling Working Group report and proposal digest to the Sora
   Parliament Infrastructure Panel.
2. Record approval details in
   `docs/source/sorafs/council_minutes_YYYY-MM-DD.md`.
3. Publish the Parliament-signed envelope alongside the fixtures:
   `fixtures/sorafs_chunker/manifest_signatures.json`.
4. Verify the envelope is accessible via the governance fetch helper:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures <url-or-path-to-manifest_signatures.json> \
     --out fixtures/sorafs_chunker
   ```

## 3. Staging Rollout

Refer to the [staging manifest playbook](runbooks/staging_manifest_playbook.md) for a
detailed walkthrough of these steps.

1. Deploy Torii with `torii.sorafs` discovery enabled and admission
   enforcement turned on (`enforce_admission = true`).
2. Push the approved provider admission envelopes to the staging registry
   directory referenced by `torii.sorafs.discovery.admission.envelopes_dir`.
3. Verify provider adverts propagate via the discovery API:
   ```bash
   curl -sS http://<torii-host>/v1/sorafs/providers | jq .
   ```
4. Exercise manifest/plan endpoints with governance headers:
   ```bash
   sorafs-fetch --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider "...staging config..." \
     --gateway-manifest-id <manifest-hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0
   ```
5. Confirm telemetry dashboards (`torii_sorafs_*`) and alert rules report the
   new profile without errors.

## 4. Production Rollout

1. Repeat the staging steps against production Torii nodes.
2. Announce the activation window (date/time, grace period, rollback plan) to
   operator and SDK channels.
3. Merge the release PR containing:
   - Updated fixtures and envelope
   - Documentation changes (charter references, determinism report)
   - Roadmap/status refresh
4. Tag the release and archive the signed artifacts for provenance.

## 5. Post-Rollout Audit

1. Capture final metrics (discovery counts, fetch success rate, error
   histograms) 24h after rollout.
2. Update `status.md` with a short summary and link to the determinism report.
3. File any follow-up tasks (e.g., additional profile authoring guidance) in
   `roadmap.md`.

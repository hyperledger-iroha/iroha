---
lang: ru
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/sorafs/staging-manifest-playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9f708c9c597c0455761049a17989369498d318be348e28f71196bb82761dd36b
source_last_modified: "2026-01-03T18:07:58.297179+00:00"
translation_last_reviewed: 2026-01-30
---


:::note Canonical Source
Mirrors `docs/source/sorafs/runbooks/staging_manifest_playbook.md`. Keep both copies aligned across releases.
:::

## Overview

This playbook walks through enabling the Parliament-ratified chunker profile on a staging Torii deployment before promoting the change to production. It assumes the SoraFS governance charter has been ratified and the canonical fixtures are available in the repository.

## 1. Prerequisites

1. Sync the canonical fixtures and signatures:

   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```

2. Prepare the admission envelope directory that Torii will read at startup (example path): `/var/lib/iroha/admission/sorafs`.
3. Ensure the Torii config enables the discovery cache and admission enforcement:

   ```toml
   [torii.sorafs.discovery]
   discovery_enabled = true
   known_capabilities = ["torii_gateway", "chunk_range_fetch", "vendor_reserved"]

   [torii.sorafs.discovery.admission]
   envelopes_dir = "/var/lib/iroha/admission/sorafs"

   [torii.sorafs.storage]
   enabled = true

   [torii.sorafs.gateway]
   enforce_admission = true
   enforce_capabilities = true
   ```

## 2. Publish Admission Envelopes

1. Copy the approved provider admission envelopes into the directory referenced by `torii.sorafs.discovery.admission.envelopes_dir`:

   ```bash
   install -m 0644 fixtures/sorafs_manifest/provider_admission/*.json \
     /var/lib/iroha/admission/sorafs/
   ```

2. Restart Torii (or send a SIGHUP if you wrapped the loader with on-the-fly reload).
3. Tail the logs for admission messages:

   ```bash
   torii | grep "loaded provider admission envelope"
   ```

## 3. Validate Discovery Propagation

1. Post the signed provider advert payload (Norito bytes) produced by your
   provider pipeline:

   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```

2. Query the discovery endpoint and confirm the advert appears with canonical aliases:

   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```

   Ensure `profile_aliases` includes `"sorafs.sf1@1.0.0"` as the first entry.

## 4. Exercise Manifest & Plan Endpoints

1. Fetch the manifest metadata (requires a stream token if admission is enforced):

   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_specs.json \
     --gateway-provider name=staging,provider-id=<hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

2. Inspect the JSON output and verify:
   - `chunk_profile_handle` is `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` matches the determinism report.
   - `chunk_digests_blake3` align with the regenerated fixtures.

## 5. Telemetry Checks

- Confirm Prometheus exposes the new profile metrics:

  ```bash
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```

- Dashboards should show the staging provider under the expected alias and keep brownout counters at zero while the profile is active.

## 6. Rollout Readiness

1. Capture a short report with the URLs, manifest ID, and telemetry snapshot.
2. Share the report in the Nexus rollout channel alongside the planned production activation window.
3. Proceed to the production checklist (Section 4 in `chunker_registry_rollout_checklist.md`) once stakeholders sign off.

Keeping this playbook updated ensures every chunker/admission rollout follows the same deterministic steps across staging and production.

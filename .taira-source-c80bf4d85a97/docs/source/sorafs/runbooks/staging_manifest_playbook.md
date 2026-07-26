# SoraFS Staging Manifest Playbook

This playbook walks through enabling the Parliament-ratified chunker profile on
a staging Torii deployment before promoting the change to production. It
assumes the SoraFS governance charter has been ratified and the canonical
fixtures are available in the repository.

> **Portal:** This content is mirrored in the Docusaurus portal at
> `docs/portal/docs/sorafs/staging-manifest-playbook.md`. Update both copies
> to keep the SoraFS rollout guidance aligned across doc sets.

## 1. Prerequisites

1. Sync the canonical fixtures and signatures:
   ```bash
   cargo xtask sorafs-fetch-fixture \
     --signatures https://nexus.example/api/sorafs/manifest_signatures.json \
     --out fixtures/sorafs_chunker
   ci/check_sorafs_fixtures.sh
   ```
2. Prepare the admission envelope directory that Torii will read at startup
   (example path): `/var/lib/iroha/admission/sorafs`.
3. Ensure the Torii config enables the discovery cache and admission
   enforcement:
   ```toml
   [sorafs.discovery]
   discovery_enabled = true
   known_capabilities = ["torii_gateway", "chunk_range_fetch", "vendor_reserved"]

   [sorafs.discovery.admission]
   envelopes_dir = "/var/lib/iroha/admission/sorafs"
   trusted_council_keys = [
     "ed0120REPLACE_WITH_COUNCIL_KEY_1",
     "ed0120REPLACE_WITH_COUNCIL_KEY_2",
     "ed0120REPLACE_WITH_COUNCIL_KEY_3",
   ]
   signature_threshold = 2

   [sorafs.storage]
   enabled = true

   [sorafs.gateway]
   enforce_admission = true
   enforce_capabilities = true
   ```

   Torii refuses to start when discovery/admission is enabled without a non-empty, satisfiable
   Ed25519 council policy. Use governance council keys only; node identity, provider advert, and
   request-signing keys are not admission trust roots.

## 2. Publish Admission Envelopes

1. Copy the approved provider admission envelopes into the directory referenced
   by `sorafs.discovery.admission.envelopes_dir`:
   ```bash
   install -m 0644 governance/providers/*.to \
     /var/lib/iroha/admission/sorafs/
   ```
   Keep only canonical `.to` envelopes and the documented `README.md` in this directory. Symlinks,
   directories, unknown files, oversized envelopes, corrupt Norito, and duplicate providers fail
   startup.
2. Restart Torii. Admission policy and registry replacement is a startup operation; do not assume
   SIGHUP reloads trust roots.
3. Tail the logs for admission messages:
   ```
   torii | grep "loaded provider admission envelope"
   ```

## 3. Validate Discovery Propagation

1. Post the signed provider advert payload (Norito bytes) produced by your
   provider pipeline:
   ```bash
   curl -sS -X POST --data-binary @provider_advert.to \
     http://staging-torii:8080/v1/sorafs/provider/advert
   ```
2. Query the discovery endpoint and confirm the advert appears with canonical
   aliases:
   ```bash
   curl -sS http://staging-torii:8080/v1/sorafs/providers | jq .
   ```
   Ensure `profile_aliases` includes `"sorafs.sf1@1.0.0"` as the first entry.

## 4. Exercise Manifest & Plan Endpoints

1. Fetch the manifest metadata (requires a stream token if admission is
   enforced):
   ```bash
   sorafs-fetch \
     --plan fixtures/chunk_fetch_plan.json \
     --gateway-provider name=staging,provider-id=<hex>,gateway-key=<ed25519-public-key-hex>,base-url=https://staging-gateway/,stream-token=<base64> \
     --gateway-manifest-id <manifest_id_hex> \
     --gateway-chunker-handle sorafs.sf1@1.0.0 \
     --json-out=reports/staging_manifest.json
   ```

   `chunk_fetch_plan.json` must be the strict payload-bound
   `sorafs.chunk_fetch_plan.v1` object emitted by the builder; a copied
   `chunk_fetch_specs` report field is not a standalone plan.
2. Inspect the JSON output and verify:
   - `chunk_profile_handle` is `sorafs.sf1@1.0.0`.
   - `manifest_digest_hex` matches the determinism report.
   - `chunk_digests_blake3` align with the regenerated fixtures.

## 5. Telemetry Checks

- Confirm Prometheus exposes the new profile metrics:
  ```
  curl -sS http://staging-torii:8080/metrics | grep torii_sorafs_chunk_range_requests_total
  ```
- Dashboards should show the staging provider under the expected alias and keep
  brownout counters at zero while the profile is active.

## 6. Rollout Readiness

1. Capture a short report with the URLs, manifest ID, and telemetry snapshot.
2. Share the report in the Nexus rollout channel alongside the planned
   production activation window.
3. Proceed to the production checklist (Section 4 in
   `chunker_registry_rollout_checklist.md`) once stakeholders sign off.

Keeping this playbook updated ensures every chunker/admission rollout follows
the same deterministic steps across staging and production.

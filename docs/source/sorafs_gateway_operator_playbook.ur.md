---
lang: ur
direction: rtl
source: docs/source/sorafs_gateway_operator_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 0b0d1b2ada1254b20926b840d8557df46f1056ef06fa28dd329d7f481e4c5a56
source_last_modified: "2026-01-03T18:07:57.764666+00:00"
translation_last_reviewed: 2026-01-30
---

---
title: SoraFS Gateway Chunk-Range Operator Playbook
summary: Operational guidance for chunk-range endpoints, stream tokens, and telemetry.
---

# SoraFS Gateway Chunk-Range Operator Playbook

## 1. Prerequisites

- Gateway upgraded to trustless profile (`docs/source/sorafs_gateway_profile.md`).
- Stream token enforcement enabled (see `sorafs_gateway_chunk_range.md`).
- TLS/ECH automation configured (`sorafs_gateway_tls_automation.md`).

## 2. Configuration Checklist

1. Enable chunk-range endpoints:
   ```toml
   [sorafs.gateway.chunk_range]
   enabled = true
   max_parallel_streams = 512
   token_ttl_secs = 900
   rate_limit_bytes_per_sec = "100 MiB"
   ```
2. Point gateway at admission registry (`sorafs_manifest::provider_admission`).
3. Configure telemetry exporter (Prometheus/OpenTelemetry).
4. Set up log aggregation for token issuance/revocation events.

## 3. Operational Procedures

### Token Issuance

- Use `POST /token` with signed manifest envelope.
- Store response (token, expiry) in operator dashboard.
- Rotate tokens proactively before TTL when running 24/7 workloads.

### Monitoring

- Dashboards:
  - `sorafs_gateway_chunk_range_requests_total`
  - `sorafs_gateway_stream_tokens_active`
  - `sorafs_gateway_stream_token_denials_total`
  - Latency histograms per chunker handle.
- Alerts:
  - Token denials > threshold.
  - Range latency > SLO.
  - Proof verification failures (422 responses).

### Incident Response

- Token exhaustion: increase rate limit or issue new token; notify orchestrator operators.
- Proof failures: quarantine provider, regenerate proofs, rerun conformance harness.
- Admission mismatch: sync admission envelopes from governance; update Torii cache.

## 4. Troubleshooting

| Symptom | Possible Cause | Action |
|---------|----------------|--------|
| 428 `required_headers_missing` | Client downgrade / missing `dag-scope` | Validate client library version, update orchestrator. |
| 429 `stream_token_exhausted` | Token over quota | Issue new token, adjust `rate_limit_bytes_per_sec`. |
| 412 `admission_required` | Envelope missing/expired | Refresh admission registry, verify manifest signatures. |
| 422 proof failure | Corrupted chunk or fixture mismatch | Re-run conformance suite, compare PoR roots. |

## 5. Maintenance

- Run SF-5a self-cert kit before and after major upgrades.
- Update fixtures when governance publishes new dataset.
- Review observability dashboards weekly, ensure alert routing functioning.

## 6. Automation & Incident Playbooks

### 6.1 Stream-token rotation script

To keep stream tokens fresh without manual intervention, operators SHOULD use the helper
`scripts/sorafs_gateway_rotate_tokens.sh` (shipped alongside the gateway packages):

```bash
./scripts/sorafs_gateway_rotate_tokens.sh \
  --provider-id "$PROVIDER_ID" \
  --manifest-catalog manifests/pin_registry.csv \
  --token-endpoint https://gateway.example.com/token \
  --ttls 3600 \
  --parallel 8 \
  --log-dir /var/log/sorafs/token_rotation
```

Key behaviours:

- Reads the manifest catalog emitted by the Pin Registry and batches `/token` requests
  per provider/manifest handle.
- Signs requests with the configured operator key (`SORA_FS_OPERATOR_SK` env var) and
  records the issued token IDs in the rotation log for auditing.
- Automatically retries with exponential back-off when the gateway returns `429` or
  `503`, and annotates failures via `telemetry_sorafs_token_rotation_errors_total`.

Recommended automation pattern:

1. Schedule the script on each provider host via cron/systemd (`*/30 * * * *`).
2. Export logs to the central logging pipeline; alerts should fire when failures exceed
   5% in a given hour.
3. Run the script manually after every admission update to ensure new manifests receive
   fresh tokens before operators open the gateway to orchestrators.

### 6.2 Incident playbook integration

Token rotation ties into the existing incident playbooks maintained under
`docs/source/sorafs_gateway_tls_automation.md` (TLS/ECH) and `docs/source/sorafs_gateway_capability_tests.md`
 (GAR refusals). Operators should extend those playbooks with the following guidance:

- **TLS/ECH outages** – If the TLS automation workflow rolls back certificates, rerun the
  rotation script immediately after the rollout to prevent expired token signatures due to
  mismatched certificate chains.
- **Gateway Admission Rate (GAR) incidents** – When GAR triggers throttle/deny behaviour,
  the incident coordinator should notify the token rotation on-call so they suspend the
  script (set `SORA_FS_ROTATION_SUSPEND=1`) and avoid flooding the cluster, then resume
  once GAR clears.
- **Fallback to chunk-range safe mode** – When the orchestrator failover plan relies on a
  reduced set of providers, update `--manifest-catalog` to point at a filtered list so the
  script only issues tokens for active providers.

Document the above adjustments in the local runbook and ensure PagerDuty incidents for
TLS/ECH or GAR include a checklist item to confirm stream-token automation has either
been paused/resumed as appropriate.

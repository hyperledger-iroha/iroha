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

1. Enable storage and stream-token issuance with bounded defaults:
   ```toml
   [sorafs.storage]
   enabled = true

   [sorafs.storage.stream_tokens]
   enabled = true
   signing_key_path = "/etc/iroha/sorafs_gateway_secrets/token_signing_sk"
   key_version = 1
   default_ttl_secs = 900
   default_max_streams = 32
   default_rate_limit_bytes = 104857600
   default_requests_per_minute = 60
   ```
2. Point gateway at admission registry (`sorafs_manifest::provider_admission`).
3. Configure telemetry exporter (Prometheus/OpenTelemetry).
4. Set up log aggregation for token issuance/revocation events.

## 3. Operational Procedures

### Token Issuance

- Use the authenticated `POST /v1/sorafs/storage/token` route with
  `X-SoraFS-Client`, a unique `X-SoraFS-Nonce`, and the manifest/provider JSON
  body described in `sorafs_gateway_chunk_range.md`.
- Store only token ID, key version, and expiry in the operator dashboard. Keep
  the encoded bearer token in a secret manager and redact it from logs.
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

### 6.1 Stream-token refresh automation

This repository does not ship a token-rotation script. Use the deployment's
authenticated secret-delivery job to call the canonical Torii endpoint and put
the result directly into the consumer secret store. A minimal single-manifest
probe looks like:

```bash
umask 077
curl --fail --silent --show-error \
  -X POST https://gateway.example.com/v1/sorafs/storage/token \
  -H "Content-Type: application/json" \
  -H "X-SoraFS-Client: ${CLIENT_ID}" \
  -H "X-SoraFS-Nonce: ${UNIQUE_NONCE}" \
  --data-binary "{\"manifest_id_hex\":\"${MANIFEST_ID}\",\"provider_id_hex\":\"${PROVIDER_ID}\"}" \
  > "${RUNTIME_SECRET_DIR}/stream-token.json"
```

Required automation behaviour:

- Authenticate at the Torii perimeter; client ID and nonce headers alone are
  not credentials.
- Compare `X-SoraFS-Verifying-Key` with the independently approved gateway key,
  verify the domain-separated token signature, provider/manifest/profile
  bindings, issuance time, and expiry, then deploy the exact approved key as
  `gateway-key` with the token.
- Never log the response body or a full provider descriptor. Record token ID,
  key version, expiry, provider ID, manifest ID, and approval reference only.
- Honour `Retry-After` on `429`, use bounded jittered retries for transient
  failures, and fail closed on every signature, key, or binding mismatch.

Recommended automation pattern:

1. Schedule the secret-delivery job so refresh completes before token expiry.
2. Export logs to the central logging pipeline; alerts should fire when failures exceed
   5% in a given hour.
3. Run the job after every admission update to ensure new manifests receive
   fresh tokens before operators open the gateway to orchestrators.

### 6.2 Incident playbook integration

Token rotation ties into the existing incident playbooks maintained under
`docs/source/sorafs_gateway_tls_automation.md` (TLS/ECH) and `docs/source/sorafs_gateway_capability_tests.md`
 (GAR refusals). Operators should extend those playbooks with the following guidance:

- **TLS/ECH outages** – A certificate rollback does not alter Ed25519 token
  signatures. Restore trusted HTTPS first, then refresh tokens only if their
  normal expiry window requires it; never enable plaintext fallback.
- **Gateway Admission Rate (GAR) incidents** – When GAR triggers throttle/deny behaviour,
  the incident coordinator should notify the token rotation on-call so they suspend the
  issuance job and avoid flooding the cluster, then resume
  once GAR clears.
- **Fallback to chunk-range safe mode** – When the orchestrator failover plan relies on a
  reduced set of providers, filter the issuance inventory so the job only
  issues tokens for active providers.

Document the above adjustments in the local runbook and ensure PagerDuty incidents for
TLS/ECH or GAR include a checklist item to confirm stream-token automation has either
been paused/resumed as appropriate.

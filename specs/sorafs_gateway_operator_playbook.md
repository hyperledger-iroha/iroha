---
title: SoraFS Gateway Chunk-Range Operator Playbook
summary: Operational guidance for chunk-range endpoints, stream tokens, and telemetry.
---

# SoraFS Gateway Chunk-Range Operator Playbook

## 1. Prerequisites

- Gateway upgraded to trustless profile (`specs/sorafs_gateway_profile.md`).
- Stream token enforcement enabled (see `sorafs_gateway_chunk_range.md`).
- TLS/ECH automation configured (`sorafs_gateway_tls_automation.md`).

## 2. Configuration Checklist

1. Enable storage and stream-token issuance with bounded defaults:
   ```toml
   [torii]
   api_tokens = ["<runtime-provisioned-stream-token-issuer-credential>"]

   [sorafs.storage]
   enabled = true

   [sorafs.storage.stream_tokens]
   enabled = true
   signer_handle = "pkcs11:prod/stream-token/v4"
   signer_public_key_hex = "<64-lowercase-hex-characters>"
   signer_revision = 4
   signer_policy_digest_hex = "<64-lowercase-nonzero-hex-characters>"
   admission_provider_handle = "sealed-cas:prod/stream-token/admission/v1"
   admission_provider_revision = 7
   admission_provider_policy_digest_hex = "<64-lowercase-nonzero-hex-characters>"
   key_version = 4
   default_ttl_secs = 900
   default_max_streams = 32
   default_rate_limit_bytes = 104857600
   default_requests_per_minute = 60
   ```
   At least one Torii API token is mandatory for this issuance route.
   `require_api_token = true` remains the recommended listener-wide posture,
   but this route validates its credential independently.
2. Configure distinct proof-outcome, repair, reserve, and orderbook entries under
   `sorafs.storage.native_transaction_signers`, and inject all four matching
   live providers. Storage startup requires them even when the corresponding
   new-work generation flags are disabled.
3. Inject the authenticated external software-signer adapter for the configured non-secret
   handle. The Ed25519 private key must remain non-exportable; its credentials,
   session, and PIN are runtime-only and must never be committed or written to
   TOML, signing-key files, logs, or readiness artefacts. The TOML `enabled`
   value is the only production activation control; an environment variable
   cannot enable issuance.
4. Require two startup probes to bind the adapter's reported handle, public key,
   non-zero revision, and public-policy digest exactly to `signer_handle`,
   `signer_public_key_hex`, `signer_revision`, and
   `signer_policy_digest_hex`. For every issuance, revalidate that identity
   before and after signing, then strictly verify the raw 64-byte signature
   against the configured public key before releasing the token. Missing,
   mismatched, drifting, stale, substituted, or test-marked bindings,
   unavailable/refusing signers, and malformed or non-verifying output fail
   closed.
5. Point gateway at admission registry (`sorafs_manifest::provider_admission`).
6. Configure telemetry exporter (Prometheus/OpenTelemetry).
7. Set up payload-free log aggregation for token issuance/revocation outcomes.

## 3. Operational Procedures

### Token Issuance

- Use the authenticated `POST /v1/sorafs/storage/token` route with
  exactly one configured `X-API-Token`, `X-SoraFS-Client`, a unique
  `X-SoraFS-Nonce`, and the manifest/provider JSON body described in
  `sorafs_gateway_chunk_range.md`. The API credential, not the client label,
  owns the issuance budget.
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
- Reconcile the configured signer handle, public-key fingerprint, and
  `key_version` with the approved external software-signer inventory after every deployment.

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
  -H "X-API-Token: ${TORII_API_TOKEN}" \
  -H "X-SoraFS-Client: ${CLIENT_ID}" \
  -H "X-SoraFS-Nonce: ${UNIQUE_NONCE}" \
  --data-binary "{\"manifest_id_hex\":\"${MANIFEST_ID}\",\"provider_id_hex\":\"${PROVIDER_ID}\"}" \
  > "${RUNTIME_SECRET_DIR}/stream-token.json"
```

Required automation behaviour:

- Supply one `torii.api_tokens` credential. The issuance handler validates it
  even when the listener-wide `torii.require_api_token` setting is false;
  client ID and nonce headers are never credentials.
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

### 6.2 Signing-key rotation

1. Create the replacement Ed25519 key inside the independently administered
   software signer, keep it encrypted and runtime-only, and assign a new non-secret signer handle.
2. In one controlled rollout, inject the replacement adapter and update
   `signer_handle`, `signer_public_key_hex`, `signer_revision`,
   `signer_policy_digest_hex`, and `key_version`.
3. Restart the issuer, require both exact startup qualification probes, and issue a probe token.
   Strictly verify the returned signature against the new configured public key
   before publishing that key through authenticated provider inventory.
4. Deploy a matching `gateway-key` and token atomically. If an overlap is
   necessary, use separately named old/new descriptors; there is no implicit
   multi-key acceptance or file/env fallback.
5. Remove the old descriptor by its final token expiry, revoke the old software-signing
   key, and retain only payload-free evidence: non-secret handles, public-key
   fingerprints, versions, approval, activation/expiry times, and negative
   old-key/cross-key/wrong-handle probes.

### 6.3 Incident playbook integration

Token rotation ties into the existing incident playbooks maintained under
`specs/sorafs_gateway_tls_automation.md` (TLS/ECH) and `specs/sorafs_gateway_capability_tests.md`
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

---
title: SoraFS Gateway Deployment & Operations Handbook
summary: End-to-end rollout, monitoring, and incident response guidance for production Torii gateway operators.
---

# SoraFS Gateway Deployment & Operations Handbook

This handbook gives infra teams a single playbook for shipping and running Torii’s SoraFS gateway service. It covers blue/green deployment strategy, required observability hooks, and the incident runbooks/ compliance artefacts that operators must maintain.

## 1. Scope & Audience

- **Audience:** platform/Ops teams onboarding new gateways, SREs responsible for day-two operations, and governance reviewers validating evidence of compliance.
- **Out of scope:** SoraFS provider economics, PoR sampling design, or Torii core upgrade procedures. See `docs/source/sorafs_*.md` for complementary design notes.

## 2. Prerequisites

| Requirement | Notes |
|-------------|-------|
| Torii build ≥ `2026-02-18` | Must include stream-token enforcement and `torii.sorafs_gateway` config surface. |
| GAR admission artefacts | Gateway admission envelope + manifest signed via governance tooling. |
| Stream token signing key | Stored at `sorafs_gateway_secrets/token_signing_sk`; distribute its Ed25519 public key through authenticated provider inventory. Rotate as described in §5.4. |
| Observability stack | Prometheus + Grafana dashboards (`grafana_sorafs_gateway_*`) shipped in `docs/source/`. |
| Smoke tooling | Latest `sorafs-fetch` CLI (`cargo run -p sorafs_fetch -- --help`) with the gateway options described below. |

## 3. Configuration Checklist

1. Populate `torii.sorafs_gateway` in the node config:
   ```toml
   [torii.sorafs_gateway]
   enable = true
   require_manifest_envelope = true
   enforce_admission = true
   direct_mode.default_mapping = "vanity"
   stream_tokens.signing_key_path = "/etc/iroha/sorafs_gateway_secrets/token_signing_sk"
   stream_tokens.default_ttl_secs = 900
   stream_tokens.default_max_streams = 8
   ```
2. Ensure TLS automation controller is active (`tls_automation` section) so the `X-Sora-TLS-State` header is populated.
3. Configure observability exporters (Prometheus scrape of `torii_metrics` endpoint). Dashboards referenced in §4 expect metric names `torii_sorafs_chunk_range_requests_total`, `torii_sorafs_stream_token_denials_total{reason=…}`, etc.

The signing-key path must name one regular, non-symlink, single-link file. On
Unix it must be owned by the process effective user and grant no group or other
permissions (`0400` or `0600` are the recommended modes). The file is read with
`O_NOFOLLOW`, bounded to 64 bytes, and must contain either exactly 32 raw seed
bytes or exactly 64 lowercase hexadecimal characters without whitespace.
Startup fails closed for wrong ownership, permissive permissions, all-zero
material, path replacement during the read, hard links, or non-canonical key
text. Configure positive token defaults only: zero no longer means unlimited,
TTL is capped at one hour, and request overrides may only reduce these defaults.

### 3.1 CID cache hydration and origin isolation

Torii treats every provider-advertised hydration route as untrusted input. A
provider Torii endpoint must resolve to a clean HTTPS origin: credentials,
queries, fragments, non-root base paths, redirects, and HTTP endpoints are
rejected. Torii resolves the hostname once, rejects the entire answer when any
address is loopback, private, link-local, multicast, documentation, transition,
or otherwise reserved, and pins the validated addresses into the request client.
This prevents a later DNS answer from rebinding the request to an internal
service. Rustls certificate-chain and hostname verification remain mandatory;
there is no insecure-certificate override or plaintext fallback. Operators must
therefore publish public, directly reachable Torii
origins in provider adverts; split-horizon or RFC 1918 routes are intentionally
not supported by public CID hydration.

Remote manifest and payload responses are streamed into bounded buffers. Torii
caps provider attempts, DNS answers, response headers, metadata, file entries,
payload bytes, and concurrent distinct CID hydrations. Declared lengths are
checked before allocation and streamed lengths are checked again. Canonical
base64/Norito, manifest policy and digest bindings, exact fetch ranges, payload
and chunk-plan digests, file layout, local free capacity, and the active denylist
must all pass before anything is persisted. Redirects are never followed. A
`429` with `Retry-After` means the bounded hydration single-flight table is full;
a `502` means every approved route failed transport or integrity checks.

The `/sorafs/cid/<cid>/…` path gateway is not an execution origin. HTML, CSS,
JavaScript, SVG, XML, PDF, WebAssembly, and other explicitly active formats are
redirected to the configured `<cid>.sorafs…` isolated origin, even for
non-navigation requests. If Torii cannot derive an approved CID origin it
returns `421` instead of serving the bytes on the API origin. Passive assets may
remain on the path gateway, always with `X-Content-Type-Options: nosniff`; unknown
media types are forced to download with `Content-Disposition: attachment`.
Individual responses are limited to 8 MiB; larger objects require one HTTP byte
range of at most 8 MiB and receive `206` plus `Content-Range`.

## 4. Blue/Green Rollout Procedure

### 4.1 Pre-flight Validation

1. Generate a stream token for the new gateway (stage) host:
   ```bash
   curl -sS -X POST https://stage-gw.example/token \
     -H "X-SoraFS-Client: stage-orchestrator" \
     -H "X-SoraFS-Nonce: $(uuidgen)" \
     --data-binary @admission_envelope.to \
     | jq .
   ```
2. Run a gateway fetch dry-run from a staging orchestrator using the new CLI flag:
   ```bash
   sorafs-fetch \
     --plan=plan.json \
     --gateway-provider=name=stage-gw,provider-id=<hex>,gateway-key=<ed25519-public-key-hex>,base-url=https://stage-gw.example/,stream-token=<base64> \
     --gateway-manifest-id=<manifest_id_hex> \
     --gateway-chunker-handle=sorafs.sf1@1.0.0 \
     --gateway-client-id=stage-orchestrator \
     --output=/tmp/stage.bin \
     --json-out=/tmp/stage_report.json
   ```
   The report should show zero retries and `provider_reports[].metadata.capabilities` must include `chunk_range_fetch`.
3. Capture a trustless verification summary for the staged CAR:
   ```bash
   cargo run -p sorafs_car --bin soranet_trustless_verifier --features cli --locked -- \
     --manifest fixtures/sorafs_gateway/1.0.0/manifest_v1.to \
     --car fixtures/sorafs_gateway/1.0.0/gateway.car \
     --json-out=artifacts/gateway/stage_trustless_summary.json --quiet
   ```
   Attach the JSON to the blue/green evidence packet; it records the manifest/payload digests, chunk plan SHA3-256 digest, and PoR root using the shared verifier config (`configs/soranet/gateway_m0/gateway_trustless_verifier.toml`).

### 4.2 Blue/Green Cutover

| Step | Description | Acceptance criteria |
|------|-------------|---------------------|
| 1 | Deploy new gateway instances (green) alongside existing blue pool. | healthz + `/v1/sorafs/storage/status` pass. |
| 2 | Register green hosts via admission registry update; publish governance envelope. | `torii.sorafs_gateway.direct_mode.plan` dry-run shows expected vanity mapping. |
| 3 | Shift a small traffic slice (5%) using orchestration tokens targeted at `stage-gw`. | Dashboard `grafana_sorafs_gateway_load_tests.json` shows ≤1% refusal. |
| 4 | Expand to 50% once telemetry remains within SLOs for 30 minutes. | `torii_sorafs_stream_token_denials_total` slope near zero; `scripts/sorafs_direct_mode_smoke.sh` passes against staging gateway endpoints. |
| 5 | Drain blue instances, revoke their stream tokens, and remove from admission registry. | `sorafs_gateway_token_revocations_total` increments and old hosts stop serving traffic. |

Reuse `scripts/sorafs_direct_mode_smoke.sh` for the direct-mode gate. Supply provider adverts via
CLI or point the script to `docs/examples/sorafs_direct_mode_smoke.conf` (update the manifest hash
and stream-token entries before running):

```bash
./scripts/sorafs_direct_mode_smoke.sh \
  --config docs/examples/sorafs_direct_mode_smoke.conf \
  --provider name=green-1,provider-id=<hex>,gateway-key=<ed25519-public-key-hex>,base-url=https://gw-green.example/,stream-token=<base64>
```

The wrapper persists the scoreboard declared in
`docs/examples/sorafs_direct_mode_policy.json`, records the fetch summary, and
leverages the new `sorafs_cli fetch --orchestrator-config=…` flag so operators
can reference the same policy JSON used during planning.

### 4.3 Post-Deployment Tasks

- Archive rollout evidence: CLI commands, Grafana snapshots, token logs.
- Update `status.md` with completion notes if required by governance.

## 5. Operations & Incident Response

### 5.1 Monitoring Deck

Pin the following dashboards (JSON available under `docs/source/`):
- `grafana_sorafs_gateway_load_tests.json` – latency, refusal rate, throughput.
- `grafana_sorafs_gateway_direct_mode.json` – host mapping status, denylist hits.
- `grafana_sorafs_gateway_tokens.json` – active tokens, issuance/denial counts.

Recommended alerts:
- `torii_sorafs_chunk_range_requests_total{result="5xx"}` over 1% for 5 minutes.
- `torii_sorafs_stream_token_denials_total{reason="rate_limited"}` > 10/min.
- `gateway_policy_denials_total{reason="admission_unavailable"}` > 0.

### 5.2 Common Incidents & Playbooks

| Incident | Detection | Immediate Action | Follow-up |
|----------|-----------|------------------|-----------|
| Stream token exhaustion | Alert: `rate_limited` denials | Issue a token with `max_streams` no greater than the configured ceiling; adjust orchestrator concurrency. | Review client budget; update `torii.sorafs_gateway.stream_tokens.default_max_streams` through the controlled configuration rollout if the ceiling is too low. |
| GAR mismatch / provider not admitted | Policy denial metric spikes | Verify admission registry sync; run `iroha_cli app sorafs direct-mode status`. | Re-sign manifest/envelope; document in governance log. |
| TLS automation failure | `X-Sora-TLS-State` transitions to `degraded` | Trigger manual ACME renewal (`sorafs-gateway tls renew`); fall back to stored cert. | File incident report with cert timeline. |
| Denylist hit / governance takedown | `gateway_policy_denials_total{reason="denylisted"}` | Confirm request metadata, inform governance, block offending provider/alias. | Update denylist documentation; retain logs for compliance. |

### 5.3 Log & Audit Requirements

- Retain stream-token issuance logs (JSON body + headers) for ≥90 days.
- Forward policy violations to SIEM with context: manifest ID, provider alias, reason code.
- Store Grafana snapshots for go/no-go decisions in the release ticket.

### 5.4 Key Rotation

1. Generate a fresh non-zero Ed25519 seed in the approved KMS/HSM workflow and
   install it at the configured `sorafs.storage.stream_tokens.signing_key_path`.
   Increment `key_version` before issuing tokens from the new key.
2. Publish the new 32-byte public key in the authenticated provider deployment
   inventory and bind it to the same admitted provider ID. The token endpoint's
   `X-SoraFS-Verifying-Key` header is useful for comparison but is not a trust
   anchor by itself.
3. Stage a new provider descriptor containing the new `gateway-key` and a token
   signed by that key. A descriptor pins exactly one key; never pair a new key
   with an old token or silently fall back to a key carried beside a failed
   token.
4. Switch consumers atomically after the inventory update is visible. If an
   overlap window is required, use separately named old/new descriptors so each
   token remains bound to its own pinned key, then remove the old descriptor no
   later than its final token expiry.
5. Revoke and destroy the old secret, retain the public-key fingerprint,
   `token_pk_version`, activation time, final expiry, and change approval in the
   audit evidence, and run negative probes proving old-key tokens are rejected.

### 5.5 WAF Policy Pack

Gateway rollouts must include the shared WAF/rate pack emitted under `configs/soranet/gateway_m0/gateway_waf_policy.yaml` (version `2`). The pack sets a 300 s shadow window, bakes in the FP harness (`ci/check_sorafs_gateway_conformance.sh`), and links to this handbook as the rollback playbook. Attach the pack alongside the trustless summary and TLS evidence in every governance submission.

## 6. Compliance & Governance Checklist

- **Evidence pack**: admission envelopes, token rotation logs, change tickets.
- **Self-cert validation**: run `docs/source/sorafs_gateway_self_cert.md` workflows before onboarding operators.
- **Regulatory alignment**: ensure storage providers sign the gateway operations SLA covering stream-token retention, TLS rotation cadence, and denylist response time.

## 7. Appendix: CLI Reference

### 7.1 `sorafs-fetch` gateway usage

```
sorafs-fetch \
  --plan=chunk_fetch_plan.json \
  --gateway-provider=name=gw-a,provider-id=e1ab...,gateway-key=<ed25519-public-key-hex>,base-url=https://gw-a.example/,stream-token=<b64> \
  --gateway-provider=name=gw-b,provider-id=f2cd...,gateway-key=<ed25519-public-key-hex>,base-url=https://gw-b.example/,stream-token=<b64> \
  --gateway-manifest-id=<manifest_id_hex> \
  --gateway-chunker-handle=sorafs.sf1@1.0.0 \
  --gateway-client-id=stage-orchestrator \
  --json-out=gateway_report.json
```

Outputs include provider metadata (`metadata.capabilities`, `metadata.range_capability`) so operators can archive conformance evidence alongside the rollout record.

Guard-aware fetches reuse the same entry guards as the SoraNet client stack. Add the following flags when the directory consensus is available:

- `--guard-directory=consensus.json` — Norito JSON payload containing the relay descriptors published by SNNet-3. The CLI refreshes the pinned guards before running the fetch.
- `--guard-cache=/var/lib/sorafs/guards.norito` — Norito-encoded `GuardSet` persisted after a successful run; reruns reuse the cache even without a new directory.
- `--guard-target 3` / `--guard-retention-days 30` — Override the default pin count (3) and retention window (30 days) when staging region-specific rollouts.

The cache file lives alongside the orchestrator config so multi-source runs stay deterministic across CLI/SDK tooling.

---

**Change control:** Update this handbook whenever gateway configs, telemetry labels, or stream-token policies change. Reference this document in future roadmap entries for SF-5d/SF-7 tasks.

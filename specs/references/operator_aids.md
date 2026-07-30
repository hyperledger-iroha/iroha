# Torii Endpoints — Operator Aids (Quick Reference)

This page lists non-consensus, operator-facing endpoints that help with visibility and troubleshooting. Responses are JSON unless noted.

Consensus (Sumeragi)
- Metrics: `sumeragi_new_view_receipts_by_hv{height,view}` gauges mirror the counts.
- GET `/v1/sumeragi/status`
- Snapshot of leader index, highest/locked QCs (`highest_qc`/`locked_qc`, heights, views, subject hashes), collector/VRF counters, pacemaker deferrals, tx queue depth, and RBC store health (`rbc_store.{sessions,bytes,pressure_level,persist_drops_total,evictions_total,recent_evictions[...]}`).
- GET `/v1/sumeragi/status/sse`
  - SSE stream (≈1s) of the same payload as `/v1/sumeragi/status` for live dashboards.
- GET `/v1/sumeragi/qc`
- Snapshot of highest/locked QCs; includes `subject_block_hash` for the highest QC when known.
- GET `/v1/sumeragi/pacemaker`
  - Pacemaker timers/config: `{ backoff_ms, rtt_floor_ms, jitter_ms, backoff_multiplier, rtt_floor_multiplier, max_backoff_ms, jitter_frac_permille }`.
- GET `/v1/sumeragi/leader`
  - Leader index snapshot. In NPoS mode, includes PRF context: `{ height, view, epoch_seed }`.
- GET `/v1/sumeragi/telemetry`
  - Aggregated consensus telemetry: `availability.collectors` contains observed collector indices, peer IDs, and ingested-vote counts; `rbc_backlog` contains missing-chunk totals; `rbc_pending` contains bounded pre-session queue totals, drops, and limits. This is not a deterministic collector plan or a per-session RBC contract.
- GET `/v1/sumeragi/params`
  - Snapshot of on-chain Sumeragi parameters `{ block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, redundant_send_r, da_enabled, next_mode, mode_activation_height, chain_height }`.
  - When `da_enabled` is true, availability evidence (availability votes or RBC `READY`) is tracked but does not gate commit; local payload is required and can be satisfied via RBC `DELIVER` or block sync. Use the aggregated telemetry endpoint, Prometheus counters, status snapshots, and logs to diagnose payload transport.

Evidence (audit; non-consensus)
- GET `/v1/sumeragi/evidence/count` → `{ "count": <u64> }`
- GET `/v1/sumeragi/evidence` → `{ "total": <u64>, "items": [...] }`
  - Includes basic fields (e.g., DoublePrepare/DoubleCommit, InvalidQc, InvalidProposal) for inspection.
  - Examples:
    - `curl -s http://127.0.0.1:8080/v1/sumeragi/evidence/count | jq .`
    - `curl -s http://127.0.0.1:8080/v1/sumeragi/evidence | jq .`
- POST `/v1/sumeragi/evidence` → `{ "status": "accepted", "kind": "<variant>" }`
  - CLI helpers:
    - `iroha --output-format text ops sumeragi evidence list`
    - `iroha --output-format text ops sumeragi evidence count`
    - `iroha ops sumeragi evidence submit --evidence-hex <hex>` (or `--evidence-hex-file <path>`)

Operator authentication (WebAuthn/mTLS)
- POST `/v1/operator/auth/registration/options`
  - Returns WebAuthn registration options (`publicKey`) for initial credential enrollment.
- POST `/v1/operator/auth/registration/verify`
  - Verifies the WebAuthn attestation payload and persists the operator credential.
- POST `/v1/operator/auth/login/options`
  - Returns WebAuthn authentication options (`publicKey`) for operator login.
- POST `/v1/operator/auth/login/verify`
  - Verifies the WebAuthn assertion payload and returns an operator session token.
- Headers:
  - `x-iroha-operator-session`: session token for operator endpoints (issued by login verify).
  - `x-iroha-operator-token`: bootstrap token (allowed when `torii.operator_auth.token_fallback` permits it).
  - `x-api-token`: required when `torii.require_api_token = true` or `torii.operator_auth.token_source = "api"`.
  - `x-forwarded-client-cert`: required when `torii.operator_auth.require_mtls = true` (set by the ingress proxy).
- Enrollment flow:
  1. Call registration options with a bootstrap token (only allowed before the first credential is enrolled when `token_fallback = "bootstrap"`).
  2. Run `navigator.credentials.create` in the operator UI and submit the attestation to registration verify.
  3. Call login options and login verify to obtain `x-iroha-operator-session`.
  4. Send `x-iroha-operator-session` on operator endpoints.

Notes
- These endpoints are node-local views (in-memory where noted) and do not affect consensus or persistence.
- Access may be guarded by API tokens, operator auth (WebAuthn/mTLS), and rate limits depending on your Torii configuration.

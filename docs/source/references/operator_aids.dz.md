---
lang: dz
direction: ltr
source: docs/source/references/operator_aids.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 058cade3ae411c84bff1ce5001471b59c2dbd34b5d6bc9b422708b0baa92681a
source_last_modified: "2026-01-28T18:22:38.971838+00:00"
translation_last_reviewed: 2026-02-07
---

# Torii Endpoints — Operator Aids (Quick Reference)

日本語の概要は [`operator_aids.ja.md`](./operator_aids.ja.md) を参照してください。

This page lists non-consensus, operator-facing endpoints that help with visibility and troubleshooting. Responses are JSON unless noted.

Consensus (Sumeragi)
- GET `/v2/sumeragi/new_view`
  - Snapshot of NEW_VIEW receipt counts per `(height, view)`.
  - Shape: `{ "ts_ms": <u64>, "items": [{ "height": <u64>, "view": <u64>, "count": <u64> }, ...] }`
  - Example:
    - `curl -s http://127.0.0.1:8080/v2/sumeragi/new_view | jq .`
- GET `/v2/sumeragi/new_view/sse` (SSE)
  - Periodic stream (≈1s) of the same payload for dashboards.
  - Example:
    - `curl -Ns http://127.0.0.1:8080/v2/sumeragi/new_view/sse`
- Metrics: `sumeragi_new_view_receipts_by_hv{height,view}` gauges mirror the counts.
- GET `/v2/sumeragi/status`
- Snapshot of leader index, highest/locked QCs (`highest_qc`/`locked_qc`, heights, views, subject hashes), collector/VRF counters, pacemaker deferrals, tx queue depth, and RBC store health (`rbc_store.{sessions,bytes,pressure_level,persist_drops_total,evictions_total,recent_evictions[...]}`).
- GET `/v2/sumeragi/status/sse`
  - SSE stream (≈1s) of the same payload as `/v2/sumeragi/status` for live dashboards.
- GET `/v2/sumeragi/qc`
- Snapshot of highest/locked QCs; includes `subject_block_hash` for the highest QC when known.
- GET `/v2/sumeragi/pacemaker`
  - Pacemaker timers/config: `{ backoff_ms, rtt_floor_ms, jitter_ms, backoff_multiplier, rtt_floor_multiplier, max_backoff_ms, jitter_frac_permille }`.
- GET `/v2/sumeragi/leader`
  - Leader index snapshot. In NPoS mode, includes PRF context: `{ height, view, epoch_seed }`.
- GET `/v2/sumeragi/collectors`
  - Deterministic collector plan derived from the committed topology and on-chain parameters: exports `mode`, plan `(height, view)` (with `height` equal to the current chain height), `collectors_k`, `redundant_send_r`, `proxy_tail_index`, `min_votes_for_commit`, the ordered collector list, and `epoch_seed` (hex) when NPoS is active.
- GET `/v2/sumeragi/params`
  - Snapshot of on-chain Sumeragi parameters `{ block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, redundant_send_r, da_enabled, next_mode, mode_activation_height, chain_height }`.
  - When `da_enabled` is true, availability evidence (availability votes or RBC `READY`) is tracked but does not gate commit; local payload is required and can be satisfied via RBC `DELIVER` or block sync. Operators can confirm payload transport health via the RBC endpoints below.
- GET `/v2/sumeragi/rbc`
  - Aggregate Reliable Broadcast counters: `{ sessions_active, sessions_pruned_total, ready_broadcasts_total, ready_rebroadcasts_skipped_total, deliver_broadcasts_total, payload_bytes_delivered_total, payload_rebroadcasts_skipped_total }`.
- GET `/v2/sumeragi/rbc/sessions`
  - Snapshot of per-session state (block hash, height/view, chunk counts, delivered flag, `invalid` marker, payload hash, recovered boolean) to troubleshoot stalled RBC deliveries and highlight recovered sessions after restart.
  - CLI shortcut: `iroha --output-format text ops sumeragi rbc sessions` prints `hash`, `height/view`, chunk progress, ready count, and invalid/delivered flags.

Evidence (audit; non-consensus)
- GET `/v2/sumeragi/evidence/count` → `{ "count": <u64> }`
- GET `/v2/sumeragi/evidence` → `{ "total": <u64>, "items": [...] }`
  - Includes basic fields (e.g., DoublePrepare/DoubleCommit, InvalidQc, InvalidProposal) for inspection.
  - Examples:
    - `curl -s http://127.0.0.1:8080/v2/sumeragi/evidence/count | jq .`
    - `curl -s http://127.0.0.1:8080/v2/sumeragi/evidence | jq .`
- POST `/v2/sumeragi/evidence` → `{ "status": "accepted", "kind": "<variant>" }`
  - CLI helpers:
    - `iroha --output-format text ops sumeragi evidence list`
    - `iroha --output-format text ops sumeragi evidence count`
    - `iroha ops sumeragi evidence submit --evidence-hex <hex>` (or `--evidence-hex-file <path>`)

Operator authentication (WebAuthn/mTLS)
- POST `/v2/operator/auth/registration/options`
  - Returns WebAuthn registration options (`publicKey`) for initial credential enrollment.
- POST `/v2/operator/auth/registration/verify`
  - Verifies the WebAuthn attestation payload and persists the operator credential.
- POST `/v2/operator/auth/login/options`
  - Returns WebAuthn authentication options (`publicKey`) for operator login.
- POST `/v2/operator/auth/login/verify`
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

CLI watch snippets (bash)

- Poll JSON snapshot every 2s (prints the latest 10 entries):

```bash
#!/usr/bin/env bash
set -euo pipefail
TORII="${TORII:-http://127.0.0.1:8080}"
INTERVAL="${INTERVAL:-2}"
TOKEN="${TOKEN:-}"
HDR=()
if [[ -n "$TOKEN" ]]; then HDR=(-H "x-api-token: $TOKEN"); fi
while true; do
  curl -s "${HDR[@]}" "$TORII/v2/sumeragi/new_view" \
    | jq -c '{ts_ms, items:(.items|sort_by([.height,.view])|reverse|.[:10])}'
  sleep "$INTERVAL"
done
```

- Follow the SSE stream and pretty‑print (latest 10 entries):

```bash
#!/usr/bin/env bash
set -euo pipefail
TORII="${TORII:-http://127.0.0.1:8080}"
TOKEN="${TOKEN:-}"
HDR=()
if [[ -n "$TOKEN" ]]; then HDR=(-H "x-api-token: $TOKEN"); fi
curl -Ns "${HDR[@]}" "$TORII/v2/sumeragi/new_view/sse" \
  | awk '/^data:/{sub(/^data: /,"\"); print}' \
  | jq -c '{ts_ms, items:(.items|sort_by([.height,.view])|reverse|.[:10])}'
```

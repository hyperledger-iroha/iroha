---
id: puzzle-service-operations
lang: zh-hans
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Puzzle Service Operations Guide
sidebar_label: Puzzle Service Ops
description: Operating the `soranet-puzzle-service` daemon for Argon2/ML-DSA admission tickets.
---

:::note Canonical Source
:::

# Puzzle Service Operations Guide

The `soranet-puzzle-service` daemon (`tools/soranet-puzzle-service/`) issues
Argon2-backed admission tickets that mirror the relay’s `pow.puzzle.*` policy
and, when configured, brokers ML-DSA admission tokens on behalf of edge relays.
It exposes five HTTP endpoints:

- `GET /healthz` – liveness probe.
- `GET /v1/puzzle/config` – returns the effective PoW/puzzle parameters pulled
  from the relay JSON (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v1/puzzle/mint` – mints an Argon2 ticket; an optional JSON body
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  requests a shorter TTL (clamped to the policy window), binds the ticket to a
  transcript hash, and returns a relay-signed ticket + signature fingerprint
  when signing keys are configured.
- `GET /v1/token/config` – when `pow.token.enabled = true`, returns the active
  admission-token policy (issuer fingerprint, TTL/clock-skew bounds, relay ID,
  and the merged revocation set).
- `POST /v1/token/mint` – mints an ML-DSA admission token bound to the supplied
  resume hash; the request body accepts `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.

Tickets produced by the service are verified in the
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`
integration test, which also exercises relay throttles during volumetric DoS
scenarios.【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## Configuring token issuance

Set the relay JSON fields under `pow.token.*` (see
`tools/soranet-relay/deploy/config/relay.entry.json` for an example) to enable
ML-DSA tokens. At minimum provide the issuer public key and optional
revocation list:

```json
"pow": {
  "token": {
    "enabled": true,
    "issuer_public_key_hex": "<ML-DSA-44 public key>",
    "revocation_list_hex": [],
    "revocation_list_path": "/etc/soranet/relay/token_revocations.json"
  }
}
```

The puzzle service reuses these values and automatically reloads the Norito
JSON revocation file at runtime. Use the `soranet-admission-token` CLI
(`cargo run -p soranet-relay --bin soranet_admission_token`) to mint and inspect
tokens offline, append `token_id_hex` entries to the revocation file, and audit
existing credentials before pushing updates to production.

Pass the issuer secret key to the puzzle service via the CLI flags:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` is also available when the secret is managed by an out-of-band
tooling pipeline. The revocation file watcher keeps `/v1/token/config` current;
coordinate updates with the `soranet-admission-token revoke` command to avoid lagging
revocation state.

Set `pow.signed_ticket_public_key_hex` in the relay JSON to advertise the ML-DSA-44 public
key used to verify signed PoW tickets; `/v1/puzzle/config` echoes the key and its BLAKE3
fingerprint (`signed_ticket_public_key_fingerprint_hex`) so clients can pin the verifier.
Signed tickets are validated against the relay ID and transcript bindings and share the same
revocation store; raw 74-byte PoW tickets remain valid when the signed-ticket verifier is
configured. Pass the signer secret via `--signed-ticket-secret-hex` or
`--signed-ticket-secret-path` when launching the puzzle service; startup rejects mismatched
keypairs if the secret does not validate against `pow.signed_ticket_public_key_hex`.
`POST /v1/puzzle/mint` accepts `"signed": true` (and optional `"transcript_hash_hex"`) to
return a Norito-encoded signed ticket alongside the raw ticket bytes; responses include
`signed_ticket_b64` and `signed_ticket_fingerprint_hex` to help track replay fingerprints.
Requests with `signed = true` are rejected if the signer secret is not configured.

## Key rotation playbook

1. **Collect the new descriptor commit.** Governance publishes the relay
   descriptor commit in the directory bundle. Copy the hex string into
   `handshake.descriptor_commit_hex` inside the relay JSON configuration shared
   with the puzzle service.
2. **Review puzzle policy bounds.** Confirm the updated
   `pow.puzzle.{memory_kib,time_cost,lanes}` values align with the release
   plan. Operators should keep the Argon2 configuration deterministic across
   relays (minimum 4 MiB memory, 1 ≤ lanes ≤ 16).
3. **Stage the restart.** Reload the systemd unit or container once governance
   announces the rotation cutover. The service has no hot-reload support; a
   restart is required to pick up the new descriptor commit.
4. **Validate.** Issue a ticket via `POST /v1/puzzle/mint` and confirm the
   returned `difficulty` and `expires_at` match the new policy. The soak report
   (`docs/source/soranet/reports/pow_resilience.md`) captures expected latency
   bounds for reference. When tokens are enabled, fetch `/v1/token/config` to
   ensure the advertised issuer fingerprint and revocation count match the
   expected values.

## Emergency disable procedure

1. Set `pow.puzzle.enabled = false` in the shared relay configuration. Keep
   `pow.required = true` if hashcash fallback tickets must remain mandatory.
2. Optionally enforce `pow.emergency` entries to reject stale descriptors while
   the Argon2 gate is offline.
3. Restart both the relay and the puzzle service to apply the change.
4. Monitor `soranet_handshake_pow_difficulty` to ensure the difficulty drops to
   the expected hashcash value, and verify `/v1/puzzle/config` reports
   `puzzle = null`.

## Monitoring and alerting

- **Latency SLO:** Track `soranet_handshake_latency_seconds` and keep the P95
  below 300 ms. The soak test offsets provide calibration data for guard
  throttles.【docs/source/soranet/reports/pow_resilience.md:1】
- **Quota pressure:** Use `soranet_guard_capacity_report.py` with relay metrics
  to tune `pow.quotas` cooldowns (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **Puzzle alignment:** `soranet_handshake_pow_difficulty` should match the
  difficulty returned by `/v1/puzzle/config`. Divergence indicates stale relay
  config or a failed restart.
- **Token readiness:** Alert if `/v1/token/config` drops to `enabled = false`
  unexpectedly or if `revocation_source` reports stale timestamps. Operators
  should rotate the Norito revocation file via the CLI whenever a token is
  retired to keep this endpoint accurate.
- **Service health:** Probe `/healthz` in the usual liveness cadence and alert
  if `/v1/puzzle/mint` returns HTTP 500 responses (indicates Argon2 parameter
  mismatch or RNG failures). Token minting errors surface through HTTP 4xx/5xx
  responses on `/v1/token/mint`; treat repeated failures as a paging condition.

## Compliance and audit logging

Relays emit structured `handshake` events that include throttle reasons and
cooldown durations. Ensure the compliance pipeline described in
`docs/source/soranet/relay_audit_pipeline.md` ingests these logs so puzzle
policy changes remain auditable. When the puzzle gate is enabled, archive the
minted ticket samples and the Norito configuration snapshot with the rollout
ticket for future audits. Admission tokens minted ahead of maintenance windows
should be tracked with their `token_id_hex` values and inserted into the
revocation file once they expire or are revoked.

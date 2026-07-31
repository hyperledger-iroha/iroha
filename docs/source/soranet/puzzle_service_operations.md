# Puzzle Service Operations Guide

The `soranet-puzzle-service` daemon (`tools/soranet-puzzle-service/`) issues
Argon2-backed admission tickets that mirror the relay’s `pow.puzzle.*` policy
and, when configured, brokers ML-DSA admission tokens on behalf of edge relays.
It exposes five HTTP endpoints:

- `GET /healthz` – liveness probe.
- `GET /v1/puzzle/config` – returns the effective PoW/puzzle parameters pulled
  from the relay JSON (`handshake.descriptor_commit_hex`, `pow.*`).
  The response also echoes the PoW revocation store capacity/TTL so operators
  can validate the shared replay cache settings.
- `POST /v1/puzzle/mint` – mints an Argon2 ticket. The JSON body must contain
  `"transcript_hash_hex"` with exactly 32 non-zero bytes; `"ttl_secs"` and
  `"signed"` are optional:
  `{ "transcript_hash_hex": "<32-byte hex>", "ttl_secs": <u64>, "signed": true }`.
  The service clamps TTL overrides to the policy window and returns a
  relay-signed ticket plus signature fingerprint when signing keys are
  configured.
- `GET /v1/token/config` – when `pow.token.enabled = true`, returns the active
  admission-token policy (issuer fingerprint, TTL/clock-skew bounds, relay ID,
  and the merged revocation set).
- `POST /v1/token/mint` – mints an ML-DSA admission token bound to the supplied
  resume hash; the request body accepts `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.

Tickets produced by the service are verified in the
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`
integration test, which also exercises relay throttles during volumetric DoS
scenarios.【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

The JavaScript SDK exposes a lightweight client (`SoranetPuzzleClient`) that
wraps these endpoints with typed DTOs and timeout/header handling so operators
and automation do not need bespoke HTTP glue (see
`javascript/iroha_js/README.md` for usage examples). Its
`mintPuzzleTicket(transcriptHashHex, options)` API requires the same non-zero
32-byte binding before it sends a request.

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
    "replay_store_capacity": 8192,
    "replay_store_path": "/var/lib/soranet-relay/token_replays.norito",
    "revocation_list_hex": [],
    "revocation_list_path": "/etc/soranet/relay/token_revocations.json"
  }
}
```

The replay store enforces single-use admission tokens with a bounded capacity
and retains each record through the token's late clock-skew allowance. Its
admissible retention window is derived from
`max_ttl_secs + 2 * clock_skew_secs`, covering both early and late skew edges,
so replay retention cannot undercut a token the verifier still accepts. The
relay always persists consumed `token_id_hex` entries at `replay_store_path`
and reloads them on restart. Active entries are never evicted: a full store
rejects new token admissions until records expire. A process-lifetime
exclusive sidecar lock makes a second ledger owner fail startup instead of
forking consumption state.
Empty, malformed, over-TTL, or over-capacity snapshots fail startup rather
than silently discarding replay history. Keep the TTL bound aligned with
`max_ttl_secs`, place the snapshot on durable storage, and size the capacity
to cover the expected client burst for the configured retention window.
Relay metrics expose `soranet_token_verify_total{issuer,relay,outcome}` counters
for acceptance, replay, expiry/TTL, mismatch, revocation, and store failures so
dashboards can alert on replay spikes or issuer/relay mismatches.

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

## Signed-ticket revocation store

Accepted puzzle/PoW tickets are consumed in a Norito replay snapshot on disk.
The store keys signed tickets by a BLAKE3 fingerprint of the ML-DSA signature
and unsigned tickets by a fingerprint of their canonical payload. Configure
`network.soranet_handshake.pow.{revocation_store_capacity,revocation_store_ttl_secs,revocation_store_path}`
for nodes, or the equivalent top-level `pow.*` fields for the standalone relay.
Place the snapshot on durable storage, keep the directory writable by the relay
user, and size the TTL/capacity to cover the longest accepted ticket lifetime
and peak issuance rate. Active entries are never evicted: a full store rejects
new handshakes until records expire. A missing snapshot starts as an empty
persistent store at the configured path; an unreadable or malformed snapshot
fails startup instead of silently discarding consumption history.
Telemetry exposes `soranet_privacy_pow_rejects_total{reason}` with `relay_mismatch`,
`replay`, and `store_error` reason labels so dashboards can distinguish cross-relay
presentation from genuine replay attempts and correlate spikes with store errors or relay
ID rotation. Alert on sustained `store_error` spikes; they usually indicate an
unwritable snapshot path, exhausted capacity, or corruption. A persistence
error on live traffic rejects the handshake; address the underlying filesystem
issue and purge expired entries once the store is writable again.
Tickets that exceed `revocation_store_ttl_secs` are rejected with the same `store_error`
label; keep the TTL cap aligned with `pow.ticket_ttl` (or lower it only when you
intentionally want shorter replay retention windows).
Set `pow.signed_ticket_public_key_hex` in the relay JSON to advertise the ML-DSA-44 public
key used to verify signed PoW tickets; the `/v1/puzzle/config` endpoint now echoes both the
public key and its BLAKE3 fingerprint (`signed_ticket_public_key_fingerprint_hex`) so clients
can pin the verifier key. Signed tickets are validated against the relay ID and transcript
bindings and still share the same revocation store. Relays with a configured
signed-ticket verifier key reject raw 74-byte PoW tickets; raw tickets are only
accepted by relays that do not configure a signed-ticket verifier key.
Pass the signer secret via `--signed-ticket-secret-hex` or `--signed-ticket-secret-path` when
launching the puzzle service; startup rejects mismatched keypairs if the secret does not
validate against `pow.signed_ticket_public_key_hex`. `POST /v1/puzzle/mint` accepts
`"signed": true` together with the required `"transcript_hash_hex"` to return a
Norito-encoded signed ticket alongside the raw ticket bytes; responses include
`signed_ticket_b64` and
`signed_ticket_fingerprint_hex` so clients can pin the replay fingerprint. Requests with
`signed = true` are rejected if the signer secret is not configured.
The p2p handshake path now records every accepted PoW ticket into the same Norito
snapshot and rejects handshakes while the cache is unavailable. Ops tooling can query
and prune the live cache via the in-process helpers (`active_revocations`,
`purge_expired_revocations`) to surface revocation counts on dashboards or to force a
deterministic purge without deleting the snapshot on disk.

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
4. **Validate.** Issue a ticket via `POST /v1/puzzle/mint` with a fresh,
   non-zero `transcript_hash_hex` and confirm the
   returned `difficulty` and `expires_at` match the new policy. The soak report
   (`docs/source/soranet/reports/pow_resilience.md`) captures expected latency
   bounds for reference. When tokens are enabled, fetch `/v1/token/config` to
   ensure the advertised issuer fingerprint and revocation count match the
   expected values.

## Emergency hardening procedure

1. Keep `pow.required = true`, use a non-zero difficulty, and leave the Argon2
   puzzle gate enabled. The first release has no operator-facing
   puzzle-disable path; startup and live config updates reject attempts to make
   PoW optional, set zero difficulty, or clear puzzle admission.
2. Enforce `pow.emergency` entries to reject stale descriptors while the service
   is degraded.
3. Restart both the relay and the puzzle service after cost or emergency-list
   changes.
4. Monitor `soranet_handshake_pow_difficulty` to ensure the advertised policy
   matches the expected Argon2-backed value, and verify `/v1/puzzle/config` reports
   the active puzzle parameters.

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
- **Token outcomes:** Track `soranet_token_verify_total{issuer,relay,outcome}`.
  Sustained increases in `outcome=replay|ttl_exceeded|store_error|issuer_mismatch`
  indicate token reuse, clock skew, or misconfiguration; page when replay/error
  counts exceed baseline.
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

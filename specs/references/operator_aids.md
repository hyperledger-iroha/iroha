# Torii Endpoints — Operator Aids (Quick Reference)

This page lists non-consensus, operator-facing endpoints that help with visibility and troubleshooting. Responses are JSON unless noted.

All finite Sumeragi reads below require a fresh exact-`NetworkId` operator
request signature. The CLI examples therefore name an explicit runtime-only
operator key file; API tokens, account keys, redirects, and retries are not
substitutes. The protocol-handshake `/v1/sumeragi/status/sse` stream is the
intentional public exception.

Consensus (Sumeragi)
- Metrics: `sumeragi_new_view_receipts_by_hv{height,view}` gauges mirror the counts.
- GET `/v1/sumeragi/status`
  - Authoritative revision-4 reducer and operator snapshot: protocol/shared-context fingerprints, height/view/phase/leader, QC and TimeoutCertificate references, body/persistence state, latest durable commit, bounded lane state, adapter queues, and transaction queue.
- GET `/v1/sumeragi/status/sse`
  - SSE stream (≈1s) of the same payload as `/v1/sumeragi/status` for live dashboards.
- GET `/v1/sumeragi/qc`
- Snapshot of highest/locked QCs; includes `subject_block_hash` for the highest QC when known.
- GET `/v1/sumeragi/pacemaker`
  - Non-authoritative legacy-labeled gauge snapshot. Backoff, RTT, and jitter fields do not configure revision-4 deadlines and may remain zero.
- GET `/v1/sumeragi/leader`
  - Leader index snapshot. In NPoS mode, includes PRF context: `{ height, view, epoch_seed }`.
- GET `/v1/sumeragi/telemetry`
  - Legacy-labeled aggregate transport observations. Collector/RBC-named fields are not authoritative revision-4 state and may remain zero; correlate authenticated status with consensus logs instead.
- GET `/v1/sumeragi/params`
  - Compatibility snapshot of governed NPoS/V1 parameter records. It does not replace signed revision-4 height context or the shared configuration fingerprint.

Evidence (audit; non-consensus)
- GET `/v1/sumeragi/evidence/count` → `{ "count": <u64> }`
- GET `/v1/sumeragi/evidence` → `{ "total": <u64>, "items": [...] }`
  - Includes basic fields (e.g., DoublePrepare/DoubleCommit, InvalidQc, InvalidProposal) for inspection.
  - CLI helpers:
    - `iroha --operator-private-key-file /run/secrets/iroha/operator.key --output-format text ops sumeragi evidence list`
    - `iroha --operator-private-key-file /run/secrets/iroha/operator.key --output-format text ops sumeragi evidence count`
  - Evidence admission is consensus-authenticated; Torii has no mutation endpoint.

Operator authentication (exact request signature, optional WebAuthn/mTLS second factor)
- POST `/v1/operator/auth/registration/options`
  - Returns WebAuthn registration options (`publicKey`) for initial credential enrollment.
- POST `/v1/operator/auth/registration/verify`
  - Verifies the WebAuthn attestation payload and persists the operator credential.
- POST `/v1/operator/auth/login/options`
  - Returns WebAuthn authentication options (`publicKey`) for operator login.
- POST `/v1/operator/auth/login/verify`
  - Verifies the WebAuthn assertion payload and returns an operator session token.
- Headers:
  - `x-iroha-operator-public-key`, `x-iroha-operator-timestamp-ms`, `x-iroha-operator-nonce`, and `x-iroha-operator-signature`: mandatory on every route cataloged as `OperatorSignature`. The signature covers the exact runtime `NetworkId`, HTTP method, path, sorted query, raw body hash, timestamp, and nonce.
  - `x-iroha-operator-session`: optional second-factor session token issued by login verify when `[torii.operator_auth]` is enabled. It never replaces the exact request signature.
  - `x-iroha-operator-token`: bootstrap/second-factor token when `torii.operator_auth.token_fallback` permits it. It never replaces the exact request signature.
  - `x-api-token`: listener credential when `torii.require_api_token = true`, or an optional operator-auth second factor when `torii.operator_auth.token_source = "api"`; it never replaces the exact request signature.
  - `x-forwarded-client-cert`: required when `torii.operator_auth.require_mtls = true` (set by the ingress proxy).
- Enrollment flow:
  1. Call registration options with a bootstrap token (only allowed before the first credential is enrolled when `token_fallback = "bootstrap"`).
  2. Run `navigator.credentials.create` in the operator UI and submit the attestation to registration verify.
  3. Call login options and login verify to obtain `x-iroha-operator-session`.
  4. Send `x-iroha-operator-session` together with a fresh exact-network operator request signature on each operator endpoint. If `[torii.operator_auth]` is disabled, the exact request signature remains mandatory by itself.

Notes
- These endpoints are node-local views (in-memory where noted) and do not affect consensus or persistence.
- Operator routes always require an allow-listed exact-network request signature. API tokens, WebAuthn/mTLS, and rate limits can add gates but cannot weaken or replace it.

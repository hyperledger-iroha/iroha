# Torii Endpoints — Operator Aids (Quick Reference)

This page lists read-only, operator-facing endpoints that help with visibility and troubleshooting. Some routes project committed consensus state; responses are JSON unless noted.

All finite Sumeragi reads below require a fresh exact-`NetworkId` operator
request signature. The CLI examples therefore name an explicit runtime-only
operator key file; API tokens, account keys, redirects, and retries are not
substitutes. The `/v1/sumeragi/status/sse` stream uses the same operator
request-signature boundary before opening the long-lived response.

Consensus (Sumeragi)
- GET `/v1/sumeragi/status`
  - Authoritative snapshot of the node's Sumeragi core (`SumeragiStatus`, spec §12.1): protocol version, signed-genesis configuration fingerprint, instance id, height, view and routing stage, leader and proxy tail, the lock's view, pacemaker level and start level, retransmission interval, committed and applied heights, whether the node awaits the next configuration, the signing key (absent on an observer), `unanchored` and `abstaining`, halt reason, memory footprint, and the beacon horizon.
- GET `/v1/sumeragi/status/sse`
  - Operator-authenticated SSE stream (≈1s) of the same payload as `/v1/sumeragi/status` for live dashboards.
- GET `/v1/sumeragi/diagnostics`
  - Non-authoritative operator and lane diagnostics.
- GET `/v1/sumeragi/lanes`
  - The global chain's lane records (`specs/sumeragi_lanes.md`) and the node's lane instances.
- GET `/v1/sumeragi/bls-keys` and `/v1/sumeragi/consensus-keys`
  - The consensus key rosters.
- GET `/v1/sumeragi/params`
  - Governed NPoS parameter records.

Evidence (read-only committed consensus audit)
- GET `/v1/sumeragi/evidence/count` → `{ "count": <u64> }`
- GET `/v1/sumeragi/evidence` → `{ "total": <u64>, "items": [...] }`
  - Each item is the sole first-release kind `NativeSumeragiEvidence`: its `class` (`proposal`, `phase_vote`, `timeout_vote`, `invalid_proposal` or `conflicting_certificates`), instance, height, epoch, context id, authority generation, offenders, `safety_violation`, the native frame hash, the recording height/view/time and the penalty status.
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
  - `x-iroha-operator-token`: dedicated bootstrap token accepted by the registration routes only before the first WebAuthn credential exists. It never authorizes an operator route or replaces the exact request signature.
  - `x-api-token`: listener credential when `torii.require_api_token = true`; it is never an operator-auth bootstrap credential or second factor.
  - `x-forwarded-client-cert`: required when `torii.operator_auth.require_mtls = true` (set by the ingress proxy).
- Enrollment flow:
  1. Call registration options with a configured `tokens` entry in `x-iroha-operator-token`; this is accepted only before the first credential is enrolled.
  2. Run `navigator.credentials.create` in the operator UI and submit the attestation to registration verify.
  3. Call login options and login verify to obtain `x-iroha-operator-session`.
  4. Send `x-iroha-operator-session` together with a fresh exact-network operator request signature on each operator endpoint. If `[torii.operator_auth]` is disabled, the exact request signature remains mandatory by itself.

Notes
- Status, metrics, and other explicitly in-memory diagnostics are node-local views and do not mutate consensus or persistence. The evidence routes instead project the canonical evidence records already persisted in WSV; unadmitted node-local proofs are not exposed.
- Operator routes always require an allow-listed exact-network request signature. After the first WebAuthn enrollment, they additionally require a valid operator session; API tokens never satisfy this boundary.

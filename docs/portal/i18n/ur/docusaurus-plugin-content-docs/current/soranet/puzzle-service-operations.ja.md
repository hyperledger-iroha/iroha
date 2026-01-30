---
lang: ja
direction: ltr
source: docs/portal/i18n/ur/docusaurus-plugin-content-docs/current/soranet/puzzle-service-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4ee21ae6629b662d10abe1166b47b2f6f7535aaea6177d9cdd2455f3c704d80a
source_last_modified: "2025-11-14T04:43:22.492903+00:00"
translation_last_reviewed: 2026-01-30
---

:::note Canonical Source
`docs/source/soranet/puzzle_service_operations.md` کی عکاسی کرتا ہے۔ جب تک پرانا documentation set retire نہ ہو، دونوں ورژنز sync رکھیں۔
:::

# Puzzle Service Operations Guide

`tools/soranet-puzzle-service/` کا `soranet-puzzle-service` daemon
Argon2-backed admission tickets جاری کرتا ہے جو relay کی `pow.puzzle.*` policy
کو mirror کرتے ہیں، اور جب configure ہو تو edge relays کی جانب سے ML-DSA
admission tokens broker کرتا ہے۔ یہ پانچ HTTP endpoints expose کرتا ہے:

- `GET /healthz` - liveness probe.
- `GET /v1/puzzle/config` - relay JSON (`handshake.descriptor_commit_hex`, `pow.*`) سے
  اٹھائے گئے موثر PoW/puzzle parameters واپس کرتا ہے۔
- `POST /v1/puzzle/mint` - Argon2 ticket mint کرتا ہے؛ optional JSON body
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  کم TTL کی درخواست کرتا ہے (policy window تک clamp)، ticket کو transcript hash
  سے bind کرتا ہے، اور signing keys configured ہوں تو relay-signed ticket +
  signature fingerprint واپس کرتا ہے۔
- `GET /v1/token/config` - جب `pow.token.enabled = true` ہو تو active admission-token
  policy واپس کرتا ہے (issuer fingerprint، TTL/clock-skew bounds، relay ID، اور
  merged revocation set).
- `POST /v1/token/mint` - ML-DSA admission token mint کرتا ہے جو supplied resume hash
  سے bound ہوتا ہے؛ request body `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`
  قبول کرتا ہے۔

Service کے بنائے گئے tickets کو integration test
`volumetric_dos_soak_preserves_puzzle_and_latency_slo` میں verify کیا جاتا ہے، جو
volumetric DoS scenarios کے دوران relay throttles بھی exercise کرتا ہے۔
【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## Token issuance configure کرنا

`pow.token.*` کے تحت relay JSON fields set کریں (مثال کے لئے
`tools/soranet-relay/deploy/config/relay.entry.json` دیکھیں) تاکہ ML-DSA tokens
enable ہوں۔ کم از کم issuer public key اور optional revocation list فراہم کریں:

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

Puzzle service انہی values کو reuse کرتا ہے اور runtime میں Norito JSON revocation
فائل کو خودکار طور پر reload کرتا ہے۔ CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) استعمال کریں تاکہ
tokens offline mint/inspect ہوں، revocation فائل میں `token_id_hex` entries append ہوں،
اور production updates سے پہلے موجودہ credentials audit ہوں۔

Issuer secret key کو CLI flags کے ذریعے puzzle service میں پاس کریں:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` بھی دستیاب ہے جب secret out-of-band tooling pipeline کے ذریعے manage ہو۔
Revocation file watcher `/v1/token/config` کو current رکھتا ہے؛ updates کو
`soranet-admission-token revoke` کمانڈ کے ساتھ coordinate کریں تاکہ revocation state
lag نہ کرے۔

Relay JSON میں `pow.signed_ticket_public_key_hex` set کریں تاکہ signed PoW tickets
verify کرنے کے لئے ML-DSA-44 public key advertise ہو؛ `/v1/puzzle/config` یہ key اور
اس کا BLAKE3 fingerprint (`signed_ticket_public_key_fingerprint_hex`) echo کرتا ہے تاکہ
clients verifier pin کر سکیں۔ Signed tickets relay ID اور transcript bindings کے خلاف
validate ہوتے ہیں اور اسی revocation store کو share کرتے ہیں؛ raw 74-byte PoW tickets
signed-ticket verifier configured ہونے پر بھی valid رہتے ہیں۔ Signer secret کو
`--signed-ticket-secret-hex` یا `--signed-ticket-secret-path` کے ذریعے service launch
پر pass کریں؛ startup mismatched keypairs reject کرتا ہے اگر secret
`pow.signed_ticket_public_key_hex` کے خلاف validate نہ ہو۔ `POST /v1/puzzle/mint`
`"signed": true` (اور optional `"transcript_hash_hex"`) قبول کرتا ہے تاکہ Norito-encoded
signed ticket raw ticket bytes کے ساتھ واپس ہو؛ responses میں `signed_ticket_b64`
اور `signed_ticket_fingerprint_hex` شامل ہوتے ہیں تاکہ replay fingerprints track ہوں۔
` signed = true` والی requests reject ہوتی ہیں اگر signer secret configured نہ ہو۔

## Key rotation playbook

1. **نیا descriptor commit جمع کریں۔** Governance directory bundle میں relay
   descriptor commit publish کرتی ہے۔ Hex string کو relay JSON config میں
   `handshake.descriptor_commit_hex` میں copy کریں جو puzzle service کے ساتھ shared ہے۔
2. **Puzzle policy bounds review کریں۔** تصدیق کریں کہ updated
   `pow.puzzle.{memory_kib,time_cost,lanes}` values release plan کے مطابق ہیں۔ Operators کو
   Argon2 configuration relays میں deterministic رکھنا چاہئے (کم از کم 4 MiB memory،
   1 <= lanes <= 16).
3. **Restart stage کریں۔** Governance rotation cutover announce کرے تو systemd unit یا
   container reload کریں۔ Service میں hot-reload نہیں ہے؛ نیا descriptor commit لینے کے لئے
   restart ضروری ہے۔
4. **Validate کریں۔** `POST /v1/puzzle/mint` کے ذریعے ticket issue کریں اور تصدیق کریں کہ
   `difficulty` اور `expires_at` نئی policy سے match ہوں۔ Soak report
   (`docs/source/soranet/reports/pow_resilience.md`) reference کے لئے expected latency bounds
   capture کرتا ہے۔ Tokens enable ہوں تو `/v1/token/config` fetch کریں تاکہ advertised issuer
   fingerprint اور revocation count expected values سے match ہوں۔

## Emergency disable procedure

1. Shared relay configuration میں `pow.puzzle.enabled = false` set کریں۔
   `pow.required = true` رکھیں اگر hashcash fallback tickets لازمی رہنے چاہئیں۔
2. Optional طور پر `pow.emergency` entries enforce کریں تاکہ Argon2 gate offline ہونے پر
   stale descriptors reject ہوں۔
3. Relay اور puzzle service دونوں restart کریں تاکہ تبدیلی apply ہو۔
4. `soranet_handshake_pow_difficulty` monitor کریں تاکہ difficulty expected hashcash value
   تک drop ہو، اور verify کریں کہ `/v1/puzzle/config` `puzzle = null` رپورٹ کرے۔

## Monitoring اور alerting

- **Latency SLO:** `soranet_handshake_latency_seconds` track کریں اور P95 کو 300 ms سے نیچے رکھیں۔
  Soak test offsets guard throttles کے لئے calibration data فراہم کرتے ہیں۔
  【docs/source/soranet/reports/pow_resilience.md:1】
- **Quota pressure:** `soranet_guard_capacity_report.py` کو relay metrics کے ساتھ استعمال کریں تاکہ
  `pow.quotas` cooldowns (`soranet_abuse_remote_cooldowns`, `soranet_handshake_throttled_remote_quota_total`) tune ہوں۔
  【docs/source/soranet/relay_audit_pipeline.md:68】
- **Puzzle alignment:** `soranet_handshake_pow_difficulty` کو `/v1/puzzle/config` سے واپس ہونے والی
  difficulty کے ساتھ match ہونا چاہئے۔ Divergence stale relay config یا failed restart کی نشاندہی ہے۔
- **Token readiness:** اگر `/v1/token/config` غیر متوقع طور پر `enabled = false` ہو جائے یا
  `revocation_source` stale timestamps رپورٹ کرے تو alert کریں۔ Operators کو CLI کے ذریعے Norito
  revocation file rotate کرنا چاہئے جب کوئی token retire ہو تاکہ یہ endpoint درست رہے۔
- **Service health:** `/healthz` کو معمول کی liveness cadence پر probe کریں اور alert کریں اگر
  `/v1/puzzle/mint` HTTP 500 responses دے (Argon2 parameter mismatch یا RNG failures کی نشاندہی).
  Token minting errors `/v1/token/mint` پر HTTP 4xx/5xx responses کے ذریعے نظر آتے ہیں؛ repeated failures
  کو paging condition سمجھیں۔

## Compliance اور audit logging

Relays structured `handshake` events emit کرتے ہیں جن میں throttle reasons اور cooldown durations شامل ہوتے ہیں۔
یقینی بنائیں کہ `docs/source/soranet/relay_audit_pipeline.md` میں بیان کردہ compliance pipeline ان logs کو ingest کرے
تاکہ puzzle policy changes auditable رہیں۔ Puzzle gate enable ہو تو minted ticket samples اور Norito configuration
snapshot کو rollout ticket کے ساتھ archive کریں تاکہ مستقبل کے audits کے لئے دستیاب ہوں۔ Maintenance windows سے پہلے
mint کئے گئے admission tokens کو ان کے `token_id_hex` values کے ساتھ track کیا جانا چاہئے اور expire یا revoke ہونے
پر revocation file میں insert کیا جانا چاہئے۔

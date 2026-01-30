---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/soranet/puzzle-service-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 95cbdd5a47e0f49be091323776c9670101705cc546ae6e3444e2a580ab4c0212
source_last_modified: "2025-11-14T04:43:22.490244+00:00"
translation_last_reviewed: 2026-01-30
---

:::note Canonical Source
`docs/source/soranet/puzzle_service_operations.md` を反映する。従来の docs が廃止されるまで両方を同期しておくこと。
:::

# Puzzle Service 運用ガイド

`tools/soranet-puzzle-service/` の `soranet-puzzle-service` daemon は、relay の
`pow.puzzle.*` policy を反映した Argon2 バックの admission tickets を発行し、
設定されていれば edge relays の代わりに ML-DSA admission tokens を仲介する。
5 つの HTTP endpoints を公開する:

- `GET /healthz` - liveness probe。
- `GET /v1/puzzle/config` - relay JSON (`handshake.descriptor_commit_hex`, `pow.*`) から
  取得した有効な PoW/puzzle parameters を返す。
- `POST /v1/puzzle/mint` - Argon2 ticket を発行する; optional JSON body
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  は短い TTL を要求 (policy window に clamp)、ticket を transcript hash に
  bind し、署名キーが設定されていれば relay-signed ticket + signature fingerprint
  を返す。
- `GET /v1/token/config` - `pow.token.enabled = true` のとき、active admission-token policy
  (issuer fingerprint, TTL/clock-skew bounds, relay ID, merged revocation set) を返す。
- `POST /v1/token/mint` - supply された resume hash に bind された ML-DSA admission token を
  発行; request body は `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`
  を受け付ける。

Service が生成する tickets は integration test
`volumetric_dos_soak_preserves_puzzle_and_latency_slo` で検証され、volumetric DoS
シナリオにおける relay throttles も同時に検証する。【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## Token 発行の設定

`pow.token.*` 配下の relay JSON fields を設定する (例: `tools/soranet-relay/deploy/config/relay.entry.json`) と
ML-DSA tokens が有効になる。最低限 issuer public key と optional revocation list を用意する:

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

Puzzle service はこれらの値を再利用し、Norito JSON revocation ファイルを runtime で自動リロードする。
CLI `soranet-admission-token` (`cargo run -p soranet-relay --bin soranet_admission_token`) を使い、
offline で tokens を発行/検査し、`token_id_hex` entries を revocation ファイルに追記し、
production に更新を反映する前に既存 credentials を監査する。

Issuer secret key を CLI flags で puzzle service に渡す:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` は secret を out-of-band tooling pipeline で管理する場合に利用できる。
Revocation file watcher は `/v1/token/config` を最新に保つ; `soranet-admission-token revoke`
コマンドと更新を調整し、revocation state の遅れを避ける。

Relay JSON に `pow.signed_ticket_public_key_hex` を設定して、signed PoW tickets を検証する
ML-DSA-44 public key を広告する; `/v1/puzzle/config` は key とその BLAKE3 fingerprint
(`signed_ticket_public_key_fingerprint_hex`) をエコーし、clients が verifier を pin できる。
Signed tickets は relay ID と transcript bindings に対して検証され、同じ revocation store を共有する;
Signed-ticket verifier が設定されていても raw 74-byte PoW tickets は有効のまま。Puzzle service の
起動時に `--signed-ticket-secret-hex` または `--signed-ticket-secret-path` で signer secret を渡す;
secret が `pow.signed_ticket_public_key_hex` に合致しない場合、起動時に keypair mismatch を拒否する。
`POST /v1/puzzle/mint` は `"signed": true` (optional `"transcript_hash_hex"`) を受け付け、
raw ticket bytes と並行して Norito エンコードの signed ticket を返す; responses には
`signed_ticket_b64` と `signed_ticket_fingerprint_hex` が含まれ replay fingerprints を追跡できる。
`signed = true` のリクエストは signer secret が未設定の場合に拒否される。

## Key rotation playbook

1. **新しい descriptor commit を収集。** Governance が directory bundle に relay descriptor commit を公開する。
   hex string を relay JSON の `handshake.descriptor_commit_hex` にコピーし、puzzle service と共有する。
2. **Puzzle policy bounds の確認。** 更新された `pow.puzzle.{memory_kib,time_cost,lanes}` が release plan に
   合致することを確認する。Operators は relays 間で Argon2 設定を決定的に保つこと
   (最小 4 MiB メモリ、1 <= lanes <= 16)。
3. **再起動のステージング。** Governance が rotation cutover を告知したら systemd unit または container を
   リロードする。サービスは hot-reload をサポートしないため、新しい descriptor commit を反映するには
   再起動が必要。
4. **検証。** `POST /v1/puzzle/mint` で ticket を発行し、返却された `difficulty` と `expires_at` が
   新しい policy と一致することを確認する。Soak report (`docs/source/soranet/reports/pow_resilience.md`) が
   参考として期待 latency bounds を記録している。Tokens が有効な場合は `/v1/token/config` を取得し、
   advertised issuer fingerprint と revocation count が期待値と一致することを確認する。

## Emergency disable 手順

1. 共有 relay configuration で `pow.puzzle.enabled = false` を設定する。Hashcash fallback tickets を
   必須にする必要がある場合は `pow.required = true` を維持する。
2. オプションで `pow.emergency` entries を強制し、Argon2 gate が offline の間に stale descriptors を拒否する。
3. Relay と puzzle service の両方を再起動して変更を反映する。
4. `soranet_handshake_pow_difficulty` を監視し、難易度が期待される hashcash 値に下がることを確認し、
   `/v1/puzzle/config` が `puzzle = null` を報告することを検証する。

## Monitoring と alerting

- **Latency SLO:** `soranet_handshake_latency_seconds` を追跡し、P95 を 300 ms 以下に保つ。Soak test offsets は
  guard throttles の calibration data を提供する。【docs/source/soranet/reports/pow_resilience.md:1】
- **Quota pressure:** `soranet_guard_capacity_report.py` と relay metrics を使い、`pow.quotas` cooldowns
  (`soranet_abuse_remote_cooldowns`, `soranet_handshake_throttled_remote_quota_total`) を調整する。
  【docs/source/soranet/relay_audit_pipeline.md:68】
- **Puzzle alignment:** `soranet_handshake_pow_difficulty` は `/v1/puzzle/config` の difficulty と一致すべき。
  乖離は stale relay config または restart failure を示す。
- **Token readiness:** `/v1/token/config` が予期せず `enabled = false` になるか、`revocation_source` が stale
  timestamps を報告したら alert。Operators は token が退役したら CLI で Norito revocation file を回転し、
  この endpoint を正確に保つべき。
- **Service health:** 通常の liveness cadence で `/healthz` を probe し、`/v1/puzzle/mint` が HTTP 500 を返したら
  alert (Argon2 parameter mismatch または RNG failures を示す)。Token minting errors は `/v1/token/mint` の
  HTTP 4xx/5xx responses に現れるため、繰り返し失敗は paging condition として扱う。

## Compliance と audit logging

Relays は throttle reasons と cooldown durations を含む構造化 `handshake` events を出力する。
`docs/source/soranet/relay_audit_pipeline.md` に記載された compliance pipeline がこれらの logs を
取り込むようにし、puzzle policy の変更を監査可能にする。Puzzle gate が有効な場合は、発行した
ticket のサンプルと Norito configuration snapshot を rollout ticket とともにアーカイブし、将来の
監査に備える。Maintenance window 前に mint された admission tokens は `token_id_hex` 値で追跡し、
有効期限切れまたは revoke されたら revocation file に挿入する。

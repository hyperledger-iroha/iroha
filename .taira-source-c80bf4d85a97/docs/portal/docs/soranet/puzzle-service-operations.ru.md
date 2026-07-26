---
lang: ru
direction: ltr
source: docs/portal/docs/soranet/puzzle-service-operations.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2e27636b3e6916fdbd6d9e7723f7c331d65c9486021849b24f6fd3e32daed4a8
source_last_modified: "2025-11-15T19:17:38.777873+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: puzzle-service-operations
title: Руководство по эксплуатации Puzzle Service
sidebar_label: Puzzle Service Ops
description: Эксплуатация daemon `soranet-puzzle-service` для Argon2/ML-DSA admission tickets.
---

:::note Канонический источник
:::

# Руководство по эксплуатации Puzzle Service

Daemon `soranet-puzzle-service` (`tools/soranet-puzzle-service/`) выпускает
Argon2-backed admission tickets, которые отражают policy `pow.puzzle.*` у relay
и, когда настроено, брокерит ML-DSA admission tokens от имени edge relays.
Он предоставляет пять HTTP endpoints:

- `GET /healthz` - liveness probe.
- `GET /v1/puzzle/config` - возвращает эффективные параметры PoW/puzzle,
  считанные из relay JSON (`handshake.descriptor_commit_hex`, `pow.*`).
- `POST /v1/puzzle/mint` - выпускает Argon2 ticket; optional JSON body
  `{ "ttl_secs": <u64>, "transcript_hash_hex": "<32-byte hex>", "signed": true }`
  запрашивает более короткий TTL (clamped к policy window), привязывает ticket
  к transcript hash и возвращает relay-signed ticket + signature fingerprint
  при наличии signing keys.
- `GET /v1/token/config` - когда `pow.token.enabled = true`, возвращает активную
  admission-token policy (issuer fingerprint, TTL/clock-skew bounds, relay ID,
  и merged revocation set).
- `POST /v1/token/mint` - выпускает ML-DSA admission token, связанный с supplied
  resume hash; body принимает `{ "transcript_hash_hex": "...", "ttl_secs": <u64>, "flags": <u8> }`.

Tickets, выпущенные сервисом, проверяются в интеграционном тесте
`volumetric_dos_soak_preserves_puzzle_and_latency_slo`, который также
упражняет relay throttles во время volumetric DoS сценариев.【tools/soranet-relay/tests/adaptive_and_puzzle.rs:337】

## Настройка выпуска токенов

Задайте поля relay JSON под `pow.token.*` (см.
`tools/soranet-relay/deploy/config/relay.entry.json` как пример), чтобы
включить ML-DSA tokens. Минимально предоставьте issuer public key и optional
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

Puzzle service повторно использует эти значения и автоматически перезагружает
Norito JSON файл revocation в runtime. Используйте CLI `soranet-admission-token`
(`cargo run -p soranet-relay --bin soranet_admission_token`) чтобы выпускать и
проверять tokens offline, добавлять `token_id_hex` entries в revocation файл и
проводить audit существующих credentials до публикации updates в production.

Передайте issuer secret key в puzzle service через CLI flags:

```bash
cargo run -p soranet-puzzle-service -- \
  --relay-config /etc/soranet/relay/relay.entry.json \
  --token-secret-path /etc/soranet/relay/token_issuer_secret.hex \
  --token-revocation-file /etc/soranet/relay/token_revocations.json \
  --token-revocation-refresh-secs 60
```

`--token-secret-hex` доступен, когда secret управляется out-of-band tooling
pipeline. Revocation file watcher держит `/v1/token/config` актуальным;
координируйте обновления с командой `soranet-admission-token revoke`, чтобы
избежать отставания revocation state.

Установите `pow.signed_ticket_public_key_hex` в relay JSON, чтобы объявить
ML-DSA-44 public key для проверки signed PoW tickets; `/v1/puzzle/config` эхо
возвращает key и его BLAKE3 fingerprint (`signed_ticket_public_key_fingerprint_hex`),
чтобы clients могли pin-ить verifier. Signed tickets проверяются по relay ID и
transcript bindings и используют тот же revocation store; raw 74-byte PoW tickets
остаются валидными при настроенном signed-ticket verifier. Передайте signer secret
через `--signed-ticket-secret-hex` или `--signed-ticket-secret-path` при запуске
puzzle service; старт отклонит несоответствующие keypairs, если secret не валидируется
против `pow.signed_ticket_public_key_hex`. `POST /v1/puzzle/mint` принимает
`"signed": true` (и optional `"transcript_hash_hex"`) чтобы вернуть
Norito-encoded signed ticket вместе с raw ticket bytes; ответы включают
`signed_ticket_b64` и `signed_ticket_fingerprint_hex` для отслеживания replay fingerprints.
Запросы с `signed = true` отклоняются, если signer secret не настроен.

## Playbook ротации ключей

1. **Соберите новый descriptor commit.** Governance публикует relay descriptor
   commit в directory bundle. Скопируйте hex string в `handshake.descriptor_commit_hex`
   внутри relay JSON конфигурации, которой пользуется puzzle service.
2. **Проверьте bounds policy puzzle.** Подтвердите, что обновленные значения
   `pow.puzzle.{memory_kib,time_cost,lanes}` соответствуют release плану. Операторы
   должны держать Argon2 конфигурацию детерминированной между relays (минимум 4 MiB
   памяти, 1 <= lanes <= 16).
3. **Подготовьте рестарт.** Перезагрузите systemd unit или container после того, как
   governance объявит rotation cutover. Сервис не поддерживает hot-reload; для
   применения нового descriptor commit требуется restart.
4. **Провалидируйте.** Выпустите ticket через `POST /v1/puzzle/mint` и подтвердите,
   что `difficulty` и `expires_at` соответствуют новой policy. Soak report
   (`docs/source/soranet/reports/pow_resilience.md`) содержит ожидаемые latency bounds
   для справки. Когда tokens включены, запросите `/v1/token/config`, чтобы убедиться,
   что advertised issuer fingerprint и revocation count соответствуют ожидаемым значениям.

## Emergency disable процедура

1. Установите `pow.puzzle.enabled = false` в общей relay конфигурации. Оставьте
   `pow.required = true`, если hashcash fallback tickets должны оставаться обязательными.
2. Опционально примените `pow.emergency` entries, чтобы отклонять устаревшие
   descriptors пока Argon2 gate offline.
3. Перезапустите relay и puzzle service, чтобы применить изменение.
4. Мониторьте `soranet_handshake_pow_difficulty`, чтобы убедиться, что сложность
   упала до ожидаемого hashcash значения, и проверьте, что `/v1/puzzle/config`
   сообщает `puzzle = null`.

## Monitoring и alerting

- **Latency SLO:** Отслеживайте `soranet_handshake_latency_seconds` и держите P95
  ниже 300 ms. Soak test offsets дают calibration data для guard throttles.
  【docs/source/soranet/reports/pow_resilience.md:1】
- **Quota pressure:** Используйте `soranet_guard_capacity_report.py` с relay metrics
  для настройки `pow.quotas` cooldowns (`soranet_abuse_remote_cooldowns`,
  `soranet_handshake_throttled_remote_quota_total`).【docs/source/soranet/relay_audit_pipeline.md:68】
- **Puzzle alignment:** `soranet_handshake_pow_difficulty` должен совпадать с
  difficulty из `/v1/puzzle/config`. Дивергенция указывает на stale relay config
  или неудачный restart.
- **Token readiness:** Alert, если `/v1/token/config` неожиданно падает до
  `enabled = false` или `revocation_source` сообщает stale timestamps. Операторы
  должны ротировать Norito revocation file через CLI при выводе токена, чтобы
  endpoint оставался точным.
- **Service health:** Пробуйте `/healthz` в обычной liveness cadence и alert,
  если `/v1/puzzle/mint` возвращает HTTP 500 (указывает на Argon2 parameter mismatch
  или RNG failures). Ошибки token minting проявляются как HTTP 4xx/5xx на
  `/v1/token/mint`; повторяющиеся сбои следует считать paging condition.

## Compliance и audit logging

Relays публикуют структурированные `handshake` events, включающие throttle reasons
и cooldown durations. Убедитесь, что compliance pipeline из
`docs/source/soranet/relay_audit_pipeline.md` поглощает эти logs, чтобы изменения
puzzle policy оставались auditables. Когда puzzle gate включен, архивируйте
образцы minted tickets и Norito configuration snapshot вместе с rollout ticket
для будущих audits. Admission tokens, выпущенные перед maintenance windows,
следует отслеживать по `token_id_hex` и добавлять в revocation файл после
истечения или отзыва.

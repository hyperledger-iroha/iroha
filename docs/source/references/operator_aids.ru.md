---
lang: ru
direction: ltr
source: docs/source/references/operator_aids.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf412f2645cea9d5468f4541ff48d4ace67bc6f3d60a97e68561dde4949ff9be
source_last_modified: "2025-12-13T10:25:50.323533+00:00"
translation_last_reviewed: 2026-01-01
---

# Конечные точки Torii — помощь оператору (краткая справка)

На этой странице перечислены неконсенсусные операторские эндпоинты, которые помогают с наблюдаемостью и устранением неполадок. Ответы — JSON, если не указано иначе.

Консенсус (Sumeragi)
- Метрики: gauge `sumeragi_new_view_receipts_by_hv{height,view}` отражают эти счётчики.
- GET `/v1/sumeragi/status`
  - Снимок индекса лидера, Highest/Locked QCs (`highest_qc`/`locked_qc`, высоты, view, хэши subject), счётчики collector/VRF, отсрочки pacemaker, глубина очереди транзакций и здоровье хранилища RBC (`rbc_store.{sessions,bytes,pressure_level,persist_drops_total,evictions_total,recent_evictions[...]}`).
- GET `/v1/sumeragi/status/sse`
  - Поток SSE (≈1 с) того же payload, что и `/v1/sumeragi/status`, для живых дашбордов.
- GET `/v1/sumeragi/qc`
  - Снимок highest/locked QCs; включает `subject_block_hash` для highest QC, если известно.
- GET `/v1/sumeragi/pacemaker`
  - Таймеры/конфигурация pacemaker: `{ backoff_ms, rtt_floor_ms, jitter_ms, backoff_multiplier, rtt_floor_multiplier, max_backoff_ms, jitter_frac_permille }`.
- GET `/v1/sumeragi/leader`
  - Снимок индекса лидера. В режиме NPoS включает контекст PRF: `{ height, view, epoch_seed }`.
- GET `/v1/sumeragi/telemetry`
  - Aggregated consensus telemetry: `availability.collectors` contains observed collector indices, peer IDs, and ingested-vote counts; `rbc_backlog` contains missing-chunk totals; `rbc_pending` contains bounded pre-session queue totals, drops, and limits. This is not a deterministic collector plan or a per-session RBC contract.
- GET `/v1/sumeragi/params`
  - Снимок on-chain параметров Sumeragi `{ block_time_ms, commit_time_ms, min_finality_ms, pacing_factor_bps, max_clock_drift_ms, collectors_k, redundant_send_r, da_enabled, next_mode, mode_activation_height, chain_height }`.
  - When `da_enabled` is true, availability evidence is tracked but does not gate commit; local payload is required and can be satisfied via RBC `DELIVER` or block sync. Use the aggregated telemetry endpoint, Prometheus counters, status snapshots, and logs to diagnose payload transport.

Доказательства (аудит; вне консенсуса)
- GET `/v1/sumeragi/evidence/count` → `{ "count": <u64> }`
- GET `/v1/sumeragi/evidence` → `{ "total": <u64>, "items": [...] }`
  - Включает базовые поля (например, DoublePrepare/DoubleCommit, InvalidQc, InvalidProposal) для инспекции.
  - Примеры:
    - `curl -s http://127.0.0.1:8080/v1/sumeragi/evidence/count | jq .`
    - `curl -s http://127.0.0.1:8080/v1/sumeragi/evidence | jq .`
- POST `/v1/sumeragi/evidence` → `{ "status": "accepted", "kind": "<variant>" }`
  - CLI‑помощники:
    - `iroha --output-format text ops sumeragi evidence list`
    - `iroha --output-format text ops sumeragi evidence count`
    - `iroha ops sumeragi evidence submit --evidence-hex <hex>` (или `--evidence-hex-file <path>`)

Аутентификация оператора (WebAuthn/mTLS)
- POST `/v1/operator/auth/registration/options`
  - Возвращает параметры регистрации WebAuthn (`publicKey`) для первоначального enrolment учётных данных.
- POST `/v1/operator/auth/registration/verify`
  - Проверяет attestation‑payload WebAuthn и сохраняет операторские учётные данные.
- POST `/v1/operator/auth/login/options`
  - Возвращает параметры аутентификации WebAuthn (`publicKey`) для входа оператора.
- POST `/v1/operator/auth/login/verify`
  - Проверяет assertion‑payload WebAuthn и возвращает токен сессии оператора.
- Заголовки:
  - `x-iroha-operator-session`: токен сессии для операторских endpoints (выдан login verify).
  - `x-iroha-operator-token`: bootstrap‑токен (разрешён, когда `torii.operator_auth.token_fallback` это допускает).
  - `x-api-token`: требуется, когда `torii.require_api_token = true` или `torii.operator_auth.token_source = "api"`.
  - `x-forwarded-client-cert`: требуется, когда `torii.operator_auth.require_mtls = true` (задаётся ingress‑прокси).
- Процесс enrolment:
  1. Вызовите registration options с bootstrap‑токеном (разрешено только до регистрации первой учётной записи при `token_fallback = "bootstrap"`).
  2. Запустите `navigator.credentials.create` в UI оператора и отправьте attestation в registration verify.
  3. Вызовите login options и login verify, чтобы получить `x-iroha-operator-session`.
  4. Отправляйте `x-iroha-operator-session` на операторских endpoints.

Примечания
- Эти endpoints — локальные представления узла (в памяти, где указано) и не влияют на консенсус или хранение.
- Доступ может быть защищён API‑токенами, операторской аутентификацией (WebAuthn/mTLS) и лимитами скорости в зависимости от конфигурации Torii.

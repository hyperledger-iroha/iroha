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
- GET `/v1/sumeragi/new_view`
  - Снимок количества полученных NEW_VIEW по `(height, view)`.
  - Формат: `{ "ts_ms": <u64>, "items": [{ "height": <u64>, "view": <u64>, "count": <u64> }, ...] }`
  - Пример:
    - `curl -s http://127.0.0.1:8080/v1/sumeragi/new_view | jq .`
- GET `/v1/sumeragi/new_view/sse` (SSE)
  - Периодический поток (≈1 с) того же payload для дашбордов.
  - Пример:
    - `curl -Ns http://127.0.0.1:8080/v1/sumeragi/new_view/sse`
- Метрики: gauge `sumeragi_new_view_receipts_by_hv{height,view}` отражают эти счётчики.
- GET `/v1/sumeragi/status`
  - Снимок индекса лидера, Highest/Locked commit certificates (`highest_qc`/`locked_qc`, высоты, view, хэши subject), счётчики collector/VRF, отсрочки pacemaker, глубина очереди транзакций и здоровье хранилища RBC (`rbc_store.{sessions,bytes,pressure_level,evictions_total,recent_evictions[...]}`).
- GET `/v1/sumeragi/status/sse`
  - Поток SSE (≈1 с) того же payload, что и `/v1/sumeragi/status`, для живых дашбордов.
- GET `/v1/sumeragi/qc`
  - Снимок highest/locked commit certificates; включает `subject_block_hash` для highest commit certificate, если известно.
- GET `/v1/sumeragi/pacemaker`
  - Таймеры/конфигурация pacemaker: `{ backoff_ms, rtt_floor_ms, jitter_ms, backoff_multiplier, rtt_floor_multiplier, max_backoff_ms, jitter_frac_permille }`.
- GET `/v1/sumeragi/leader`
  - Снимок индекса лидера. В режиме NPoS включает контекст PRF: `{ height, view, epoch_seed }`.
- GET `/v1/sumeragi/collectors`
  - Детерминированный план коллекторов, полученный из зафиксированной топологии и on-chain параметров: экспортирует `mode`, план `(height, view)` (где `height` равен текущей высоте цепи), `collectors_k`, `redundant_send_r`, `proxy_tail_index`, `min_votes_for_commit`, упорядоченный список коллекторов и `epoch_seed` (hex), когда активен NPoS.
- GET `/v1/sumeragi/params`
  - Снимок on-chain параметров Sumeragi `{ block_time_ms, commit_time_ms, max_clock_drift_ms, collectors_k, redundant_send_r, da_enabled, next_mode, mode_activation_height, chain_height }`.
  - Когда `da_enabled` равно true, доказательство доступности (`availability evidence` или RBC `READY`) отслеживается, но commit не ждёт его; локальный `DELIVER` RBC также не является требованием. Операторы могут подтвердить здоровье транспорта payload через endpoints RBC ниже.
- GET `/v1/sumeragi/rbc`
  - Агрегированные счётчики Reliable Broadcast: `{ sessions_active, sessions_pruned_total, ready_broadcasts_total, deliver_broadcasts_total, payload_bytes_delivered_total }`.
- GET `/v1/sumeragi/rbc/sessions`
  - Снимок состояния по сессиям (хэш блока, height/view, счётчики чанков, флаг delivered, метка `invalid`, хэш payload, boolean recovered) для диагностики зависших доставок RBC и выделения восстановленных сессий после перезапуска.
  - CLI‑сокращение: `iroha sumeragi rbc sessions --summary` печатает `hash`, `height/view`, прогресс чанков, количество ready и флаги invalid/delivered.

Доказательства (аудит; вне консенсуса)
- GET `/v1/sumeragi/evidence/count` → `{ "count": <u64> }`
- GET `/v1/sumeragi/evidence` → `{ "total": <u64>, "items": [...] }`
  - Включает базовые поля (например, DoublePrepare/DoubleCommit, InvalidQc, InvalidProposal) для инспекции.
  - Примеры:
    - `curl -s http://127.0.0.1:8080/v1/sumeragi/evidence/count | jq .`
    - `curl -s http://127.0.0.1:8080/v1/sumeragi/evidence | jq .`
- POST `/v1/sumeragi/evidence` → `{ "status": "accepted", "kind": "<variant>" }`
  - CLI‑помощники:
    - `iroha sumeragi evidence list --summary`
    - `iroha sumeragi evidence count --summary`
    - `iroha sumeragi evidence submit --evidence-hex <hex>` (или `--evidence-hex-file <path>`)

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

Фрагменты CLI для мониторинга (bash)

- Опрашивать JSON‑снимок каждые 2 с (печатает последние 10 записей):

```bash
#!/usr/bin/env bash
set -euo pipefail
TORII="${TORII:-http://127.0.0.1:8080}"
INTERVAL="${INTERVAL:-2}"
TOKEN="${TOKEN:-}"
HDR=()
if [[ -n "$TOKEN" ]]; then HDR=(-H "x-api-token: $TOKEN"); fi
while true; do
  curl -s "${HDR[@]}" "$TORII/v1/sumeragi/new_view" \
    | jq -c '{ts_ms, items:(.items|sort_by([.height,.view])|reverse|.[:10])}'
  sleep "$INTERVAL"
done
```

- Следить за SSE‑потоком и форматировать (последние 10 записей):

```bash
#!/usr/bin/env bash
set -euo pipefail
TORII="${TORII:-http://127.0.0.1:8080}"
TOKEN="${TOKEN:-}"
HDR=()
if [[ -n "$TOKEN" ]]; then HDR=(-H "x-api-token: $TOKEN"); fi
curl -Ns "${HDR[@]}" "$TORII/v1/sumeragi/new_view/sse" \
  | awk '/^data:/{sub(/^data: /,""); print}' \
  | jq -c '{ts_ms, items:(.items|sort_by([.height,.view])|reverse|.[:10])}'
```

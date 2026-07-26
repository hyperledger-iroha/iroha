---
lang: ru
direction: ltr
source: docs/source/runbooks/torii_norito_rpc_canary.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44aa7b618a377a4a62f3be210ce6b9ccc6906b7791c4bbf6d3f60899fe08e7b8
source_last_modified: "2025-12-14T09:53:36.245431+00:00"
translation_last_reviewed: 2025-12-28
---

# Ранбук канареечного запуска Torii Norito-RPC (NRPC-2C)

Этот ранбук операционализирует план **NRPC-2**, описывая, как продвинуть
Norito‑RPC транспорт из staging‑лаборатории в production‑stage “canary”.
Рекомендуется читать вместе с:

- [`docs/source/torii/nrpc_spec.md`](../torii/nrpc_spec.md) (контракт протокола)
- [`docs/source/torii/norito_rpc_rollout_plan.md`](../torii/norito_rpc_rollout_plan.md)
- [`docs/source/torii/norito_rpc_telemetry.md`](../torii/norito_rpc_telemetry.md)

## Роли и входы

| Роль | Ответственность |
|------|----------------|
| Torii Platform TL | Утверждает конфиг‑дельты, подписывает smoke‑тесты. |
| NetOps | Применяет ingress/envoy изменения и мониторит здоровье canary‑пула. |
| Observability liaison | Проверяет дашборды/алерты и собирает доказательства. |
| Platform Ops | Ведёт change‑тикет, координирует rollback‑репетицию, обновляет трекеры. |

Необходимые артефакты:

- Последний `iroha_config` Norito‑патч с `transport.norito_rpc.stage = "canary"` и
  заполненным `transport.norito_rpc.allowed_clients`.
- Фрагмент конфигурации Envoy/Nginx, сохраняющий `Content-Type: application/x-norito`
  и enforcing mTLS‑профиль canary‑клиентов (`defaults/torii_ingress_mtls.yaml`).
- Allowlist токенов (YAML или Norito‑манифест) для canary‑клиентов.
- URL Grafana + API‑токен для `dashboards/grafana/torii_norito_rpc_observability.json`.
- Доступ к smoke‑harness
  (`python/iroha_python/scripts/run_norito_rpc_smoke.sh`) и скрипту drill‑алертов
  (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`).

## Pre‑flight чек‑лист

1. **Подтвердить freeze спецификации.** Убедитесь, что hash `docs/source/torii/nrpc_spec.md`
   совпадает с последним подписанным релизом, и нет PR, меняющих заголовок/лейаут Norito.
2. **Валидация конфига.** Выполните
   ```bash
   cargo xtask validate-config --config <patch.json> --schema client_api
   ```
   чтобы убедиться, что новые `transport.norito_rpc.*` корректно парсятся.
3. **Caps схемы.** Установите консервативные `torii.preauth_scheme_limits.norito_rpc`
   (например, 25 одновременных соединений), чтобы бинарные клиенты не вытеснили JSON‑трафик.
4. **Rehearsal ingress.** Примените Envoy‑патч в staging, выполните негативный тест
   (`cargo test -p iroha_torii -- norito_ingress`) и убедитесь, что удалённые заголовки
   отклоняются с HTTP 415.
5. **Проверка телеметрии.** В staging выполните `scripts/telemetry/test_torii_norito_rpc_alerts.sh
   --env staging --dry-run` и приложите полученный evidence‑bundle.
6. **Инвентаризация токенов.** Проверьте, что allowlist canary содержит минимум
   двух операторов на регион; сохраните манифест в `artifacts/norito_rpc/<YYYYMMDD>/allowlist.json`.
7. **Ticketing.** Откройте change‑тикет с окном старта/окончания, планом rollback и
   ссылками на этот ранбук и телеметрические доказательства.

## Процедура продвижения canary

1. **Применить конфиг‑патч.**
   - Развернуть delta `iroha_config` (stage=`canary`, allowlist заполнен,
     лимиты схемы заданы) через admission.
   - Перезапустить или hot‑reload Torii и убедиться, что патч принят по логам
     `torii.config.reload`.
2. **Обновить ingress.**
   - Развернуть конфигурацию Envoy/Nginx, включающую Norito‑headers routing/mTLS‑профиль
     для canary‑пула.
   - Убедиться, что ответы `curl -vk --cert <client.pem>` содержат заголовки Norito
     `X-Iroha-Error-Code` когда требуется.
3. **Smoke‑тесты.**
   - Запустить `python/iroha_python/scripts/run_norito_rpc_smoke.sh --profile canary`
     с canary‑bastion. Сохранить JSON + Norito‑транскрипты в
     `artifacts/norito_rpc/<YYYYMMDD>/smoke/`.
   - Записать hashes в `docs/source/torii/norito_rpc_stage_reports.md`.
4. **Наблюдать телеметрию.**
   - Отслеживать `torii_active_connections_total{scheme="norito_rpc"}` и
     `torii_request_duration_seconds_bucket{scheme="norito_rpc"}` минимум 30 минут.
   - Экспортировать Grafana‑дашборд через API и прикрепить к change‑тикету.
5. **Drill алертов.**
   - Запустить `scripts/telemetry/test_torii_norito_rpc_alerts.sh --env canary` для
     инъекции некорректных Norito‑конвертов; убедиться, что Alertmanager записал
     синтетический инцидент и сам его очистил.
6. **Сбор доказательств.**
   - Обновить `docs/source/torii/norito_rpc_stage_reports.md` со следующими полями:
     - Digest конфига
     - Hash манифеста allowlist
     - Timestamp smoke‑тестов
     - Checksum экспорта Grafana
     - ID alert‑drill
   - Загрузить артефакты в `artifacts/norito_rpc/<YYYYMMDD>/`.

## Мониторинг и критерии выхода

Оставайтесь в canary, пока все условия ниже выполняются ≥72 часов:

- Error‑rate (`torii_request_failures_total{scheme="norito_rpc"}`) ≤1 % и нет
  устойчивых всплесков `torii_norito_decode_failures_total`.
- Паритет латентности (`p95` Norito vs JSON) в пределах 10 %.
- Дашборд алертов тихий, кроме плановых drills.
- Операторы из allowlist присылают отчёты паритета без schema‑mismatch.

Документируйте ежедневный статус в change‑тикете и сохраняйте snapshots в
`docs/source/status/norito_rpc_canary_log.md` (если есть).

## Процедура rollback

1. Переключить `transport.norito_rpc.stage` обратно на `"disabled"` и очистить
   `allowed_clients`; применить через admission.
2. Удалить Envoy/Nginx route/mTLS stanza, перезагрузить прокси и убедиться, что
   новые Norito‑соединения отклоняются.
3. Отозвать canary‑токены (или деактивировать bearer‑учётные данные), чтобы
   активные сессии завершились.
4. Мониторить `torii_active_connections_total{scheme="norito_rpc"}` до нуля.
5. Перезапустить JSON‑only smoke‑harness, чтобы проверить базовую функциональность.
6. Создать stub post‑mortem в `docs/source/postmortems/norito_rpc_rollback.md`
   в течение 24 часов и обновить change‑тикет итогами влияния + метриками.

## Post‑canary закрытие

После выполнения критериев выхода:

1. Обновить `docs/source/torii/norito_rpc_stage_reports.md` рекомендацией GA.
2. Добавить запись в `status.md` с итогами canary и evidence‑bundles.
3. Уведомить SDK‑лидов, чтобы переключили staging‑fixtures на Norito для парити‑прогонов.
4. Подготовить GA‑патч конфига (stage=`ga`, удалить allowlist) и запланировать
   продвижение согласно NRPC-2.

Следование этому ранбуку гарантирует одинаковый сбор evidence, детерминированный
rollback и выполнение критериев NRPC-2.

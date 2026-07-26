---
lang: ru
direction: ltr
source: docs/portal/docs/devportal/norito-rpc-adoption.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# График внедрения Norito-RPC

> Канонические заметки по планированию находятся в `docs/source/torii/norito_rpc_adoption_schedule.md`.  
> Эта версия портала суммирует ожидания по раскатке для авторов SDK, операторов и рецензентов.

## Цели

- Согласовать все SDK (Rust CLI, Python, JavaScript, Swift, Android) на бинарный транспорт Norito-RPC до продакшн-переключения AND4.
- Держать фазовые гейты, пакеты доказательств и телеметрические хуки детерминированными, чтобы управление могло аудитировать раскатку.
- Сделать сбор доказательств fixtures и canary простым с помощью общих хелперов, указанных в дорожной карте NRPC-4.

## Таймлайн фаз

| Фаза | Окно | Область | Критерии выхода |
|------|------|---------|-----------------|
| **P0 - Лабораторный паритет** | Q2 2025 | Smoke-наборы Rust CLI + Python запускают `/v1/norito-rpc` в CI, JS helper проходит unit-тесты, Android mock harness проверяет оба транспорта. | `python/iroha_python/scripts/run_norito_rpc_smoke.sh` и `javascript/iroha_js/test/noritoRpcClient.test.js` зеленые в CI; Android harness подключен к `./gradlew test`. |
| **P1 - SDK preview** | Q3 2025 | Общий bundle fixtures закоммичен, `scripts/run_norito_rpc_fixtures.sh --sdk <label>` пишет логи + JSON в `artifacts/norito_rpc/`, опциональные флаги транспорта Norito показаны в примерах SDK. | Манифест fixtures подписан, обновления README показывают opt-in, preview API Swift доступен за флагом IOS2. |
| **P2 - Staging / AND4 preview** | Q1 2026 | Staging-пулы Torii предпочитают Norito, клиенты AND4 preview для Android и паритетные наборы IOS2 в Swift по умолчанию используют бинарный транспорт, дашборд телеметрии `dashboards/grafana/torii_norito_rpc_observability.json` заполнен. | `docs/source/torii/norito_rpc_stage_reports.md` фиксирует canary, `scripts/telemetry/test_torii_norito_rpc_alerts.sh` проходит, реплей Android mock harness фиксирует кейсы успеха/ошибки. |
| **P3 - Production GA** | Q4 2026 | Norito становится транспортом по умолчанию для всех SDK; JSON остается fallback для brownout. Релизные job-ы архивируют артефакты паритета с каждым тегом. | Release checklist включает Norito smoke вывод для Rust/JS/Python/Swift/Android; пороги алертов для SLOs по уровню ошибок Norito vs JSON применены; `status.md` и release notes цитируют доказательства GA. |

## Доставки SDK и CI хуки

- **Rust CLI и интеграционный harness** - расширить smoke-тесты `iroha_cli pipeline`, чтобы форсировать транспорт Norito после появления `cargo xtask norito-rpc-verify`. Защитить с `cargo test -p integration_tests -- norito_streaming` (lab) и `cargo xtask norito-rpc-verify` (staging/GA), сохраняя артефакты в `artifacts/norito_rpc/`.
- **Python SDK** - сделать релизный smoke (`python/iroha_python/scripts/release_smoke.sh`) по умолчанию Norito RPC, держать `run_norito_rpc_smoke.sh` как CI-вход, и документировать паритет в `python/iroha_python/README.md`. Цель CI: `PYTHON_BIN=python3 python/iroha_python/scripts/run_norito_rpc_smoke.sh`.
- **JavaScript SDK** - стабилизировать `NoritoRpcClient`, пусть helpers governance/query по умолчанию выбирают Norito при `toriiClientConfig.transport.preferred === "norito_rpc"`, и фиксировать end-to-end примеры в `javascript/iroha_js/recipes/`. CI должен запускать `npm test` плюс dockerized `npm run test:norito-rpc` перед публикацией; provenance выгружает Norito smoke логи в `javascript/iroha_js/artifacts/`.
- **Swift SDK** - подключить Norito bridge транспорт за флагом IOS2, синхронизировать cadence fixtures, и убедиться, что паритетный набор Connect/Norito запускается в Buildkite lanes, указанных в `docs/source/sdk/swift/index.md`.
- **Android SDK** - клиенты AND4 preview и Torii mock harness переходят на Norito, а телеметрия retry/backoff документируется в `docs/source/sdk/android/networking.md`. Harness делится fixtures с другими SDK через `scripts/run_norito_rpc_fixtures.sh --sdk android`.

## Доказательства и автоматизация

- `scripts/run_norito_rpc_fixtures.sh` оборачивает `cargo xtask norito-rpc-verify`, захватывает stdout/stderr и эмитит `fixtures.<sdk>.summary.json`, чтобы владельцы SDK имели детерминированный артефакт для `status.md`. Используйте `--sdk <label>` и `--out artifacts/norito_rpc/<stamp>/` для аккуратных CI бандлов.
- `cargo xtask norito-rpc-verify` обеспечивает паритет хеша схемы (`fixtures/norito_rpc/schema_hashes.json`) и падает, если Torii возвращает `X-Iroha-Error-Code: schema_mismatch`. Каждый сбой сопровождайте захватом JSON fallback для отладки.
- `scripts/telemetry/test_torii_norito_rpc_alerts.sh` и `dashboards/grafana/torii_norito_rpc_observability.json` описывают контракты алертов для NRPC-2. Запускайте скрипт после каждого изменения дашборда и храните вывод `promtool` в canary-бандле.
- `docs/source/runbooks/torii_norito_rpc_canary.md` описывает staging и production drills; обновляйте его при изменении hashes fixtures или alert gates.

## Чеклист для ревьюеров

Перед закрытием вехи NRPC-4 подтвердите:

1. Хеши последнего bundle fixtures совпадают с `fixtures/norito_rpc/schema_hashes.json`, а соответствующий CI артефакт записан в `artifacts/norito_rpc/<stamp>/`.
2. README SDK и portal docs объясняют, как форсировать JSON fallback и указывают Norito как транспорт по умолчанию.
3. Телеметрические дашборды показывают панели dual-stack error-rate с ссылками на алерты, а dry run Alertmanager (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) приложен к трекеру.
4. Этот график внедрения совпадает с записью трекера (`docs/source/torii/norito_rpc_tracker.md`), а дорожная карта (NRPC-4) ссылается на тот же bundle доказательств.

Дисциплина по графику делает поведение cross-SDK предсказуемым и позволяет управлению аудитировать внедрение Norito-RPC без индивидуальных запросов.

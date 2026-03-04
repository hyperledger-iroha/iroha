---
lang: ru
direction: ltr
source: docs/source/confidential_assets_audit_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8691a94d23e589f46d8e8cf2359d6d9a31f7c38c5b7bf0def69c88d2dd081765
source_last_modified: "2026-01-22T15:38:30.658489+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Пособие по аудиту и операциям конфиденциальных активов, на которое ссылается `roadmap.md:M4`.

# Руководство по аудиту и операциям конфиденциальных активов

В этом руководстве собраны доказательства, на которые полагаются аудиторы и операторы.
при проверке потоков конфиденциальных активов. Он дополняет сценарий ротации.
(`docs/source/confidential_assets_rotation.md`) и журнал калибровки.
(`docs/source/confidential_assets_calibration.md`).

## 1. Выборочное раскрытие информации и ленты событий

- Каждая конфиденциальная инструкция генерирует структурированную полезную нагрузку `ConfidentialEvent`.
  (`Shielded`, `Transferred`, `Unshielded`), захваченные в
  `crates/iroha_data_model/src/events/data/events.rs:198` и сериализованный
  исполнители (`crates/iroha_core/src/smartcontracts/isi/world.rs:3699`–`4021`).
  Пакет регрессии проверяет конкретные полезные данные, поэтому аудиторы могут положиться на них.
  детерминированные макеты JSON (`crates/iroha_core/tests/zk_confidential_events.rs:19`–`299`).
— Torii предоставляет эти события через стандартный конвейер SSE/WebSocket; аудиторы
  подписаться с помощью `ConfidentialEventFilter` (`crates/iroha_data_model/src/events/data/filters.rs:82`),
  при необходимости можно ограничиться одним определением актива. Пример интерфейса командной строки:

  ```bash
  iroha ledger events data watch --filter '{ "confidential": { "asset_definition_id": "rose#wonderland" } }'
  ```

- Метаданные политики и ожидающие переходы доступны через
  `GET /v1/confidential/assets/{definition_id}/transitions`
  (`crates/iroha_torii/src/routing.rs:15205`), отражено Swift SDK
  (`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:3245`) и описано в
  как дизайн конфиденциальных активов, так и руководства по SDK
  (`docs/source/confidential_assets.md:70`, `docs/source/sdk/swift/index.md:334`).

## 2. Телеметрия, информационные панели и данные калибровки

- Метрики времени выполнения отображают глубину дерева, историю обязательств/границ, удаление корня
  счетчики и коэффициенты попаданий в кэш верификаторов
  (`crates/iroha_telemetry/src/metrics.rs:5760` – `5815`). Grafana панели мониторинга в
  `dashboards/grafana/confidential_assets.json` отправляет соответствующие панели и
  оповещения, рабочий процесс описан в `docs/source/confidential_assets.md:401`.
- Калибровочные прогоны (NS/op, gas/op, ns/gas) с подписанными журналами в реальном времени.
  `docs/source/confidential_assets_calibration.md`. Новейший Apple Silicon
  Запуск NEON заархивирован по адресу
  `docs/source/confidential_assets_calibration_neon_20260428.log` и то же самое
  В реестре фиксируются временные отказы для профилей SIMD-нейтрального и AVX2 до тех пор, пока
  хосты x86 подключаются к сети.

## 3. Реагирование на инциденты и задачи оператора

- Процедуры ротации/обновления находятся в
  `docs/source/confidential_assets_rotation.md`, о том, как создавать новые
  пакеты параметров, планирование обновлений политики и уведомление кошельков/аудиторов.
  списки трекеров (`docs/source/project_tracker/confidential_assets_phase_c.md`)
  владельцы рунбуков и ожидания от репетиций.
- При производственных репетициях или аварийных окнах операторы прикрепляют доказательства к
  `status.md` записи (например, журнал многополосной репетиции) и включают:
  `curl` подтверждение перехода политики, снимки Grafana и соответствующее событие.
  дайджесты, чтобы аудиторы могли реконструировать монетный двор → передать → раскрыть сроки.

## 4. Периодичность внешнего рассмотрения

- Объем проверки безопасности: конфиденциальные каналы, реестры параметров, политика.
  переходы и телеметрия. Этот документ плюс формы журнала калибровки
  пакет доказательств, отправленный поставщикам; планирование проверки отслеживается через
  М4 в `docs/source/project_tracker/confidential_assets_phase_c.md`.
- Операторы должны держать `status.md` в курсе любых выводов поставщиков или последующих действий.
  предметы действия. До завершения внешней проверки этот модуль Runbook служит
  Аудиторы базового уровня эксплуатации могут проверить его.
---
lang: ba
direction: ltr
source: docs/source/confidential_assets_audit_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8691a94d23e589f46d8e8cf2359d6d9a31f7c38c5b7bf0def69c88d2dd081765
source_last_modified: "2026-01-22T14:35:37.510319+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Конфиденциаль активтар аудиты & операциялар playbook һылтанма `roadmap.md:M4`.

# Конфиденциаль активтар аудит & Операциялар runbook

Был ҡулланма дәлилдәр өҫтөн нығыта аудиторҙар һәм операторҙарға таяна .
конфиденциаль-актив ағымдарҙы раҫлауҙа. Ул әйләнешле плейбукты тулыландыра .
(`docs/source/confidential_assets_rotation.md`) һәм калибровка китабы
(`docs/source/confidential_assets_calibration.md`).

## 1. Һайлап алыуҙы асыҡлау & Ваҡиғалар каналдары

- Һәр конфиденциаль инструкция структуралы `ConfidentialEvent` файҙалы йөкләмәһен сығара
  (`Shielded`, `Transferred` X, `Unshielded`) 1990 йылда төшөрөлгән.
  `crates/iroha_data_model/src/events/data/events.rs:198` һәм сериализацияланған
  башҡарыусылар (`crates/iroha_core/src/smartcontracts/isi/world.rs:3699`–`4021`).
  Регрессия люкс күнекмәләр бетон файҙалы йөктәр, шулай итеп, аудиторҙар таяна ала
  детерминистик JSON макеттары (`crates/iroha_core/tests/zk_confidential_events.rs:19`–`299`).
- Torii был ваҡиғаларҙы стандарт SSE/WebSocket торбаһы аша фашлай; аудиторҙары
  `ConfidentialEventFilter` (`crates/iroha_data_model/src/events/data/filters.rs:82`) ярҙамында яҙылырға.
  теләк буйынса бер активтарҙы билдәләүгә опплау. CLI миҫалы:

  ```bash
  iroha ledger events data watch --filter '{ "confidential": { "asset_definition_id": "62Fk4FPcMuLvW5QjDGNF2a4jAmjM" } }'
  ```

- Сәйәсәт метамағлүмәттәре һәм көтөлгән күсеүҙәр аша мөмкин.
  `GET /v1/confidential/assets/{definition_id}/transitions`
  (`crates/iroha_torii/src/routing.rs:15205`), Көҙгө Swift SDK
  (`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:3245`) һәм 1990 йылда документлаштырылған.
  икеһе лә конфиденциаль-активтар дизайны һәм SDK етәкселәр
  (`docs/source/confidential_assets.md:70`, `docs/source/sdk/swift/index.md:334`).

## 2. Телеметрия, приборҙар таҡталары һәм калибровка дәлилдәре

- Йүгереп йөрөү метрикаһы ер өҫтө ағасы тәрәнлеге, йөкләмә/сикле тарихы, тамыр сығарыу
  счетчиктар, һәм тикшерелгән-кэш хит нисбәттәре
  (`crates/iroha_telemetry/src/metrics.rs:5760`–`5815`). Grafana 1990 йылда приборҙар таҡталары.
  `dashboards/grafana/confidential_assets.json` судно менән бәйле панелдәр һәм
  иҫкәртмәләр, эш ағымы документлаштырылған `docs/source/confidential_assets.md:401`.
- Калибровка эшләй (НС/оп, газ/оп, нс/газ) менән ҡул ҡуйылған журналдар йәшәй.
  `docs/source/confidential_assets_calibration.md`. Һуңғы Apple кремний
  NEON йүгерә 1990 йылда архивланған.
  `docs/source/confidential_assets_calibration_neon_20260428.log`, һәм шул уҡ
  1990 йылға тиклем SIMD-нейтраль һәм AVX2 профилдәре өсөн ваҡытлыса отказдарҙы теркәй.
  x86 хосттар онлайн килә.

## 3. Инцидент яуап & Оператор бурыстары

- Ротацион/яңыртыу процедуралары 2019 йылда йәшәй.
  `docs/source/confidential_assets_rotation.md`, яҡтыртыу, нисек яңы сәхнәләштереү
  параметр өйөмдәре, график сәйәсәт яңыртыу, һәм хәбәр итеү янсыҡтар/аудиторҙар. 1990 й.
  трекер (`docs/source/project_tracker/confidential_assets_phase_c.md`) исемлектәре
  runbook хужалары һәм репетиция өмөттәре.
- Етештереүҙең репетициялары йәки авария тәҙрәләре өсөн операторҙарға дәлилдәр беркетелә.
  `status.md` яҙмалары (мәҫәлән, күп һыҙатлы репетиция журналы) һәм үҙ эсенә ала:
  `curl` сәйәсәт күсеүҙәре тураһында дәлил, Grafana снимоктары, һәм тейешле сара
  distests шулай аудиторҙар реконструкция ала мәтрүшкә→трансфер→десеү ваҡыт һыҙығы.

## 4. Тышҡы тикшерелгән Каденция

- Хәүефһеҙлек тикшерелеүе күләме: конфиденциаль схемалар, параметр реестрҙары, сәйәсәт
  күсеүҙәр, һәм телеметрия. Был документ плюс калибровка формаһы формалары
  һатыусыларға ебәрелгән дәлилдәр пакеты; обзор планлаштырыу аша күҙәтелә
  М4 `docs/source/project_tracker/confidential_assets_phase_c.md`-та.
- Операторҙар һаҡларға тейеш `status.md` яңыртылған теләһә ниндәй һатыусылар табыштары йәки эҙмә-эҙлекле .
  экшн әйберҙәре. Тышҡы обзор тамамланғанға тиклем, был runbook хеҙмәт итә, тип
  оператив башланғыс аудиторҙар ҡаршы һынау мөмкин.
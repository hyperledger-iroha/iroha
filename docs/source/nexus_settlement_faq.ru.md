<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: ru
direction: ltr
source: docs/source/nexus_settlement_faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9ccbc52b2d34a410c5b724b6421fc91bd403cd40d0a03360315a2ae2e3e504ec
source_last_modified: "2025-11-21T14:07:47.531238+00:00"
translation_last_reviewed: 2026-01-01
---

# FAQ по settlement в Nexus

**Ссылка на дорожную карту:** NX-14 — документация Nexus и ранбуки операторов  
**Статус:** Черновик 2026-03-24 (отражает спецификации settlement router и CBDC playbook)  
**Аудитория:** операторы, авторы SDK и ревьюеры управления, готовящие запуск Nexus (Iroha 3).

Этот FAQ отвечает на вопросы, поднятые в ходе ревью NX-14 о маршрутизации settlement, конверсии
XOR, телеметрии и аудиторских доказательствах. См. `docs/source/settlement_router.md` для полной
спецификации и `docs/source/cbdc_lane_playbook.md` для knobs политики, специфичных для CBDC.

> **TL;DR:** Все settlement-потоки проходят через Settlement Router, который
> списывает XOR-буферы на публичных lanes и применяет комиссии по lane. Операторы
> должны держать в синхронизации routing config (`config/config.toml`), телеметрические
> дашборды и audit logs с опубликованными manifest'ами.

## Часто задаваемые вопросы

### Какие lanes обрабатывают settlement и как понять, где мой DS?

- Каждый dataspace объявляет `settlement_handle` в своем manifest. Маппинг дефолтных handle такой:
  - `xor_global` для стандартных публичных lanes.
  - `xor_lane_weighted` для публичных кастомных lanes с внешней ликвидностью.
  - `xor_hosted_custody` для приватных/CBDC lanes (эскроу XOR буфер).
  - `xor_dual_fund` для гибридных/конфиденциальных lanes, смешивающих shielded + public потоки.
- Смотрите `docs/source/nexus_lanes.md` для классов lanes и
  `docs/source/project_tracker/nexus_config_deltas/*.md` для последних апрувов каталога.
  `irohad --sora --config ... --trace-config` печатает эффективный каталог в runtime для аудитов.

### Как Settlement Router определяет курсы конверсии?

- Router применяет единый путь с детерминированным ценообразованием:
  - Для публичных lanes используется on-chain пул ликвидности XOR (public DEX). Оракулы цены
    возвращаются к утвержденному TWAP, когда ликвидность тонкая.
  - Приватные lanes предварительно финансируют XOR буферы. При списании settlement router логирует
    кортеж конверсии `{lane_id, source_token, xor_amount, haircut}` и применяет haircuts,
    утвержденные управлением (`haircut.rs`), если буферы расходятся.
- Конфигурация находится под `[settlement]` в `config/config.toml`. Избегайте кастомных правок без
  указания управления. См. `docs/source/settlement_router.md` для описания полей.

### Как применяются комиссии и ребейты?

- Комиссии выражаются по lane в manifest:
  - `base_fee_bps` — применяется к каждому списанию settlement.
  - `liquidity_haircut_bps` — компенсирует общих поставщиков ликвидности.
  - `rebate_policy` — опционально (например, CBDC промо-ребейты).
- Router эмитит события `SettlementApplied` (формат Norito) с разбиением комиссий, чтобы SDK и
  аудиторы могли сверять записи в ledger.

### Какая телеметрия подтверждает здоровье settlement?

- Метрики Prometheus (экспортируются через `iroha_telemetry` и settlement router):
  - `nexus_settlement_latency_seconds{lane_id}` — P99 должен быть ниже 900 ms для публичных lanes /
    1200 ms для приватных lanes.
  - `settlement_router_conversion_total{source_token}` — подтверждает объемы конверсии по токенам.
  - `settlement_router_haircut_total{lane_id}` — алерт, когда не ноль без связанной заметки управления.
  - `iroha_settlement_buffer_xor{lane_id,dataspace_id}` — живые списания XOR по lane (micro units).
    Алерт при <25 %/10 % от Bmin.
  - `iroha_settlement_pnl_xor{lane_id,dataspace_id}` — реализованная вариация haircut для сверки с
    treasury P&L.
  - `iroha_settlement_haircut_bp{lane_id,dataspace_id}` — эффективный epsilon в последнем блоке;
    аудиторы сверяют с policy router.
  - `iroha_settlement_swapline_utilisation{lane_id,dataspace_id,profile}` — использование credit line
    sponsor/MM; алерт выше 80 %.
- `nexus_lane_block_height{lane,dataspace}` — последняя высота блока для пары lane/dataspace; держите
  соседние peers в пределах нескольких slots.
- `nexus_lane_finality_lag_slots{lane,dataspace}` — слоты между глобальным head и последним блоком
  этой lane; алерт, когда >12 вне drills.
- `nexus_lane_settlement_backlog_xor{lane,dataspace}` — backlog, ожидающий settlement в XOR;
  гейтируйте CBDC/приватные нагрузки до превышения регуляторных порогов.

`settlement_router_conversion_total` несет labels `lane_id`, `dataspace_id` и `source_token`, чтобы
доказать, какой gas asset вел каждую конверсию. `settlement_router_haircut_total` аккумулирует XOR
units (не сырые micro amounts), позволяя Treasury сверять haircut ledger напрямую из Prometheus.
- `lane_settlement_commitments[*].swap_metadata.volatility_class` показывает, применял ли router
  bucket маржи `stable`, `elevated` или `dislocated`. Записи elevated/dislocated должны ссылаться
  на incident log или governance note.
- Dashboards: `dashboards/grafana/nexus_settlement.json` плюс обзор `nexus_lanes.json`. Привяжите
  алерты к `dashboards/alerts/nexus_audit_rules.yml`.
- При деградации settlement телеметрии логируйте инцидент согласно runbook в
  `docs/source/nexus_operations.md`.

### Как экспортировать телеметрию lane для регуляторов?

Запускайте helper ниже всякий раз, когда регулятор запрашивает таблицу lanes:

```
cargo xtask nexus-lane-audit \
  --status artifacts/status.json \
  --json-out artifacts/nexus_lane_audit.json \
  --parquet-out artifacts/nexus_lane_audit.parquet \
  --markdown-out artifacts/nexus_lane_audit.md \
  --captured-at 2026-02-12T09:00:00Z \
  --lane-compliance artifacts/lane_compliance_evidence.json
```

* `--status` принимает JSON blob, возвращаемый `iroha status --format json`.
* `--json-out` сохраняет канонический JSON array по каждой lane (aliases, dataspace, высота блока,
  finality lag, TEU capacity/utilization, счетчики scheduler trigger + utilization, RBC throughput,
  backlog, метаданные управления и т.д.).
* `--parquet-out` пишет тот же payload в Parquet (Arrow schema), готовый для регуляторов, которым
  нужна columnar evidence.
* `--markdown-out` генерирует человекочитаемое резюме с пометкой lagging lanes, non-zero backlog,
  missing compliance evidence и pending manifests; по умолчанию `artifacts/nexus_lane_audit.md`.
* `--lane-compliance` опционален; если задан, должен указывать на JSON manifest, описанный в
  compliance doc, чтобы экспортированные строки включали соответствующую lane policy, подписи
  reviewers, snapshot метрик и выдержки audit log.

Архивируйте оба результата под `artifacts/` вместе с routed-trace evidence (скриншоты
`nexus_lanes.json`, состояние Alertmanager и `nexus_lane_rules.yml`).

### Какие доказательства ожидают аудиторы?

1. **Снимок конфигурации** — захватите `config/config.toml` с секцией `[settlement]` и каталог lanes,
   на который ссылается текущий manifest.
2. **Логи router** — архивируйте `settlement_router.log` ежедневно; он включает hashed settlement IDs,
   списания XOR и места применения haircuts.
3. **Экспорт телеметрии** — недельный snapshot метрик, перечисленных выше.
4. **Отчет сверки** — опционально, но рекомендуется: экспортируйте записи `SettlementRecordV1`
   (см. `docs/source/cbdc_lane_playbook.md`) и сравните с treasury ledger.

### Нужна ли SDK особая обработка для settlement?

- SDK должны:
  - Предоставлять helpers для запроса settlement событий (`/v2/settlement/records`) и интерпретации
    логов `SettlementApplied`.
  - Выводить lane IDs + settlement handles в конфигурации клиента, чтобы операторы корректно
    маршрутизировали транзакции.
  - Зеркалить Norito payloads, определенные в `docs/source/settlement_router.md` (например,
    `SettlementInstructionV1`) с end-to-end тестами.
- В quickstart SDK Nexus (следующий раздел) приведены сниппеты по языкам для onboarding на публичную
  сеть.

### Как settlements взаимодействуют с управлением или аварийными тормозами?

- Управление может поставить на паузу отдельные settlement handles через обновления manifest.
  Router уважает флаг `paused` и отклоняет новые settlements с детерминированной ошибкой
  (`ERR_SETTLEMENT_PAUSED`).
- Аварийные "haircut clamps" ограничивают максимальные списания XOR на блок, чтобы не дренировать
  общие буферы.
- Операторы должны мониторить `governance.settlement_pause_total` и следовать шаблону инцидента в
  `docs/source/nexus_operations.md`.

### Где сообщать о багах или запрашивать изменения?

- Пробелы в функционале -> открыть issue с тегом `NX-14` и сослаться на roadmap.
- Срочные settlement инциденты -> пейджить Nexus primary (см.
  `docs/source/nexus_operations.md`) и приложить router logs.
- Правки документации -> отправлять PR в этот файл и портальные аналоги
  (`docs/portal/docs/nexus/overview.md`, `docs/portal/docs/nexus/operations.md`).

### Можно показать примеры settlement потоков?

Следующие сниппеты показывают, что ожидают аудиторы для самых распространенных типов lanes.
Сохраняйте router log, ledger hashes и соответствующий telemetry export для каждого сценария, чтобы
ревьюеры могли воспроизвести доказательство.

#### Приватная CBDC lane (`xor_hosted_custody`)

Ниже - сокращенный router log для приватной CBDC lane с hosted custody handle. Лог подтверждает
детерминированные списания XOR, состав комиссий и telemetry IDs:

```text
2026-03-24T11:42:07Z settlement_router lane=3 dataspace=ds::cbdc::jp
    handle=xor_hosted_custody settlement_id=0x9c2f...a413
    source_token=JPYCBDC amount=125000.00
    xor_debited=312.500000 xor_rate=400.000000 haircut_bps=25 base_fee_bps=15
    fee_breakdown={base=0.046875, haircut=0.078125}
    ledger_tx=0x7ab1...ff11 telemetry_trace=nexus-settle-20260324T1142Z-lane3
```

В Prometheus должны быть соответствующие метрики:

```text
nexus_settlement_latency_seconds{lane_id="3"} 0.842
settlement_router_conversion_total{lane_id="3",source_token="JPYCBDC"} += 1
settlement_router_haircut_total{lane_id="3"} += 0.078125
```

Архивируйте фрагмент лога, хэш транзакции ledger и экспорт метрик вместе, чтобы аудиторы могли
восстановить поток. Следующие примеры показывают, как фиксировать доказательства для публичных и
гибридных/конфиденциальных lanes.

#### Публичная lane (`xor_global`)

Публичные data spaces маршрутизируются через `xor_global`, поэтому router списывает общий DEX buffer
и записывает актуальный TWAP, который использовался для ценообразования. Прикладывайте TWAP hash
или governance note, когда oracle падает на cached value.

```text
2026-03-25T08:11:04Z settlement_router lane=0 dataspace=ds::public::creator
    handle=xor_global settlement_id=0x81cc...991c
    source_token=XOR amount=42.000000
    xor_debited=42.000000 xor_rate=1.000000 haircut_bps=0 base_fee_bps=10
    fee_breakdown={base=0.004200, liquidity=0.000000}
    dex_twap_id=twap-20260325T0810Z ledger_tx=0x319e...dd72 telemetry_trace=nexus-settle-20260325T0811Z-lane0
```

Метрики подтверждают тот же поток:

```text
nexus_settlement_latency_seconds{lane_id="0"} 0.224
settlement_router_conversion_total{lane_id="0",source_token="XOR"} += 1
settlement_router_haircut_total{lane_id="0"} += 0
```

Сохраните запись TWAP, router log, telemetry snapshot и ledger hash в одном evidence bundle. Когда
алерты срабатывают по latency lane 0 или freshness TWAP, привяжите инцидент к этому bundle.

#### Гибридная/конфиденциальная lane (`xor_dual_fund`)

Гибридные lanes смешивают shielded buffers с публичными XOR резервами. Каждый settlement должен
показывать, какой bucket источника XOR и как policy haircut разделила комиссии. Router log раскрывает
эти детали через dual-fund metadata block:

```text
2026-03-26T19:54:31Z settlement_router lane=9 dataspace=ds::hybrid::art
    handle=xor_dual_fund settlement_id=0x55d2...c0ab
    source_token=ARTCREDIT amount=9800.00
    xor_debited_public=12.450000 xor_debited_shielded=11.300000
    xor_rate_public=780.000000 xor_rate_shielded=820.000000
    haircut_bps=35 base_fee_bps=20 dual_fund_ratio=0.52
    fee_breakdown={base=0.239000, haircut=0.418750}
    ledger_tx=0xa924...1104 telemetry_trace=nexus-settle-20260326T1954Z-lane9
```

```text
nexus_settlement_latency_seconds{lane_id="9"} 0.973
settlement_router_conversion_total{lane_id="9",source_token="ARTCREDIT"} += 1
settlement_router_haircut_total{lane_id="9"} += 0.418750
```

Архивируйте router log вместе с dual-fund policy (extract из governance catalog), экспорт
`SettlementRecordV1` по lane и telemetry snippet, чтобы аудиторы подтвердили, что split
shielded/public соблюдает лимиты управления.

Обновляйте этот FAQ, когда меняется поведение settlement router или когда управление вводит новые
классы lanes/политики комиссий.

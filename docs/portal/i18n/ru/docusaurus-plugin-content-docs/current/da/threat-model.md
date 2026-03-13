---
lang: ru
direction: ltr
source: docs/portal/docs/da/threat-model.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Канонический источник
Эта страница отражает `docs/source/da/threat_model.md`. Держите обе версии
:::

# Модель угроз Data Availability Sora Nexus

_Последняя проверка: 2026-01-19 — Следующая проверка: 2026-04-19_

Частота обслуживания: Data Availability Working Group (<=90 дней). Каждая
редакция должна появиться в `status.md` со ссылками на активные тикеты
смягчений и артефакты симуляций.

## Цель и область

Программа Data Availability (DA) обеспечивает доступность Taikai broadcast,
Nexus lane blobs и governance artefacts при Byzantine, сетевых и операционных
сбоях. Эта модель угроз закрепляет инженерную работу DA-1 (архитектура и модель
угроз) и служит базовым ориентиром для дальнейших задач DA (DA-2 .. DA-10).

Компоненты в рамках:
- Расширение Torii DA ingest и writers Norito metadata.
- Деревья хранения blobs на SoraFS (hot/cold tiers) и политики репликации.
- Nexus block commitments (wire formats, proofs, light-client APIs).
- Hooks принудительного PDP/PoTR для DA payloads.
- Операторские процессы (pinning, eviction, slashing) и observability pipelines.
- Governance approvals для допуска/исключения DA операторов и контента.

Вне рамок документа:
- Полная экономическая модель (зафиксирована в workstream DA-7).
- Базовые протоколы SoraFS, уже покрытые SoraFS threat model.
- Client SDK ergonomics вне угрозной поверхности.

## Архитектурный обзор

1. **Submission:** Клиенты отправляют blobs через Torii DA ingest API. Узел
   разбивает blobs, кодирует Norito manifests (тип blob, lane, epoch, flags codec),
   и хранит chunks в hot tier SoraFS.
2. **Advertisement:** Pin intents и replication hints распространяются к
   storage providers через registry (SoraFS marketplace) с policy tags,
   определяющими цели hot/cold retention.
3. **Commitment:** Nexus sequencers включают blob commitments (CID + optional
   KZG roots) в канонический блок. Light clients опираются на commitment hash и
   объявленную metadata для проверки availability.
4. **Replication:** Storage nodes подтягивают назначенные shares/chunks, проходят
   PDP/PoTR challenges и перемещают данные между hot/cold tiers по политике.
5. **Fetch:** Потребители получают данные через SoraFS или DA-aware gateways,
   проверяют proofs и создают repair requests при исчезновении replicas.
6. **Governance:** Парламент и комитет DA утверждают операторов, rent schedules
   и escalations enforcement. Governance artefacts проходят тем же DA путем для
   прозрачности процесса.

## Активы и владельцы

Шкала влияния: **Critical** ломает безопасность/живучесть ledger; **High**
блокирует DA backfill или клиентов; **Moderate** снижает качество, но
восстановимо; **Low** ограниченное влияние.

| Asset | Description | Integrity | Availability | Confidentiality | Owner |
| --- | --- | --- | --- | --- | --- |
| DA blobs (chunks + manifests) | Taikai, lane, governance blobs в SoraFS | Critical | Critical | Moderate | DA WG / Storage Team |
| Norito DA manifests | Типизированная metadata о blobs | Critical | High | Moderate | Core Protocol WG |
| Block commitments | CIDs + KZG roots внутри Nexus blocks | Critical | High | Low | Core Protocol WG |
| PDP/PoTR schedules | Каденс enforcement для DA replicas | High | High | Low | Storage Team |
| Operator registry | Одобренные storage providers и политики | High | High | Low | Governance Council |
| Rent and incentive records | Записи ledger для DA rent и штрафов | High | Moderate | Low | Treasury WG |
| Observability dashboards | DA SLOs, глубина репликации, алерты | Moderate | High | Low | SRE / Observability |
| Repair intents | Запросы на ре-гидратацию пропавших chunks | Moderate | Moderate | Low | Storage Team |

## Противники и возможности

| Актор | Возможности | Мотивации | Примечания |
| --- | --- | --- | --- |
| Malicious client | Отправка malformed blobs, replay stale manifests, DoS ingest. | Срыв Taikai broadcast, инъекция невалидных данных. | Нет привилегированных ключей. |
| Byzantine storage node | Drop replicas, forge PDP/PoTR proofs, collude. | Срезать DA retention, избежать rent, удерживать данные. | Имеет валидные operator credentials. |
| Compromised sequencer | Оmit commitments, equivocate blocks, reorder metadata. | Скрыть DA submissions, создать несогласованность. | Ограничен консенсусом большинства. |
| Insider operator | Злоупотребление governance доступом, подмена retention политики, утечка credentials. | Экономическая выгода, саботаж. | Доступ к hot/cold инфраструктуре. |
| Network adversary | Partition узлов, задержка replication, MITM трафик. | Снижение availability, деградация SLOs. | Не ломает TLS, но может замедлять/дропать. |
| Observability attacker | Подмена dashboards/alerts, подавление инцидентов. | Скрыть DA outages. | Требует доступа к telemetry pipeline. |

## Границы доверия

- **Ingress boundary:** Клиент -> Torii DA extension. Нужна аутентификация на
  запрос, rate limiting и валидация payload.
- **Replication boundary:** Storage nodes обмениваются chunks и proofs. Узлы
  взаимно аутентифицированы, но могут вести себя Byzantine.
- **Ledger boundary:** Committed block data vs off-chain storage. Консенсус
  защищает целостность, но availability требует off-chain enforcement.
- **Governance boundary:** Решения Council/Parliament по операторам, бюджетам и
  slashing. Нарушения здесь напрямую влияют на DA deployment.
- **Observability boundary:** Сбор metrics/logs и экспорт в dashboards/alert
  tooling. Tampering скрывает outages или атаки.

## Сценарии угроз и контрмеры

### Атаки на ingest путь

**Сценарий:** Malicious client отправляет malformed Norito payloads или
oversized blobs для истощения ресурсов или подмены metadata.

**Контрмеры**
- Валидация Norito schema с жесткой переговорами версии; reject unknown flags.
- Rate limiting и authentication на Torii ingest endpoint.
- Ограничения chunk size и детерминированный encoding в SoraFS chunker.
- Admission pipeline сохраняет manifests только после совпадения checksum.
- Deterministic replay cache (`ReplayCache`) отслеживает окна `(lane, epoch, sequence)`,
  сохраняет high-water marks на диске и отвергает duplicates/stale replays; property
  и fuzz harnesses покрывают divergent fingerprints и out-of-order submissions.
  [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**Оставшиеся пробелы**
- Torii ingest должен встроить replay cache в admission и сохранять sequence
  cursors между рестартами.
- Norito DA schemas имеют выделенный fuzz harness (`fuzz/da_ingest_schema.rs`) для
  проверки encode/decode инвариантов; coverage dashboards должны сигнализировать
  при регрессии.

### Удержание репликации

**Сценарий:** Byzantine storage operators принимают pin assignments, но drop
chunks и проходят PDP/PoTR через forged responses или collusion.

**Контрмеры**
- PDP/PoTR challenge schedule расширен на DA payloads с покрытием по epoch.
- Multi-source replication с quorum thresholds; fetch orchestrator выявляет
  missing shards и инициирует repair.
- Governance slashing привязан к failed proofs и missing replicas.
- Automated reconciliation job (`cargo xtask da-commitment-reconcile`) сравнивает
  ingest receipts с DA commitments (SignedBlockWire/`.norito`/JSON), формирует
  JSON evidence bundle для governance и падает при missing/mismatched tickets,
  чтобы Alertmanager мог пейджить по omission/tampering.

**Оставшиеся пробелы**
- Simulation harness в `integration_tests/src/da/pdp_potr.rs` (tests:
  `integration_tests/tests/da/pdp_potr_simulation.rs`) теперь покрывает collusion
  и partition, проверяя детерминированное выявление Byzantine. Продолжать
  расширение вместе с DA-5.
- Политика cold-tier eviction требует signed audit trail, чтобы исключить covert drops.

### Подмена commitments

**Сценарий:** Compromised sequencer публикует блоки с пропуском/изменением DA
commitments, вызывая fetch failures и light-client inconsistencies.

**Контрмеры**
- Консенсус проверяет block proposals против DA submission queues; peers
  отвергают предложения без обязательных commitments.
- Light clients проверяют inclusion proofs перед выдачей fetch handles.
- Audit trail сравнивает submission receipts и block commitments.
- Automated reconciliation job (`cargo xtask da-commitment-reconcile`) сравнивает
  ingest receipts с commitments DA (SignedBlockWire/`.norito`/JSON), формирует
  JSON evidence bundle и падает при missing/mismatched tickets для Alertmanager.

**Оставшиеся пробелы**
- Закрыто reconciliation job + Alertmanager hook; governance пакеты теперь
  по умолчанию ingest-ят JSON evidence bundle.

### Network partition и цензура

**Сценарий:** Adversary разделяет replication network, мешая узлам получать
назначенные chunks или отвечать на PDP/PoTR challenges.

**Контрмеры**
- Multi-region provider требования обеспечивают разнообразные network paths.
- Challenge windows включают jitter и fallback на out-of-band repair.
- Observability dashboards мониторят replication depth, challenge success и
  fetch latency с alert thresholds.

**Оставшиеся пробелы**
- Partition simulations для Taikai live events пока отсутствуют; нужны soak tests.
- Политика резервирования repair bandwidth еще не формализована.

### Внутреннее злоупотребление

**Сценарий:** Operator с доступом к registry манипулирует retention политиками,
whitelist-ит malicious providers или подавляет alerts.

**Контрмеры**
- Governance actions требуют multi-party signatures и Norito-notarised records.
- Policy changes публикуют события в monitoring и archival logs.
- Observability pipeline enforce-ит append-only Norito logs с hash chaining.
- Quarterly access review automation (`cargo xtask da-privilege-audit`) проходит
  manifest/replay директории (плюс пути от операторов), отмечает missing/non-directory/
  world-writable entries, и выпускает signed JSON bundle для governance dashboards.

**Оставшиеся пробелы**
- Dashboard tamper-evidence требует signed snapshots.

## Реестр остаточных рисков

| Risk | Likelihood | Impact | Owner | Mitigation Plan |
| --- | --- | --- | --- | --- |
| Replay DA manifests до прихода DA-2 sequence cache | Possible | Moderate | Core Protocol WG | Реализовать sequence cache + nonce validation в DA-2; добавить regression tests. |
| PDP/PoTR collusion при компрометации >f узлов | Unlikely | High | Storage Team | Вывести новый challenge schedule с cross-provider sampling; валидировать через simulation harness. |
| Cold-tier eviction audit gap | Possible | High | SRE / Storage Team | Прикрепить signed audit logs и on-chain receipts для evictions; мониторить dashboards. |
| Latency обнаружения omission sequencer | Possible | High | Core Protocol WG | Ночной `cargo xtask da-commitment-reconcile` сравнивает receipts vs commitments (SignedBlockWire/`.norito`/JSON) и пейджит governance при missing/mismatched tickets. |
| Partition resilience для Taikai live streams | Possible | Critical | Networking TL | Провести partition drills; зарезервировать repair bandwidth; документировать SOP failover. |
| Governance privilege drift | Unlikely | High | Governance Council | Quarterly `cargo xtask da-privilege-audit` (manifest/replay dirs + extra paths) с signed JSON + dashboard gate; anchor audit artefacts on-chain. |

## Required Follow-Ups

1. Опубликовать Norito schemas для DA ingest и example vectors (внести в DA-2).
2. Протянуть replay cache через Torii DA ingest и сохранять sequence cursors
   при рестартах узлов.
3. **Completed (2026-02-05):** PDP/PoTR simulation harness теперь моделирует
   collusion + partition с QoS backlog modelling; см. `integration_tests/src/da/pdp_potr.rs`
   (tests: `integration_tests/tests/da/pdp_potr_simulation.rs`) с детерминированными сводками.
4. **Completed (2026-05-29):** `cargo xtask da-commitment-reconcile` сравнивает
   ingest receipts с DA commitments (SignedBlockWire/`.norito`/JSON), эмитирует
   `artifacts/da/commitment_reconciliation.json` и подключен к Alertmanager/ governance
   packets для omission/tampering alerts (`xtask/src/da.rs`).
5. **Completed (2026-05-29):** `cargo xtask da-privilege-audit` проходит manifest/replay
   spool (и paths от операторов), отмечает missing/non-directory/world-writable и
   генерирует signed JSON bundle для dashboard/ governance reviews
   (`artifacts/da/privilege_audit.json`), закрывая gap по access-review automation.

**Где смотреть дальше:**

- Replay cache и persistence cursors landed в DA-2. Реализация в
  `crates/iroha_core/src/da/replay_cache.rs` (cache logic) и интеграция Torii в
  `crates/iroha_torii/src/da/ingest.rs`, где fingerprint checks проходят через `/v2/da/ingest`.
- PDP/PoTR streaming simulations упражняются через proof-stream harness в
  `crates/sorafs_car/tests/sorafs_cli.rs`, покрывая PoR/PDP/PoTR request flows и
  failure scenarios из модели угроз.
- Capacity и repair soak результаты в
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`, а Sumeragi soak matrix в
  `docs/source/sumeragi_soak_matrix.md` (есть локализованные варианты). Эти
  artefacts фиксируют долгие drills из реестра рисков.
- Reconciliation + privilege-audit automation находится в
  `docs/automation/da/README.md` и новых командах
  `cargo xtask da-commitment-reconcile` / `cargo xtask da-privilege-audit`; используйте
  outputs по умолчанию в `artifacts/da/` при прикреплении evidence к governance packets.

## Simulation Evidence & QoS Modelling (2026-02)

Чтобы закрыть DA-1 follow-up #3, мы реализовали детерминированный PDP/PoTR
simulation harness в `integration_tests/src/da/pdp_potr.rs` (tests:
`integration_tests/tests/da/pdp_potr_simulation.rs`). Harness распределяет
узлы по 3 регионам, вводит partitions/collusion согласно вероятностям roadmap,
отслеживает PoTR lateness и питает repair-backlog модель, отражающую hot-tier
repair budget. Запуск default scenario (12 epochs, 18 PDP challenges + 2 PoTR
windows per epoch) дал следующие метрики:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| Metric | Value | Notes |
| --- | --- | --- |
| PDP failures detected | 48 / 49 (98.0%) | Partitions все еще детектируются; единственный недетектированный сбой связан с честным jitter. |
| PDP mean detection latency | 0.0 epochs | Сбои фиксируются в исходном epoch. |
| PoTR failures detected | 28 / 77 (36.4%) | Детект срабатывает при пропуске >=2 PoTR windows, оставляя большинство событий в residual-risk register. |
| PoTR mean detection latency | 2.0 epochs | Соответствует 2-epoch lateness threshold в archival escalation. |
| Repair queue peak | 38 manifests | Backlog растет, когда partitions накапливаются быстрее 4 repairs/epoch. |
| Response latency p95 | 30,068 ms | Отражает 30 s challenge window с +/-75 ms jitter для QoS sampling. |
<!-- END_DA_SIM_TABLE -->

Эти результаты теперь питают прототипы DA dashboards и удовлетворяют критериям
приемки "simulation harness + QoS modelling" из roadmap.

Автоматизация находится за
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
который вызывает общий harness и эмитит Norito JSON в
`artifacts/da/threat_model_report.json` по умолчанию. Ночные задачи используют
этот файл для обновления матриц в документе и алертов по drift в detection rates,
repair queues или QoS samples.

Для обновления таблицы выше используйте `make docs-da-threat-model`, что вызывает
`cargo xtask da-threat-model-report`, пересоздает
`docs/source/da/_generated/threat_model_report.json`, и переписывает секцию через
`scripts/docs/render_da_threat_model_tables.py`. Зеркало `docs/portal`
(`docs/portal/docs/da/threat-model.md`) обновляется в том же проходе для синхронизации.

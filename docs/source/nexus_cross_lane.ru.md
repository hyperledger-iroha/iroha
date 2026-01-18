---
lang: ru
direction: ltr
source: docs/source/nexus_cross_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6e6f144bf3aef313ba55b539c9e92c827bd626973fe38b557f0b668cc909f589
source_last_modified: "2025-12-13T05:07:11.929584+00:00"
translation_last_reviewed: 2026-01-01
---

# Кросс-лейновые обязательства Nexus и конвейер доказательств

> **Статус:** поставка NX-4 — конвейер кросс-лейновых обязательств и доказательств (цель Q4 2025).  
> **Владельцы:** Nexus Core WG / Cryptography WG / Networking TL.  
> **Связанные пункты дорожной карты:** NX-1 (lane geometry), NX-3 (settlement router), NX-4 (этот документ), NX-8 (global scheduler), NX-11 (SDK conformance).

Этот документ описывает, как данные исполнения по лейнам превращаются в проверяемое глобальное обязательство. Он связывает существующий settlement router (`crates/settlement_router`), lane block builder (`crates/iroha_core/src/block.rs`), поверхности телеметрии/статуса и планируемые хуки LaneRelay/DA, которые еще нужно реализовать для дорожной карты **NX-4**.

## Цели

- Выпускать детерминированный `LaneBlockCommitment` для каждого lane block, фиксируя settlement, ликвидность и данные вариации без утечки приватного состояния.
- Релеить эти обязательства (и их DA-аттестации) в глобальное кольцо NPoS, чтобы merge ledger мог упорядочивать, валидировать и сохранять кросс-лейновые обновления.
- Экспортировать те же payloads через Torii и телеметрию, чтобы операторы, SDK и аудиторы могли воспроизводить конвейер без специального инструментария.
- Определить инварианты и наборы доказательств, нужные для завершения NX-4: lane доказательства, DA-аттестации, интеграция merge ledger и регрессионное покрытие.

## Компоненты и поверхности

| Компонент | Ответственность | Ссылки на реализацию |
|-----------|----------------|----------------------|
| Lane executor и settlement router | Котировать XOR-конверсии, накапливать receipts по транзакциям, применять политику буфера | `crates/iroha_core/src/settlement/mod.rs`, `crates/settlement_router` |
| Lane block builder | Сливать `SettlementAccumulator`, выпускать `LaneBlockCommitment` рядом с lane block | `crates/iroha_core/src/block.rs:3340-3415` |
| LaneRelay broadcaster | Пакетировать lane QCs + DA proofs, распространять через `iroha_p2p` и кормить merge ring | `crates/iroha_core/src/nexus/lane_relay.rs`, `crates/iroha_core/src/sumeragi/main_loop.rs` |
| Global merge ledger | Проверять lane QCs, редуцировать merge hints, сохранять коммитменты world-state | `docs/source/merge_ledger.md`, `crates/iroha_core/src/sumeragi/status.rs`, `crates/iroha_core/src/state.rs` |
| Torii status и dashboards | Публиковать `lane_commitments`, `lane_settlement_commitments`, `lane_relay_envelopes`, scheduler gauges и Grafana boards | `crates/iroha_torii/src/routing.rs:16660-16880`, `dashboards/grafana/nexus_lanes.json` |
| Хранилище доказательств | Архивировать `LaneBlockCommitment`, артефакты RBC и снимки Alertmanager для аудита | `docs/settlement-router.md`, `artifacts/nexus/*` (будущий bundle) |

## Структуры данных и layout payload

Канонические payloads находятся в `crates/iroha_data_model/src/block/consensus.rs`.

### `LaneSettlementReceipt`

- `source_id` — хэш транзакции или id, заданный вызывающей стороной.
- `local_amount_micro` — дебет gas-токена dataspace.
- `xor_due_micro` / `xor_after_haircut_micro` / `xor_variance_micro` — детерминированные записи XOR-учета и запас безопасности на receipt (`due - after haircut`).
- `timestamp_ms` — UTC таймстамп в миллисекундах, взятый при settlement.

Receipts наследуют детерминированные правила котирования из `SettlementEngine` и агрегируются внутри каждого `LaneBlockCommitment`.

### `LaneSwapMetadata`

Опциональные метаданные, фиксирующие параметры котирования:

- `epsilon_bps`, `twap_window_seconds`, `volatility_class`.
- bucket `liquidity_profile` (Tier1–Tier3).
- строка `twap_local_per_xor`, чтобы аудиторы могли точно пересчитать конверсии.

### `LaneBlockCommitment`

Сводка по лейну, сохраняемая с каждым блоком:

- Заголовок: `block_height`, `lane_id`, `dataspace_id`, `tx_count`.
- Итоги: `total_local_micro`, `total_xor_due_micro`, `total_xor_after_haircut_micro`, `total_xor_variance_micro`.
- Опциональный `swap_metadata`.
- Упорядоченный вектор `receipts`.

Эти структуры уже выводят `NoritoSerialize`/`NoritoDeserialize`, поэтому их можно стримить on-chain, через Torii или через fixtures без дрейфа схемы.

### `LaneRelayEnvelope`

`LaneRelayEnvelope` (см. `crates/iroha_data_model/src/nexus/relay.rs`) упаковывает lane
`BlockHeader`, опциональный `commit QC (`Qc`)`, опциональный хэш `DaCommitmentBundle`, полный
`LaneBlockCommitment` и число байтов RBC на лейн. Envelope хранит Norito-derived
`settlement_hash` (через `compute_settlement_hash`), чтобы получатели могли проверить settlement
payload перед пересылкой в merge ledger. Отправители должны отвергать envelope, когда `verify`
проваливается (несовпадение QC subject, DA hash или settlement hash), когда `verify_with_quorum`
проваливается (ошибки длины bitmap подписантов/кворума), или когда агрегированная QC подпись не
проверяется против roster комитета по dataspace. QC preimage покрывает hash lane block плюс
`parent_state_root` и `post_state_root`, так что членство и корректность state-root
проверяются вместе.

### Выбор комитета лейна

Lane relay QCs проверяются комитетом на уровне dataspace. Размер комитета — `3f+1`, где `f`
настраивается в каталоге dataspace (`fault_tolerance`). Пул валидаторов — это валидаторы
dataspace: governance manifests для admin-managed lanes и записи public-lane staking для
stake-elected lanes. Членство в комитете детерминированно выбирается по эпохам с использованием
VRF-сида эпохи, связанного с `dataspace_id` и `lane_id` (стабильно в пределах эпохи). Если пул
меньше `3f+1`, финальность lane relay приостанавливается до восстановления кворума. Операторы могут
расширить пул, используя admin multisig инструкцию `SetLaneRelayEmergencyValidators`
(нужны `CanManagePeers` и `nexus.lane_relay_emergency.enabled = true`, по умолчанию выключено).
Когда включено, полномочие должно быть multisig-аккаунтом, отвечающим минимальным требованиям
(`nexus.lane_relay_emergency.multisig_threshold`/`multisig_members`, по умолчанию 3-of-5). Overrides
хранятся по dataspace, применяются только когда пул ниже кворума, и очищаются отправкой пустого
списка валидаторов. Если задан `expires_at_height`, проверка игнорирует override, когда
`block_height` lane relay envelope превышает высоту истечения. Счетчик телеметрии
`lane_relay_emergency_override_total{lane,dataspace,outcome}` фиксирует, был ли override применен
(`applied`) или был missing/expired/insufficient/disabled во время валидации.

## Жизненный цикл коммитмента

1. **Котирование и подготовка receipts.**  
   Settlement фасад (`SettlementEngine`, `SettlementAccumulator`) записывает `PendingSettlement`
   для каждой транзакции. Каждая запись хранит TWAP входы, профиль ликвидности, таймстампы и суммы
   XOR, чтобы позже стать `LaneSettlementReceipt`.

2. **Запечатывание receipts в блок.**  
   Во время `BlockBuilder::finalize` каждая пара `(lane_id, dataspace_id)` сливает свой accumulator.
   Builder создает `LaneBlockCommitment`, копирует список receipts, суммирует итоги и сохраняет
   опциональные swap метаданные (через `SwapEvidence`). Полученный вектор помещается в слот статуса
   Sumeragi (`crates/iroha_core/src/sumeragi/status.rs`), чтобы Torii и телеметрия могли сразу его
   показать.

3. **Relay упаковка и DA-аттестации.**  
   `LaneRelayBroadcaster` потребляет `LaneRelayEnvelope`, выпущенные при запечатывании блока, и
   рассылает их как высокоприоритетные фреймы `NetworkMessage::LaneRelay`. Envelope проверяются,
   дедуплицируются по `(lane_id,dataspace_id,height,settlement_hash)` и сохраняются в снимке
   статуса Sumeragi (`/v1/sumeragi/status`) для операторов и аудиторов. Broadcaster будет дальше
   развиваться, чтобы прикреплять DA артефакты (RBC chunk proofs, Norito headers,
   SoraFS/Object manifests) и кормить merge ring без head-of-line блокировок.

4. **Глобальное упорядочивание и merge ledger.**  
   Кольцо NPoS валидирует каждый relay envelope: проверяет `lane_qc` против комитета по dataspace,
   пересчитывает итоги settlement, проверяет DA proofs, затем подает tip лейна в merge ledger,
   описанный в `docs/source/merge_ledger.md`. Когда merge entry запечатывается, world-state hash
   (`global_state_root`) начинает коммитить каждый `LaneBlockCommitment`.

5. **Постоянное хранение и публикация.**  
   Kura записывает lane block, merge entry и `LaneBlockCommitment` атомарно, чтобы replay мог
   восстановить ту же редукцию. `/v1/sumeragi/status` публикует:
   - `lane_commitments` (метаданные исполнения).
   - `lane_settlement_commitments` (payload, описанный здесь).
   - `lane_relay_envelopes` (relay headers, QCs, DA digests, settlement hash и счетчики байтов RBC).
  Dashboards (`dashboards/grafana/nexus_lanes.json`) читают те же поверхности телеметрии и статуса,
  показывая throughput лейнов, предупреждения о доступности DA, объем RBC, дельты settlement и
  доказательства relay.

## Правила проверки и доказательств

Merge ring ДОЛЖЕН выполнить следующее до принятия lane commitment:

1. **Валидность lane QC.** Проверить агрегированную BLS подпись над preimage commit‑голоса
   (hash блока, `parent_state_root`, `post_state_root`, высота/вид/эпоха, `chain_id` и mode tag)
   против roster комитета по dataspace; убедиться, что длина bitmap подписантов соответствует
   комитету, подписанты соответствуют валидным индексам, и высота заголовка равна
   `LaneBlockCommitment.block_height`.
2. **Целостность receipts.** Пересчитать агрегаты `total_*` из вектора receipts; отвергнуть
   commitment, если суммы расходятся или receipts содержат дубликаты `source_id`.
3. **Согласованность swap metadata.** Убедиться, что `swap_metadata` (если есть) совпадает с
   текущей settlement конфигурацией и buffer политикой лейна.
4. **DA аттестация.** Проверить, что relay-присланные RBC/SoraFS proofs хэшируются в встроенный
   digest и что набор chunks покрывает весь payload блока (`rbc_bytes_total` в телеметрии должен
   это отражать).
5. **Merge редукция.** После прохождения lane proofs включить lane tip в запись merge ledger и
   пересчитать Poseidon2 reduction (`reduce_merge_hint_roots`). Любое расхождение отменяет merge entry.
6. **Телеметрия и аудит-трейл.** Увеличить аудит-счетчики по лейну
   (`nexus_audit_outcome_total{lane_id,...}`) и сохранить envelope, чтобы evidence bundle содержал
   и доказательство, и наблюдаемость.

## Доступность данных и наблюдаемость

- **Метрики:**  
  `nexus_scheduler_lane_teu_*`, `nexus_scheduler_dataspace_*`, `sumeragi_rbc_da_reschedule_total`,
  `da_reschedule_total`, `sumeragi_da_gate_block_total{reason="missing_local_data"}`,
  `lane_relay_invalid_total{error}`, `lane_relay_emergency_override_total{outcome}` и
  `nexus_audit_outcome_total` уже есть в `crates/iroha_telemetry/src/metrics.rs`. Операторы должны
  а `lane_relay_invalid_total` должен оставаться нулевым вне атакующих учений.
- **Torii поверхности:**  
  `/v1/sumeragi/status` включает `lane_commitments`, `lane_settlement_commitments` и снимки dataspace.
  `/v1/nexus/lane-config` (планируется) опубликует геометрию `LaneConfig`, чтобы клиенты могли
  сопоставлять `lane_id` и dataspace labels.
- **Dashboards:**  
  `dashboards/grafana/nexus_lanes.json` отображает backlog лейна, сигналы доступности DA и итоги
  settlement, указанные выше. Alert-определения должны пейджить, когда:
  - `nexus_scheduler_dataspace_age_slots` нарушает политику.
  - `sumeragi_da_gate_block_total{reason="missing_local_data"}` стабильно растет.
  - `total_xor_variance_micro` отклоняется от исторической нормы.
- **Evidence bundles:**  
  Каждый релиз должен прикладывать экспорт `LaneBlockCommitment`, снимки Grafana/Alertmanager и
  relay DA manifests под `artifacts/nexus/cross-lane/<date>/`. Bundle становится каноническим
  набором доказательств при подаче NX-4 readiness отчетов.

## Чеклист реализации (NX-4)

1. **LaneRelay сервис**
   - Схема определена в `LaneRelayEnvelope`; broadcaster реализован в
     `crates/iroha_core/src/nexus/lane_relay.rs` и подключен к запечатыванию блока
     (`crates/iroha_core/src/sumeragi/main_loop.rs`), отправляя `NetworkMessage::LaneRelay` с
     дедупликацией по узлам и сохранением статуса.
   - Сохранять relay артефакты для аудита (`artifacts/nexus/relay/...`).
2. **DA attestation hooks**
   - Интегрировать RBC / SoraFS chunk proofs с relay envelopes и сохранять сводные метрики в
     `SumeragiStatus`.
   - Экспортировать DA статус через Torii и Grafana для операторов.
3. **Валидация merge ledger**
   - Расширить валидатор merge entry, требуя relay envelopes вместо сырых lane headers.
   - Добавить replay тесты (`integration_tests/tests/nexus/*.rs`), которые подают синтетические
     commitments в merge ledger и проверяют детерминированную редукцию.
4. **Обновления SDK и tooling**
   - Документировать Norito layout `LaneBlockCommitment` для потребителей SDK
     (`docs/portal/docs/nexus/lane-model.md` уже ссылается сюда; дополнить API snippets).
   - Детерминированные fixtures лежат в `fixtures/nexus/lane_commitments/*.{json,to}`; запускать
     `cargo xtask nexus-fixtures` для регенерации (или `--verify` для проверки) примеров
     `default_public_lane_commitment` и `cbdc_private_lane_commitment` при изменениях схемы.
5. **Наблюдаемость и runbooks**
   - Подключить Alertmanager pack для новых метрик и описать evidence workflow в
     `docs/source/runbooks/nexus_cross_lane_incident.md` (follow-up).

Выполнение чеклиста выше вместе с этой спецификацией закрывает документационную часть **NX-4** и
разблокирует оставшуюся реализацию.

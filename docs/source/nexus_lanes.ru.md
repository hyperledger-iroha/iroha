---
lang: ru
direction: ltr
source: docs/source/nexus_lanes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94891050512eaf78f4c0381c0facbeed445a7e7323297070ae537e4d38ca7fe4
source_last_modified: "2025-12-13T05:07:11.953030+00:00"
translation_last_reviewed: 2026-01-01
---

# Модель lanes Nexus и партиционирование WSV

> **Статус:** результат NX-1 — таксономия lanes, геометрия конфигурации и layout хранения готовы к реализации.  
> **Владельцы:** Nexus Core WG, Governance WG  
> **Связанный пункт roadmap:** NX-1

Этот документ фиксирует целевую архитектуру многополосного уровня консенсуса Nexus. Цель — получить единое детерминированное мировое состояние, позволяя отдельным data spaces (lanes) запускать публичные или приватные наборы валидаторов с изолированными нагрузками.

> **Cross-lane proofs:** Этот документ фокусируется на геометрии и storage. Lane-level settlement commitments, relay pipeline и merge-ledger proofs для roadmap **NX-4** описаны в [nexus_cross_lane.md](nexus_cross_lane.md).

## Концепции

- **Lane:** логический shard реестра Nexus с собственным набором валидаторов и backlog исполнения. Идентифицируется стабильным `LaneId`.
- **Data Space:** governance bucket, объединяющий одну или несколько lanes с общими политиками compliance, маршрутизации и settlement. Каждый dataspace также объявляет `fault_tolerance (f)`, используемый для размера комитетов relay по lane (`3f+1`).
- **Lane Manifest:** метаданные под управлением governance, описывающие валидаторов, политику DA, gas token, правила settlement и permissions маршрутизации.
- **Global Commitment:** доказательство, выпускаемое lane и суммирующее новые state roots, данные settlement и опциональные cross-lane transfers. Глобальное кольцо NPoS упорядочивает commitments.

## Таксономия lanes

Типы lanes канонически описывают их видимость, поверхность governance и settlement hooks. Геометрия конфигурации (`LaneConfig`) фиксирует эти атрибуты, чтобы узлы, SDKs и tooling могли рассуждать о layout без bespoke логики.

| Тип lane | Видимость | Членство валидаторов | Экспозиция WSV | Governance по умолчанию | Политика settlement | Типичное применение |
|----------|-----------|----------------------|----------------|-------------------------|---------------------|---------------------|
| `default_public` | public | Permissionless (global stake) | Полная реплика состояния | SORA Parliament | `xor_global` | Базовый публичный реестр |
| `public_custom` | public | Permissionless или stake-gated | Полная реплика состояния | Stake weighted module | `xor_lane_weighted` | Публичные приложения с высоким throughput |
| `private_permissioned` | restricted | Фиксированный набор валидаторов (утвержден governance) | Commitments и proofs | Federated council | `xor_hosted_custody` | CBDC, консорциумные workloads |
| `hybrid_confidential` | restricted | Смешанное членство; оборачивает ZK proofs | Commitments + selective disclosure | Programmable money module | `xor_dual_fund` | Программируемые деньги с приватностью |

Все типы lanes должны объявлять:

- Dataspace alias — человекочитаемую группу, связывающую compliance политики.
- Governance handle — идентификатор, разрешаемый через `Nexus.governance.modules`.
- Settlement handle — идентификатор, используемый settlement router для списания XOR buffers.
- Опциональные метаданные телеметрии (описание, контакт, бизнес-домен), выводимые через `/status` и dashboards.

## Геометрия конфигурации lanes (`LaneConfig`)

`LaneConfig` — это runtime геометрия, выведенная из валидированного каталога lanes. Она не заменяет governance manifests; вместо этого предоставляет детерминированные storage идентификаторы и telemetria hints для каждой настроенной lane.

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```

- `LaneConfig::from_catalog` пересчитывает геометрию при каждом загрузочном цикле конфигурации (`State::set_nexus`).
- Aliases санитизируются в нижний регистр; последовательные неалфавитно-цифровые символы схлопываются в `_`. Если alias дает пустой slug, используем `lane{id}`.
- Key prefixes гарантируют, что WSV держит диапазоны ключей по lanes раздельно даже при общем backend.
- `shard_id` выводится из metadata ключа каталога `da_shard_id` (по умолчанию `lane_id`) и управляет persisted shard cursor journal, чтобы сохранить детерминированный DA replay при restart/resharding.
- Названия Kura сегментов детерминированы между hosts; аудиторы могут сверять директории и manifests без bespoke tooling.
- Merge сегменты (`lane_{id:03}_merge`) содержат последние merge-hint roots и global state commitments для этой lane.
- Когда governance переименовывает alias lane, узлы автоматически переименовывают директории `blocks/lane_{id:03}_{slug}` (и tiered snapshots), чтобы аудиторы всегда видели канонический slug без ручной чистки.

## Партиционирование world-state

- Логическое мировое состояние Nexus — это объединение state пространств по lanes. Public lanes хранят полное состояние; private/confidential lanes экспортируют Merkle/commitment roots в merge ledger.
- MV storage префиксирует каждый ключ 4-байтовым префиксом lane из `LaneConfigEntry::key_prefix`, формируя ключи вида `[00 00 00 01] ++ PackedKey`.
- Shared tables (accounts, assets, triggers, governance records) поэтому хранят записи, сгруппированные по префиксу lane, сохраняя детерминированные range scans.
- Метаданные merge-ledger отражают тот же layout: каждая lane пишет merge-hint roots и reduced global state roots в `lane_{id:03}_merge`, позволяя направленную retention/eviction при retirement lane.
- Cross-lane индексы (account aliases, asset registries, governance manifests) хранят явные пары `(LaneId, DataSpaceId)`. Эти индексы живут в shared column families, но используют префикс lane и явные dataspace ids, чтобы сохранять детерминированные lookups.
- Merge workflow объединяет public данные с private commitments через кортежи `(lane_id, dataspace_id, height, state_root, settlement_root, proof_root)` из merge-ledger записей.

## Партиционирование Kura и WSV

- **Kura сегменты**
  - `lane_{id:03}_{slug}` — основной block сегмент lane (blocks, indexes, receipts).
  - `lane_{id:03}_merge` — merge-ledger сегмент, записывающий reduced state roots и settlement artefacts.
  - Глобальные сегменты (consensus evidence, telemetria caches) остаются shared, потому что они lane-neutral; их ключи не содержат lane prefixes.
- Runtime следит за обновлениями каталога lanes: новые lanes получают свои block и merge-ledger директории автоматически под `kura/blocks/` и `kura/merge_ledger/`, а retired lanes архивируются под `kura/retired/{blocks,merge_ledger}/lane_{id:03}_*`.
- Tiered-state snapshots следуют тому же lifecycle; каждая lane пишет в `cold_store_root/lanes/lane_{id:03}_{slug}` и retirement перемещает дерево в `cold_store_root/retired/lanes/`.
- **Key prefixes** — 4-байтовый префикс из `LaneId` всегда добавляется к MV-encoded ключам. Host-specific hashing не используется, порядок одинаков на всех узлах.
- **Block log layout** — block data, index и hashes лежат под `kura/blocks/lane_{id:03}_{slug}/`. Merge-ledger journals используют тот же slug (`kura/merge/lane_{id:03}_{slug}.log`), изолируя recovery flows по lanes.
- **Retention policy** — public lanes хранят полные block bodies; commitment-only lanes могут компактировать старые bodies после checkpoints, поскольку commitments являются авторитетными. Confidential lanes держат ciphertext journals в выделенных сегментах, чтобы не блокировать другие workloads.
- **Tooling** — `cargo xtask nexus-lane-maintenance --config <path> [--compact-retired]` инспектирует `<store>/blocks` и `<store>/merge_ledger` на основе derived `LaneConfig`, сообщает активные vs retired сегменты и архивирует retired директории/логи в `<store>/retired/...` для детерминированных доказательств. Утилиты обслуживания (`kagami`, CLI admin commands) должны использовать slugged namespace при экспонировании метрик, Prometheus labels или при архивировании Kura segments.

## Routing и APIs

- REST/gRPC endpoints Torii принимают опциональный `lane_id`; отсутствие означает `lane_default`.
- SDKs показывают lane selectors и сопоставляют user-friendly aliases с `LaneId` через каталог lanes.
- Routing rules работают на validated catalog и могут выбирать lane и dataspace. `LaneConfig` предоставляет telemetria-friendly aliases для dashboards и logs.

## Settlement и fees

- Каждая lane платит XOR fees глобальному набору валидаторов. Lanes могут взимать нативные gas tokens, но должны escrow XOR equivalents вместе с commitments.
- Settlement proofs включают amount, conversion metadata и proof escrow (например, перевод в global fee vault).
- Unified settlement router (NX-3) списывает buffers с использованием lane prefixes, чтобы settlement telemetria совпадала с storage geometry.

## Governance

- Lanes объявляют свой governance module через каталог. `LaneConfigEntry` несет исходные alias и slug, чтобы telemetria и audit trails были читаемыми.
- Nexus registry распространяет подписанные lane manifests, которые включают `LaneId`, dataspace binding, governance handle, settlement handle и metadata.
- Runtime-upgrade hooks продолжают применять governance policies (`gov_upgrade_id` по умолчанию) и логируют diffs через telemetria bridge (events `nexus.config.diff`).
- Lane manifests определяют dataspace validator pool для admin-managed lanes; stake-elected lanes выводят validator pool из записей стейкинга public lanes.

## Telemetria и status

- `/status` показывает lane aliases, dataspace bindings, governance handles и settlement profiles, derived из каталога и `LaneConfig`.
- Scheduler metrics (`nexus_scheduler_lane_teu_*`) отображают lane aliases/slugs, чтобы операторы быстро сопоставляли backlog и TEU pressure.
- `nexus_lane_configured_total` считает число derived lane entries и пересчитывается при изменении конфигурации. Телеметрия излучает signed diffs при изменении геометрии lanes.
- Dataspace backlog gauges включают alias/description metadata, помогая сопоставить queue pressure с бизнес-доменами.

## Configuration и Norito types

- `LaneCatalog`, `LaneConfig` и `DataSpaceCatalog` живут в `iroha_data_model::nexus` и предоставляют структуры в формате Norito для manifests и SDKs.
- `LaneConfig` живет в `iroha_config::parameters::actual::Nexus` и автоматически выводится из каталога; Norito encoding не требуется, потому что это внутренний runtime helper.
- Пользовательская конфигурация (`iroha_config::parameters::user::Nexus`) продолжает принимать декларативные описания lanes и dataspaces; парсинг теперь выводит геометрию и отклоняет invalid aliases или дублирующиеся lane ids.
- `DataSpaceMetadata.fault_tolerance` управляет размером lane-relay committee; membership выбирается детерминированно по эпохе из dataspace validator pool, используя VRF epoch seed, привязанный к `(dataspace_id, lane_id)`.

## Оставшаяся работа

- Интегрировать обновления settlement router (NX-3) с новой геометрией, чтобы XOR buffer debits и receipts были помечены lane slug.
- Завершить merge algorithm (ordering, pruning, conflict detection) и добавить regression fixtures для cross-lane replay.
- Добавить compliance hooks для whitelists/blacklists и programmable-money policies (отслеживается под NX-12).

---

*Этот документ будет развиваться по мере прогресса задач NX-2 - NX-18. Просьба фиксировать открытые вопросы в roadmap или governance tracker.*

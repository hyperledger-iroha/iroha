---
lang: he
direction: rtl
source: docs/portal/docs/nexus/lane-model.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-lane-model
title: Модель lanes Nexus
description: Логическая таксономия lanes, геометрия конфигурации и правила слияния world-state для Sora Nexus.
---

# Модель lanes Nexus и партиционирование WSV

> **Статус:** поставка NX-1 - таксономия lanes, геометрия конфигурации и раскладка хранения готовы к внедрению.  
> **Owners:** Nexus Core WG, Governance WG  
> **Ссылка в roadmap:** NX-1 в `roadmap.md`

Эта страница портала зеркалирует канонический brief `docs/source/nexus_lanes.md`, чтобы операторы Sora Nexus, owners SDK и ревьюеры могли читать руководство по lanes без погружения в дерево mono-repo. Целевая архитектура сохраняет детерминизм world state, при этом позволяя отдельным data spaces (lanes) запускать публичные или частные наборы валидаторов с изолированными workloads.

## Concepts

- **Lane:** логический shard ledger Nexus со своим набором валидаторов и backlog выполнения. Идентифицируется стабильным `LaneId`.
- **Data Space:** governance bucket, группирующий одну или несколько lanes с общими политиками compliance, routing и settlement.
- **Lane Manifest:** governance-контролируемая metadata, описывающая валидаторов, DA policy, gas token, правила settlement и routing permissions.
- **Global Commitment:** proof, выпускаемая lane и суммирующая новые state roots, settlement data и опциональные cross-lane transfers. Глобальное кольцо NPoS упорядочивает commitments.

## Таксономия lanes

Типы lanes канонически описывают видимость, governance surface и settlement hooks. Геометрия конфигурации (`LaneConfig`) фиксирует эти атрибуты, чтобы узлы, SDKs и tooling могли понимать layout без bespoke логики.

| Тип lane | Видимость | Membership валидаторов | Экспозиция WSV | Governance по умолчанию | Политика settlement | Типичное применение |
|-----------|------------|----------------------|--------------|--------------------|-------------------|-------------|
| `default_public` | public | Permissionless (global stake) | Full state replica | SORA Parliament | `xor_global` | Базовый публичный ledger |
| `public_custom` | public | Permissionless or stake-gated | Full state replica | Stake weighted module | `xor_lane_weighted` | Публичные приложения с высоким throughput |
| `private_permissioned` | restricted | Фиксированный набор валидаторов (одобрен governance) | Commitments & proofs | Federated council | `xor_hosted_custody` | CBDC, консорциумные workloads |
| `hybrid_confidential` | restricted | Смешанная membership; оборачивает ZK proofs | Commitments + selective disclosure | Programmable money module | `xor_dual_fund` | Privacy-preserving programmable money |

Все типы lane обязаны объявлять:

- Dataspace alias - человекочитаемая группировка, связывающая политики compliance.
- Governance handle - идентификатор, разрешаемый через `Nexus.governance.modules`.
- Settlement handle - идентификатор, потребляемый settlement router для списания XOR buffers.
- Опциональная telemetry metadata (description, contact, business domain), отображаемая через `/status` и dashboards.

## Геометрия конфигурации lanes (`LaneConfig`)

`LaneConfig` - runtime геометрия, выводимая из валидированного каталога lanes. Она не заменяет governance manifests; вместо этого предоставляет детерминированные storage identifiers и telemetry hints для каждой сконфигурированной lane.

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

- `LaneConfig::from_catalog` пересчитывает геометрию при загрузке конфигурации (`State::set_nexus`).
- Aliases санитизируются в нижний регистр slug; последовательности неалфавитно-цифровых символов схлопываются в `_`. Если alias дает пустой slug, используем `lane{id}`.
- `shard_id` выводится из metadata key `da_shard_id` (по умолчанию `lane_id`) и управляет журналом shard cursor, чтобы DA replay оставался детерминированным между restarts/resharding.
- Key prefixes гарантируют, что WSV сохраняет раздельные key ranges по lane, даже если backend общий.
- Имена Kura segment детерминированы между hosts; аудиторы могут сопоставлять директории сегментов и manifests без bespoke tooling.
- Merge segments (`lane_{id:03}_merge`) хранят последние merge-hint roots и global state commitments для этой lane.

## Партиционирование world-state

- Логический world state Nexus - это объединение state spaces по lane. Public lanes сохраняют full state; private/confidential lanes экспортируют Merkle/commitment roots в merge ledger.
- MV storage префиксирует каждый ключ 4-байтовым префиксом `LaneConfigEntry::key_prefix`, получая ключи вида `[00 00 00 01] ++ PackedKey`.
- Shared tables (accounts, assets, triggers, governance records) хранят записи, сгруппированные по lane prefix, сохраняя детерминизм range scans.
- Merge-ledger metadata зеркалирует тот же layout: каждая lane пишет merge-hint roots и reduced global state roots в `lane_{id:03}_merge`, что позволяет делать targeted retention/eviction при выводе lane.
- Cross-lane indexes (account aliases, asset registries, governance manifests) хранят явные lane prefixes, чтобы операторы могли быстро выполнять reconciliation.
- **Retention policy** - public lanes сохраняют полные block bodies; commitment-only lanes могут компактизировать старые bodies после checkpoints, потому что commitments авторитетны. Confidential lanes хранят ciphertext journals в выделенных сегментах, чтобы не блокировать другие workloads.
- **Tooling** - утилиты обслуживания (`kagami`, CLI admin commands) должны ссылаться на slugged namespace при экспонировании metrics, Prometheus labels или архивировании Kura segments.

## Routing & APIs

- Torii REST/gRPC endpoints принимают опциональный `lane_id`; отсутствие означает `lane_default`.
- SDKs показывают lane selectors и маппят user-friendly aliases в `LaneId` через каталог lanes.
- Routing rules работают по валидированному каталогу и могут выбирать и lane, и dataspace. `LaneConfig` предоставляет telemetry-friendly aliases для dashboards и logs.

## Settlement & fees

- Каждая lane платит XOR fees глобальному набору валидаторов. Lanes могут собирать native gas tokens, но обязаны escrow XOR equivalents вместе с commitments.
- Settlement proofs включают amount, conversion metadata и proof escrow (например, перевод в глобальный fee vault).
- Единый settlement router (NX-3) дебетует buffers теми же lane prefixes, поэтому settlement telemetry совпадает со storage геометрией.

## Governance

- Lanes объявляют свой governance module через каталог. `LaneConfigEntry` несет оригинальные alias и slug, чтобы telemetry и audit trails оставались читаемыми.
- Nexus registry распространяет подписанные lane manifests, включающие `LaneId`, dataspace binding, governance handle, settlement handle и metadata.
- Runtime-upgrade hooks продолжают применять governance policies (`gov_upgrade_id` по умолчанию) и логируют diffs через telemetry bridge (events `nexus.config.diff`).

## Telemetry & status

- `/status` показывает lane aliases, dataspace bindings, governance handles и settlement profiles, производные от каталога и `LaneConfig`.
- Scheduler metrics (`nexus_scheduler_lane_teu_*`) отображают aliases/slugs, чтобы операторы быстро связывали backlog и TEU pressure.
- `nexus_lane_configured_total` считает количество выведенных lane entries и пересчитывается при изменении конфигурации. Telemetry испускает подписанные diffs при изменении lane geometry.
- Dataspace backlog gauges включают alias/description metadata, помогая операторам связывать queue pressure с бизнес-доменами.

## Configuration & Norito types

- `LaneCatalog`, `LaneConfig`, и `DataSpaceCatalog` находятся в `iroha_data_model::nexus` и предоставляют Norito-совместимые структуры для manifests и SDKs.
- `LaneConfig` находится в `iroha_config::parameters::actual::Nexus` и автоматически выводится из каталога; Norito encoding не требуется, так как это внутренний runtime helper.
- Пользовательская конфигурация (`iroha_config::parameters::user::Nexus`) продолжает принимать декларативные descriptors lane и dataspace; парсинг теперь выводит геометрию и отклоняет невалидные aliases или дублирующиеся lane IDs.

## Оставшаяся работа

- Интегрировать settlement router updates (NX-3) с новой геометрией, чтобы XOR buffer debits и receipts помечались slug lane.
- Расширить admin tooling для списка column families, компакта retired lanes и проверки per-lane block logs через slugged namespace.
- Финализировать merge алгоритм (ordering, pruning, conflict detection) и добавить regression fixtures для cross-lane replay.
- Добавить compliance hooks для whitelists/blacklists и programmable-money policies (трекится под NX-12).

---

*Эта страница продолжит отслеживать NX-1 follow-ups по мере посадки NX-2 до NX-18. Пожалуйста, поднимайте открытые вопросы в `roadmap.md` или governance tracker, чтобы портал оставался синхронизированным с каноническими документами.*

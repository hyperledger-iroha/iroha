---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-refactor-plan
title: План рефакторинга Sora Nexus Ledger
description: Зеркало `docs/source/nexus_refactor_plan.md`, описывающее поэтапную зачистку кодовой базы Iroha 3.
---

:::note Канонический источник
Эта страница отражает `docs/source/nexus_refactor_plan.md`. Держите обе копии синхронизированными, пока многоязычная версия не появится в портале.
:::

# План рефакторинга Sora Nexus Ledger

Этот документ фиксирует ближайший roadmap по рефакторингу Sora Nexus Ledger ("Iroha 3"). Он отражает текущую структуру репозитория и регрессии, замеченные в учете genesis/WSV, консенсусе Sumeragi, триггерах смарт-контрактов, snapshot queries, host bindings pointer-ABI и Norito codecs. Цель - прийти к согласованной, тестируемой архитектуре, не пытаясь посадить все исправления одним монолитным патчем.

## 0. Направляющие принципы
- Сохранять детерминированное поведение на гетерогенном железе; использовать ускорение только через opt-in feature flags с идентичными fallbacks.
- Norito - слой сериализации. Любые изменения state/schema должны включать Norito encode/decode round-trip тесты и обновления fixtures.
- Конфигурация проходит через `iroha_config` (user -> actual -> defaults). Уберите ad-hoc environment toggles из продакшн путей.
- Политика ABI остается V1 и не подлежит обсуждению. Hosts должны детерминированно отклонять неизвестные pointer types/syscalls.
- `cargo test --workspace` и golden tests (`ivm`, `norito`, `integration_tests`) остаются базовым гейтом для каждого этапа.

## 1. Снимок топологии репозитория
- `crates/iroha_core`: Sumeragi actors, WSV, genesis loader, pipelines (query, overlay, zk lanes), glue для smart-contract host.
- `crates/iroha_data_model`: авторитетная схема on-chain данных и queries.
- `crates/iroha`: client API, используемый CLI, tests, SDK.
- `crates/iroha_cli`: операторский CLI, сейчас зеркалит множество APIs из `iroha`.
- `crates/ivm`: Kotodama bytecode VM, entry points для pointer-ABI host integration.
- `crates/norito`: serialization codec с JSON adapters и AoS/NCB backends.
- `integration_tests`: кросс-компонентные assertions, покрывающие genesis/bootstrap, Sumeragi, triggers, pagination и т.д.
- Docs уже описывают цели Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`), но реализация фрагментирована и частично устарела относительно кода.

## 2. Опорные элементы рефакторинга и вехи

### Phase A - Foundations and Observability
1. **WSV Telemetry + Snapshots**
   - Зафиксировать canonical snapshot API в `state` (trait `WorldStateSnapshot`), используемый queries, Sumeragi и CLI.
   - Использовать `scripts/iroha_state_dump.sh` для детерминированных snapshots через `iroha state dump --format norito`.
2. **Genesis/Bootstrap Determinism**
   - Переписать ingest genesis так, чтобы он проходил через единый Norito pipeline (`iroha_core::genesis`).
   - Добавить integration/regression покрытие, которое воспроизводит genesis плюс первый блок и подтверждает идентичные WSV roots между arm64/x86_64 (трекится в `integration_tests/tests/genesis_replay_determinism.rs`).
3. **Cross-crate Fixity Tests**
   - Расширить `integration_tests/tests/genesis_json.rs`, чтобы валидировать WSV, pipeline и ABI invariants в одном harness.
   - Ввести scaffold `cargo xtask check-shape`, который паникует при schema drift (трекится в DevEx tooling backlog; см. action item в `scripts/xtask/README.md`).

### Phase B - WSV & Query Surface
1. **State Storage Transactions**
   - Свести `state/storage_transactions.rs` к транзакционному адаптеру, который обеспечивает порядок commit и детектирует конфликты.
   - Unit tests теперь проверяют, что модификации assets/world/triggers откатываются при ошибках.
2. **Query Model Refactor**
   - Перенести pagination/cursor логику в переиспользуемые компоненты под `crates/iroha_core/src/query/`. Согласовать Norito representations в `iroha_data_model`.
   - Добавить snapshot queries для triggers, assets и roles с детерминированным порядком (отслеживается через `crates/iroha_core/tests/snapshot_iterable.rs` для текущего покрытия).
3. **Snapshot Consistency**
   - Убедиться, что `iroha ledger query` CLI использует тот же snapshot path, что и Sumeragi/fetchers.
   - CLI snapshot regression tests находятся в `tests/cli/state_snapshot.rs` (feature-gated для медленных прогонов).

### Phase C - Sumeragi Pipeline
1. **Topology & Epoch Management**
   - Вынести `EpochRosterProvider` в trait с реализациями, основанными на WSV stake snapshots.
   - `WsvEpochRosterAdapter::from_peer_iter` предлагает простой constructor, удобный для mock в benches/tests.
2. **Consensus Flow Simplification**
   - Реорганизовать `crates/iroha_core/src/sumeragi/*` в модули: `pacemaker`, `aggregation`, `availability`, `witness` с общими типами под `consensus`.
   - Заменить ad-hoc message passing на типизированные Norito envelopes и добавить view-change property tests (трекится в Sumeragi messaging backlog).
3. **Lane/Proof Integration**
   - Согласовать lane proofs с DA commitments и обеспечить единообразный RBC gating.
   - End-to-end интеграционный тест `integration_tests/tests/extra_functional/seven_peer_consistency.rs` теперь проверяет путь с включенным RBC.

### Phase D - Smart Contracts & Pointer-ABI Hosts
1. **Host Boundary Audit**
   - Консолидировать pointer-type проверки (`ivm::pointer_abi`) и host adapters (`iroha_core::smartcontracts::ivm::host`).
   - Ожидания pointer table и host manifest bindings покрыты тестами `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` и `ivm_host_mapping.rs`, которые проверяют golden TLV mappings.
2. **Trigger Execution Sandbox**
   - Перефакторить triggers, чтобы они выполнялись через общий `TriggerExecutor`, который применяет gas, pointer validation и event journaling.
   - Добавить regression tests для call/time triggers с покрытием failure paths (трекится через `crates/iroha_core/tests/trigger_failure.rs`).
3. **CLI & Client Alignment**
   - Обеспечить, чтобы CLI операции (`audit`, `gov`, `sumeragi`, `ivm`) использовали общие client функции `iroha`, чтобы избежать drift.
   - CLI JSON snapshot tests находятся в `tests/cli/json_snapshot.rs`; держите их актуальными, чтобы вывод core команд совпадал с canonical JSON reference.

### Phase E - Norito Codec Hardening
1. **Schema Registry**
   - Создать Norito schema registry под `crates/norito/src/schema/`, чтобы задавать canonical encodings для core data types.
   - Добавлены doc tests, проверяющие encoding sample payloads (`norito::schema::SamplePayload`).
2. **Golden Fixtures Refresh**
   - Обновить golden fixtures в `crates/norito/tests/*`, чтобы они соответствовали новой WSV schema после посадки рефакторинга.
   - `scripts/norito_regen.sh` детерминированно регенерирует Norito JSON goldens через helper `norito_regen_goldens`.
3. **IVM/Norito Integration**
   - Провалидировать сериализацию Kotodama manifest end-to-end через Norito, гарантируя консистентность pointer ABI metadata.
   - `crates/ivm/tests/manifest_roundtrip.rs` удерживает Norito encode/decode паритет для manifests.

## 3. Сквозные вопросы
- **Testing Strategy**: Каждый этап продвигает unit tests -> crate tests -> integration tests. Падающие тесты фиксируют текущие регрессии; новые тесты не дают им повториться.
- **Documentation**: После посадки каждой фазы обновлять `status.md` и переносить незакрытые пункты в `roadmap.md`, удаляя завершенные задачи.
- **Performance Benchmarks**: Сохранять существующие benches в `iroha_core`, `ivm` и `norito`; добавлять базовые измерения после рефакторинга, чтобы подтвердить отсутствие регрессий.
- **Feature Flags**: Оставлять crate-level toggles только для backends, требующих внешних toolchains (`cuda`, `zk-verify-batch`). CPU SIMD пути всегда собираются и выбираются во время выполнения; обеспечить детерминированные scalar fallbacks для неподдерживаемого железа.

## 4. Ближайшие действия
- Phase A scaffolding (snapshot trait + telemetry wiring) - см. actionable tasks в обновлениях roadmap.
- Недавний аудит дефектов по `sumeragi`, `state` и `ivm` выделил следующие моменты:
  - `sumeragi`: dead-code allowances защищают broadcast view-change proof, VRF replay state и EMA telemetry export. Они остаются gated, пока не будут выполнены deliverables Phase C по упрощению consensus flow и интеграции lane/proof.
  - `state`: `Cell` cleanup и telemetry routing переходят в Phase A WSV telemetry track, а заметки SoA/parallel-apply входят в Phase C pipeline optimization backlog.
  - `ivm`: экспонирование CUDA toggle, envelope validation и Halo2/Metal coverage маппятся на Phase D host-boundary работу плюс сквозную тему GPU acceleration; kernels остаются в отдельном GPU backlog до готовности.
- Подготовить cross-team RFC с резюме этого плана для sign-off перед посадкой инвазивных изменений кода.

## 5. Открытые вопросы
- Должен ли RBC оставаться опциональным после P1, или он обязателен для Nexus ledger lanes? Требуется решение стейкхолдеров.
- Нужны ли DS composability groups в P1 или держать их выключенными до зрелости lane proofs?
- Какое каноническое место для параметров ML-DSA-87? Кандидат: новый crate `crates/fastpq_isi` (создание ожидается).

---

_Последнее обновление: 2025-09-12_

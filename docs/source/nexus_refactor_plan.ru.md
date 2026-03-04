---
lang: ru
direction: ltr
source: docs/source/nexus_refactor_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44b7100fddd377c97dfcab678ce425ec35edfa4a1276f9b6a22aa2c64135a94d
source_last_modified: "2025-11-02T04:40:40.017979+00:00"
translation_last_reviewed: 2026-01-01
---

# План рефакторинга Sora Nexus Ledger

Этот документ фиксирует ближайшую дорожную карту рефакторинга Sora Nexus Ledger ("Iroha 3"). Он
отражает текущую структуру репозитория и регрессии, наблюдаемые в учете genesis/WSV, консенсусе
Sumeragi, триггерах smart-contract, запросах snapshot, привязках host pointer-ABI и кодеках Norito.
Цель — прийти к согласованной, тестируемой архитектуре без попытки внести все исправления одним
монолитным патчем.

## 0. Руководящие принципы
- Сохранять детерминированное поведение на разнородном железе; использовать ускорение только через
  opt-in feature flags с идентичными fallback.
- Norito — слой сериализации. Любые изменения состояния/схемы должны включать round-trip тесты
  Norito encode/decode и обновление fixtures.
- Конфигурация проходит через `iroha_config` (user -> actual -> defaults). Удалить ad-hoc
  environment toggles из production путей.
- Политика ABI остается V1 и не обсуждается. Hosts должны детерминированно отклонять неизвестные
  pointer types/syscalls.
- `cargo test --workspace` и golden тесты (`ivm`, `norito`, `integration_tests`) остаются базовым
  gate для каждого вехового этапа.

## 1. Снимок топологии репозитория
- `crates/iroha_core`: акторы Sumeragi, WSV, загрузчик genesis, пайплайны (query, overlay, zk lanes),
  glue host для smart-contract.
- `crates/iroha_data_model`: авторитетная схема для on-chain данных и запросов.
- `crates/iroha`: клиентский API, используемый CLI, тестами и SDK.
- `crates/iroha_cli`: CLI оператора, сейчас зеркалирует множество API из `iroha`.
- `crates/ivm`: VM байткода Kotodama, точки входа интеграции host pointer-ABI.
- `crates/norito`: сериализационный codec с JSON адаптерами и AoS/NCB backend.
- `integration_tests`: cross-component assertions по genesis/bootstrap, Sumeragi, trigger, pagination и т.д.
- Документация уже описывает цели Sora Nexus Ledger (`nexus.md`, `new_pipeline.md`, `ivm.md`), но
  реализация фрагментирована и частично устарела относительно кода.

## 2. Пилоны рефакторинга и вехи

### Фаза A - Фундамент и наблюдаемость
1. **WSV Telemetry + Snapshots**
   - Установить канонический API snapshot в `state` (trait `WorldStateSnapshot`), используемый
     запросами, Sumeragi и CLI.
   - Использовать `scripts/iroha_state_dump.sh` для детерминированных snapshots через
     `iroha state dump --format norito`.
2. **Детерминизм genesis/bootstrap**
   - Рефакторить ingestion genesis через единый Norito-пайплайн (`iroha_core::genesis`).
   - Добавить интеграционное/регрессионное покрытие, которое проигрывает genesis плюс первый блок
     и утверждает идентичные WSV roots на arm64/x86_64 (трек в
     `integration_tests/tests/genesis_replay_determinism.rs`).
3. **Тесты фиксации cross-crate**
   - Расширить `integration_tests/tests/genesis_json.rs` для проверки инвариантов WSV, pipeline и
     ABI в одном harness.
   - Ввести scaffold `cargo xtask check-shape`, который падает при schema drift (трек в backlog
     DevEx tooling; см. action item `scripts/xtask/README.md`).

### Фаза B - WSV и поверхность запросов
1. **Транзакции state storage**
   - Свернуть `state/storage_transactions.rs` в транзакционный адаптер, который обеспечивает
     порядок commit и детекцию конфликтов.
   - Юнит-тесты теперь проверяют, что изменения assets/world/triggers откатываются при сбоях.
2. **Рефактор модели запросов**
   - Перенести логику pagination/cursor в переиспользуемые компоненты под
     `crates/iroha_core/src/query/`. Согласовать Norito представления в `iroha_data_model`.
   - Добавить snapshot queries для triggers, assets и roles с детерминированной сортировкой
     (трек через `crates/iroha_core/tests/snapshot_iterable.rs` для текущего покрытия).
3. **Консистентность snapshot**
   - Убедиться, что CLI `iroha ledger query` использует тот же путь snapshot, что и Sumeragi/fetchers.
   - Регрессионные snapshot тесты CLI находятся в `tests/cli/state_snapshot.rs` (feature-gated для
     медленных прогонов).

### Фаза C - Pipeline Sumeragi
1. **Топология и управление эпохами**
   - Выделить `EpochRosterProvider` в trait с реализациями на базе WSV stake snapshots.
   - `WsvEpochRosterAdapter::from_peer_iter` дает простой, mock-friendly конструктор для benches/tests.
2. **Упрощение потока консенсуса**
   - Реорганизовать `crates/iroha_core/src/sumeragi/*` в модули: `pacemaker`, `aggregation`,
     `availability`, `witness` с общими типами под `consensus`.
   - Заменить ad-hoc message passing на типизированные Norito envelopes и добавить property tests
     для view-change (трек в backlog messaging Sumeragi).
3. **Интеграция lane/proof**
   - Согласовать lane proofs с DA commitments и обеспечить единый RBC gating.
   - End-to-end тест `integration_tests/tests/extra_functional/seven_peer_consistency.rs` теперь
     проверяет путь с включенным RBC.

### Фаза D - Smart contracts и pointer-ABI hosts
1. **Аудит границы host**
   - Консолидировать проверки типов указателей (`ivm::pointer_abi`) и host адаптеры
     (`iroha_core::smartcontracts::ivm::host`).
   - Ожидания pointer table и bindings host manifest покрываются `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs`
     и `ivm_host_mapping.rs`, которые проверяют golden TLV mappings.
2. **Sandbox исполнения triggers**
   - Рефакторить triggers на общий `TriggerExecutor`, который обеспечивает gas, pointer validation
     и journaling событий.
   - Добавить регрессионные тесты для call/time triggers, покрывающие failure paths (трек в
     `crates/iroha_core/tests/trigger_failure.rs`).
3. **Согласование CLI и client**
   - Убедиться, что операции CLI (`audit`, `gov`, `sumeragi`, `ivm`) используют общие функции
     клиента `iroha`, чтобы избежать drift.
   - JSON snapshot тесты CLI находятся в `tests/cli/json_snapshot.rs`; поддерживать их в актуальном
     состоянии, чтобы вывод core команд соответствовал каноническому JSON референсу.

### Фаза E - Укрепление Norito codec
1. **Schema registry**
   - Создать Norito schema registry в `crates/norito/src/schema/` для канонических encodings core типов.
   - Добавить doc tests, проверяющие encoding sample payloads (`norito::schema::SamplePayload`).
2. **Обновление golden fixtures**
   - Обновить `crates/norito/tests/*` golden fixtures под новую WSV schema после посадки рефакторинга.
   - `scripts/norito_regen.sh` детерминированно регенерирует Norito JSON goldens через helper
     `norito_regen_goldens`.
3. **Интеграция IVM/Norito**
   - Валидировать сериализацию Kotodama manifest end-to-end через Norito, обеспечивая консистентность
     metadata pointer ABI.
   - `crates/ivm/tests/manifest_roundtrip.rs` сохраняет паритет Norito encode/decode для manifests.

## 3. Сквозные вопросы
- **Testing Strategy**: Каждая фаза продвигает unit tests -> crate tests -> integration tests.
  Падающие тесты фиксируют текущие регрессии; новые тесты предотвращают их возврат.
- **Documentation**: После каждой фазы обновлять `status.md` и переносить открытые items в
  `roadmap.md`, удаляя завершенные задачи.
- **Performance Benchmarks**: Сохранять существующие benches в `iroha_core`, `ivm`, `norito`;
  добавить базовые измерения после рефакторинга, чтобы убедиться в отсутствии регрессий.
- **Feature Flags**: Оставлять crate-level toggles только для backend, требующих внешние toolchains
  (`cuda`, `zk-verify-batch`). CPU SIMD пути всегда собираются и выбираются в runtime; предоставить
  детерминированные scalar fallbacks для неподдерживаемого hardware.

## 4. Ближайшие действия
- Scaffolding фазы A (snapshot trait + telemetry wiring) — см. задачи в обновлениях roadmap.
- Последний дефектный аудит для `sumeragi`, `state` и `ivm` выявил:
  - `sumeragi`: dead-code allowances защищают broadcast view-change proof, VRF replay state и EMA
    telemetry export. Это остается gated до упрощения консенсусного потока и интеграции lane/proof
    в фазе C.
  - `state`: cleanup `Cell` и telemetry routing переходят в фазу A WSV telemetry, а заметки
    SoA/parallel-apply уходят в backlog оптимизации pipeline фазы C.
  - `ivm`: экспозиция CUDA toggle, envelope validation и покрытие Halo2/Metal относятся к работе
    по границе host в фазе D и к сквозной теме GPU acceleration; kernels остаются в выделенном GPU
    backlog до готовности.
- Подготовить cross-team RFC, суммирующий этот план, для sign-off перед внесением инвазивных изменений.

## 5. Открытые вопросы
- Должен ли RBC оставаться опциональным после P1 или быть обязательным для Nexus ledger lanes?
  Нужное решение стейкхолдеров.
- Должны ли мы принудительно вводить DS composability groups в P1 или оставить их отключенными до
  зрелости lane proofs?
- Где каноническое место для параметров ML-DSA-87? Кандидат: новый crate `crates/fastpq_isi`
  (создание ожидается).

---

_Последнее обновление: 2025-09-12_

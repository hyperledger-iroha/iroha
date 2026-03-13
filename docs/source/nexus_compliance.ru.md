---
lang: ru
direction: ltr
source: docs/source/nexus_compliance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5635794e962a9fb1b94c5ff550dc198a744a64b4f9f05df588cb70621e9237f9
source_last_modified: "2025-11-21T18:31:26.542844+00:00"
translation_last_reviewed: 2026-01-01
---

# Движок комплаенса lanes Nexus и политика белого списка (NX-12)

Статус: 🈴 Реализовано — этот документ фиксирует рабочую модель политики и
консенсус-критичное применение, на которые ссылается пункт roadmap
**NX-12 — Движок комплаенса lane и политика белого списка**.
Он объясняет модель данных, потоки governance, телеметрию и стратегию раскатки,
реализованные в `crates/iroha_core/src/compliance` и применяемые как при admission Torii,
так и при валидации транзакций `iroha_core`, чтобы каждая lane и dataspace могла быть привязана к
детерминированным юрисдикционным политикам.

## Цели

- Позволить governance прикреплять allow/deny правила, флаги юрисдикций, лимиты переводов CBDC
  и требования аудита к каждому lane manifest.
- Проверять каждую транзакцию по этим правилам при admission в Torii и при исполнении блока,
  обеспечивая детерминированное применение политики между узлами.
- Формировать криптографически проверяемый audit trail, включая bundles доказательств Norito
  и запрашиваемую телеметрию для регуляторов и операторов.
- Сохранить гибкость модели: один и тот же policy engine покрывает приватные CBDC lanes,
  публичные settlement DS и гибридные dataspaces партнеров без bespoke forks.

## Не-Цели

- Определение процедур AML/KYC или юридических escalation workflows. Это описано в compliance playbooks,
  которые потребляют телеметрию, производимую здесь.
- Введение per-instruction toggles в IVM; движок контролирует только какие accounts/assets/domains
  могут отправлять транзакции или взаимодействовать с lane.
- Устаревание Space Directory. Манифесты остаются авторитетным источником метаданных DS;
  политика комплаенса лишь ссылается на записи Space Directory и дополняет их.

## Модель политики

### Сущности и идентификаторы

Движок политики оперирует:

- `LaneId` / `DataSpaceId` — идентифицирует область, где применяются правила.
- `UniversalAccountId (UAID)` — позволяет группировать cross-lane идентичности.
- `JurisdictionFlag` — bitmask, перечисляющий регуляторные классификации (например,
  `EU_EEA`, `JP_FIEL`, `US_FED`, `SANCTIONS_SCREENED`).
- `ParticipantSelector` — описывает, кого затрагивают правила:
  - `AccountId`, `DomainId` или `UAID`.
  - Селекторы по префиксу (`DomainPrefix`, `UaidPrefix`) для сопоставления реестров.
  - `CapabilityTag` для манифестов Space Directory (например, только DS с FX-cleared).
  - gating `privacy_commitments_any_of`, требующий, чтобы lanes заявляли конкретные Nexus privacy commitments
    до совпадения правил (отражает manifest surface NX-10 и применяется в snapshots `LanePrivacyRegistry`).

### LaneCompliancePolicy

Политики — это Norito-encoded структуры, публикуемые через governance:

```text
LaneCompliancePolicy {
    id: LaneCompliancePolicyId,
    version: u32,
    lane_id: LaneId,
    jurisdiction: JurisdictionSet,
    allow: Vec<AllowRule>,
    deny: Vec<DenyRule>,
    transfer_limits: Vec<TransferLimit>,
    audit_controls: AuditControls,
    metadata: MetadataMap,
}
```

- `AllowRule` комбинирует `ParticipantSelector`, опциональный jurisdiction override,
  capability tags и коды причин.
- `DenyRule` зеркалирует структуру allow, но оценивается первым (deny wins).
- `TransferLimit` фиксирует лимиты по активу/bucket:
  - `max_notional_xor` и `max_daily_notional_xor`.
  - `asset_limits[{asset_id, per_tx, per_day}]`.
  - `relationship_limits` (например, CBDC retail vs wholesale).
- `AuditControls` настраивает:
  - Нужно ли Torii сохранять каждый отказ в журнале аудита.
  - Нужно ли сэмплировать успешные решения в Norito digests.
  - Требуемое окно хранения для `LaneComplianceDecisionRecord`.

### Хранение и распространение

- Актуальные policy hashes живут в manifest Space Directory рядом с ключами валидаторов.
  `LaneCompliancePolicyReference` (policy id + version + hash) становится полем manifest,
  чтобы валидаторы и SDKs могли получать канонический policy blob.
- `iroha_config` открывает `compliance.policy_cache_dir` для хранения Norito payload и его detached signature.
  Узлы проверяют подписи перед применением обновлений, чтобы защититься от подмены.
- Политики также встраиваются в Norito admission manifests, используемые Torii,
  чтобы CI/SDKs могли воспроизводить оценку политик без обращения к валидаторам.

## Governance и жизненный цикл

1. **Предложение** — governance отправляет `ProposeLaneCompliancePolicy` с Norito payload,
   обоснованием юрисдикции и эпохой активации.
2. **Review** — compliance reviewers подписывают `LaneCompliancePolicyReviewEvidence`
   (аудируемо, хранится в `governance::ReviewEvidenceStore`).
3. **Активация** — после окна задержки валидаторы применяют политику, вызывая
   `ActivateLaneCompliancePolicy`. Manifest Space Directory обновляется атомарно
   с новой ссылкой на политику.
4. **Amend/Revoke** — `AmendLaneCompliancePolicy` переносит diff metadata, сохраняя прошлую
   версию для forensic replay; `RevokeLaneCompliancePolicy` закрепляет policy id как `denied`,
   чтобы Torii отклонял любой трафик на эту lane до активации замены.

Torii предоставляет:

- `GET /v2/lane-compliance/policies/{lane_id}` — получить актуальную policy reference.
- `POST /v2/lane-compliance/policies` — endpoint только для governance, отражающий ISI proposal helpers.
- `GET /v2/lane-compliance/decisions` — пагинированный аудит-лог с фильтрами по
  `lane_id`, `decision`, `jurisdiction` и `reason_code`.

Команды CLI/SDK оборачивают эти HTTP поверхности, чтобы операторы могли скриптовать ревью
и получать артефакты (подписанный policy blob + reviewer attestations).

## Pipeline применения

1. **Admission (Torii)**
   - `Torii` загружает активную политику при изменении lane manifest или истечении cache signature.
   - Каждая транзакция, входящая в очередь `/v2/pipeline`, помечается `LaneComplianceContext`
     (ids участников, UAID, метаданные manifest dataspace, policy id и актуальный snapshot
     `LanePrivacyRegistry`, описанный в `crates/iroha_core/src/interlane/mod.rs`).
   - Авторитеты с UAID должны иметь активный manifest Space Directory для маршрутизируемого dataspace;
     Torii отклоняет транзакции, когда UAID не привязан к этому dataspace до оценки правил политики.
   - `compliance::Engine` оценивает `deny` правила, затем `allow`, и затем применяет transfer limits.
     Ошибочные транзакции возвращают типизированную ошибку (`ERR_LANE_COMPLIANCE_DENIED`) с причиной
     и policy id для аудита.
   - Admission — быстрый prefilter; консенсусная валидация повторно проверяет те же правила,
     используя snapshots состояния, чтобы enforcement оставался детерминированным.
2. **Execution (iroha_core)**
   - При построении блока `iroha_core::tx::validate_transaction_internal`
     повторяет те же проверки governance/UAID/privacy/compliance для lane, используя
     snapshots `StateTransaction` (`lane_manifests`, `lane_privacy_registry`,
     `lane_compliance`). Это делает enforcement критичным для консенсуса даже при устаревших cache Torii.
   - Транзакции, которые меняют lane manifests или политики комплаенса, проходят тот же путь валидации;
     admission-only bypass отсутствует.
3. **Async hooks**
   - RBC gossip и DA fetchers прикрепляют policy id к телеметрии, чтобы поздние решения можно было
     связать с нужной версией правил.
   - `iroha_cli` и SDK helpers предоставляют `LaneComplianceDecision::explain()` для вывода понятной диагностики.

Движок детерминированный и чистый; он не обращается к внешним системам после загрузки manifest/policy.
Это упрощает CI fixtures и воспроизводимость между узлами.

## Аудит и телеметрия

- **Метрики**
  - `nexus_lane_policy_decisions_total{lane_id,decision,reason}`.
  - `nexus_lane_policy_rate_limited_total{lane_id,limit_kind}`.
  - `nexus_lane_policy_cache_age_seconds{lane_id}` (должно быть < activation delay).
- **Логи**
  - Структурированные записи фиксируют `policy_id`, `version`, `participant`, `UAID`,
    флаги юрисдикций и Norito hash нарушающей транзакции.
  - `LaneComplianceDecisionRecord` кодируется в Norito и сохраняется под
    `world.compliance_logs::<lane_id>::<ts>::<nonce>` когда `AuditControls`
    требует долговременного хранения.
- **Evidence bundles**
  - `cargo xtask nexus-lane-audit` получает режим `--lane-compliance <path>`, который объединяет политику,
    подписи ревьюеров, snapshot метрик и последний аудит-лог в JSON + Parquet выходы. Флаг ожидает JSON
    payload формата:

    ```json
    {
      "lanes": [
        {
          "lane_id": 12,
          "policy": { "...": "LaneCompliancePolicy JSON blob" },
          "reviewer_signatures": [
            {
              "reviewer": "auditor@example.com",
              "signature_hex": "deadbeef",
              "signed_at": "2026-02-12T09:00:00Z",
              "notes": "Q1 regulator packet"
            }
          ],
          "metrics_snapshot": {
            "nexus_lane_policy_decisions_total": {
              "allow": 42,
              "deny": 1
            }
          },
          "audit_log": [
            {
              "decision": "allow",
              "policy_id": "lane-12-policy",
              "recorded_at": "2026-02-12T09:00:00Z"
            }
          ]
        }
      ]
    }
    ```

    CLI проверяет, что каждый `policy` blob совпадает с `lane_id` в записи перед встраиванием,
    предотвращая устаревшие или несоответствующие доказательства в регуляторных пакетах и roadmap dashboards.
  - `--markdown-out` (по умолчанию `artifacts/nexus_lane_audit.md`) теперь рендерит читаемую сводку,
    выделяя отстающие lanes, ненулевой backlog, ожидающие manifests и недостающие evidence of compliance,
    чтобы annex пакеты включали и machine-readable артефакты, и быстрый обзор.

## План rollout

1. **P0 — Только наблюдаемость**
   - Поставить типы политик, хранение, Torii endpoints и метрики.
   - Torii оценивает политики в режиме `audit` (без enforcement) для сбора данных.
2. **P1 — Deny/allow enforcement**
   - Включить жесткие отказы в Torii и execution при срабатывании deny правил.
   - Требовать политики для всех CBDC lanes; публичные DS могут оставаться в audit режиме.
3. **P2 — Лимиты и jurisdiction overrides**
   - Включить enforcement лимитов переводов и флагов юрисдикций.
   - Подавать телеметрию в `dashboards/grafana/nexus_lanes.json`.
4. **P3 — Полная автоматизация комплаенса**
   - Интегрировать audit exports с потребителями `SpaceDirectoryEvent`.
   - Привязать обновления политик к governance runbooks и release automation.

## Приемка и тестирование

- Интеграционные тесты в `integration_tests/tests/nexus/compliance.rs` покрывают:
  - комбинации allow/deny, jurisdiction overrides и transfer limits;
  - гонки активации manifest/policy; и
  - паритет решений Torii vs `iroha_core` в multi-node прогоне.
- Unit tests в `crates/iroha_core/src/compliance` валидируют чистый evaluation engine,
  таймеры cache invalidation и разбор метаданных.
- Обновления Docs/SDK (Torii + CLI) должны показывать получение политик,
  отправку governance proposals, интерпретацию error codes и сбор audit evidence.

Закрытие NX-12 требует указанных артефактов и обновлений статуса в
`status.md`/`roadmap.md` после включения enforcement в staging кластерах.

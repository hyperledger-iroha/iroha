<!-- Auto-generated stub for Russian (ru) translation. Replace this content with the full translation. -->

---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/migration-roadmap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bfbe7de97848ee43284d448f9d80b78f68b5e95d36e0f86d2aa12c3633838867
source_last_modified: "2025-11-07T10:28:53.296738+00:00"
translation_last_reviewed: 2025-12-30
---

---
title: "Дорожная карта миграции SoraFS"
---

> Адаптировано из [`docs/source/sorafs/migration_roadmap.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_roadmap.md).

# Дорожная карта миграции SoraFS (SF-1)

Этот документ операционализирует рекомендации по миграции, зафиксированные в
`docs/source/sorafs_architecture_rfc.md`. Он разворачивает deliverables SF-1 в
готовые к исполнению вехи, критерии гейтов и чек-листы владельцев, чтобы команды
артефактов к публикации на базе SoraFS.

Дорожная карта намеренно детерминирована: каждая веха называет необходимые
артефакты, команды и шаги аттестации, чтобы downstream-пайплайны производили
идентичные выходы, а governance сохраняла аудируемый след.

## Обзор вех

| Веха | Окно | Основные цели | Должно быть доставлено | Владельцы |
|------|------|---------------|-------------------------|-----------|
| **M1 - Deterministic Enforcement** | Недели 7-12 | Принудить подписанные fixtures и подготовить alias proofs, пока пайплайны внедряют expectation flags. | Ночная верификация fixtures, манифесты с подписью совета, staging записи в alias registry. | Storage, Governance, SDKs |

Статус вех отслеживается в `docs/source/sorafs/migration_ledger.md`. Все изменения
в этой дорожной карте ДОЛЖНЫ обновлять ledger, чтобы governance и release engineering
оставались синхронизированы.

## Потоки работ

### 2. Принятие детерминированного pinning

| Шаг | Веха | Описание | Owner(s) | Выход |
|-----|------|----------|----------|-------|
| Репетиции fixtures | M0 | Еженедельные dry-runs, сравнивающие локальные chunk digests с `fixtures/sorafs_chunker`. Публиковать отчет в `docs/source/sorafs/reports/`. | Storage Providers | `determinism-<date>.md` с матрицей pass/fail. |
| Принудить подписи | M1 | `ci/check_sorafs_fixtures.sh` + `.github/workflows/sorafs-fixtures-nightly.yml` падают при drift подписей или manifests. Dev overrides требуют governance waiver в PR. | Tooling WG | Лог CI, ссылка на waiver ticket (если применимо). |
| Expectation flags | M1 | Пайплайны вызывают `sorafs_manifest_stub` с явными expectations для фиксации output: | Docs CI | Обновленные скрипты со ссылкой на expectation flags (см. блок команды ниже). |
| Registry-first pinning | M2 | `sorafs pin propose` и `sorafs pin approve` оборачивают отправку manifest; CLI по умолчанию использует `--require-registry`. | Governance Ops | Registry CLI audit log, телеметрия неудачных предложений. |
| Observability parity | M3 | Dashboards Prometheus/Grafana предупреждают о расхождении chunk inventory и registry manifests; alert'ы подключены к ops on-call. | Observability | Ссылка на dashboard, IDs правил алертов, результаты GameDay. |

#### Каноническая команда публикации

```bash
cargo run -p sorafs_manifest --bin sorafs_manifest_stub -- docs/book \
  --manifest-out artifacts/docs/book/2025-11-01/docs.manifest \
  --manifest-signatures-out artifacts/docs/book/2025-11-01/docs.manifest_signatures.json \
  --car-out artifacts/docs/book/2025-11-01/docs.car \
  --chunk-fetch-plan-out artifacts/docs/book/2025-11-01/docs.fetch_plan.json \
  --car-digest=13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482 \
  --car-size=429391872 \
  --root-cid=f40101... \
  --dag-codec=0x71
```

Замените значения digest, размера и CID на ожидаемые ссылки, указанные в записи
migration ledger для артефакта.

### 3. Переход на alias и коммуникации

| Шаг | Веха | Описание | Owner(s) | Выход |
|-----|------|----------|----------|-------|
| Alias proofs в staging | M1 | Зарегистрировать alias claims в Pin Registry staging и прикрепить Merkle proofs к manifests (`--alias`). | Governance, Docs | Proof bundle рядом с manifest + комментарий ledger с именем alias. |
| Proof enforcement | M2 | Gateways отклоняют manifests без свежих `Sora-Proof` headers; CI получает шаг `sorafs alias verify` для извлечения proofs. | Networking | Патч конфигурации gateway + CI output с успешной проверкой. |

### 4. Коммуникации и аудит

- **Дисциплина ledger:** каждое изменение состояния (fixture drift, registry submission,
  alias activation) должно добавлять датированную заметку в
  `docs/source/sorafs/migration_ledger.md`.
- **Governance minutes:** заседания совета, утверждающие изменения pin registry или
  alias политик, должны ссылаться на эту дорожную карту и ledger.
- **Внешние коммуникации:** DevRel публикует обновления статуса на каждом этапе (блог +
  excerpt changelog), подчеркивая детерминированные гарантии и таймлайны alias.

## Зависимости и риски

| Зависимость | Влияние | Митигация |
|------------|---------|-----------|
| Доступность контракта Pin Registry | Блокирует M2 pin-first rollout. | Подготовить контракт до M2 с replay тестами; поддерживать envelope fallback до отсутствия регрессий. |
| Ключи подписи совета | Требуются для manifest envelopes и registry approvals. | Signing ceremony описана в `docs/source/sorafs/signing_ceremony.md`; ротировать ключи с перекрытием и записью в ledger. |
| Cadence релизов SDK | Клиенты должны уважать alias proofs до M3. | Синхронизировать окна релизов SDK с milestone gates; добавить migration checklists в release templates. |

Остаточные риски и митигации отражены в `docs/source/sorafs_architecture_rfc.md`
и должны кросс-референситься при изменениях.

## Чеклист критериев выхода

| Веха | Критерии |
|------|----------|
| M1 | - Nightly job по fixtures зеленый семь дней подряд. <br /> - Staging alias proofs проверены в CI. <br /> - Governance ратифицирует политику expectation flags. |

## Управление изменениями

1. Предлагайте изменения через PR, обновляя этот файл **и**
   `docs/source/sorafs/migration_ledger.md`.
2. Ссылайтесь на governance minutes и CI evidence в описании PR.
3. После merge уведомить storage + DevRel mailing list с резюме и ожидаемыми
   действиями операторов.

Следование этой процедуре гарантирует, что rollout SoraFS остается детерминированным,
аудируемым и прозрачным между командами, участвующими в запуске Nexus.

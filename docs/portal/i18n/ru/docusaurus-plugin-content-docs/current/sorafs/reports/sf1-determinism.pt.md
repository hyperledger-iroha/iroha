---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
заголовок: SoraFS SF1 Детерминизм, пробный прогон
Краткое описание: Контрольный список и обзор действий для проверки или заполнения canonico `sorafs.sf1@1.0.0`.
---

# SoraFS Детерминизм SF1, пробный прогон

Это соотношение захвата или пробной базы для загрузки канонического фрагмента
`sorafs.sf1@1.0.0`. РГ по инструментарию разработает повторное выполнение или контрольный список для проверки и проверки.
обновляет оборудование или новые потребительские трубопроводы. Регистрация или результат каждого года
Команда на таблице для проведения аудита.

## Контрольный список

| Пассо | Командо | Результат успешно | Заметки |
|------|---------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | Todos os тестирует проход; или протестируйте Paridade `vectors` с успехом. | Подтвердите, что канонические настройки скомпилированы и соответствуют реализации Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | О скрипт сай ком 0; отчеты о дайджестах манифеста abaixo. | Убедитесь, что светильники восстанавливаются в неопределенном состоянии и что они постоянно теряются. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | Вход `sorafs.sf1@1.0.0` соответствует дескриптору реестра (`profile_id=1`). | Гарантия, что метаданные реестра будут синхронизированы навсегда. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | Восстановление происходит с помощью `--allow-unsigned`; архивы манифестов и ассинатура на мудам. | Принудительно проверьте детерминизм для ограничения фрагментов и манифестов. |
| 5 | `node scripts/check_sf1_vectors.mjs` | Отчеты о различных различиях между TypeScript и Rust JSON. | Помощник по желанию; гарантия безопасности во всех средах выполнения (сценарий, созданный Tooling WG). |

## Дайджесты эсперадо

- Дайджест фрагмента (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- И18НИ00000018Х: И18НИ00000019Х
- `sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## Журнал выхода из системы

| Данные | Инженер | Контрольный список результатов | Заметки |
|------|----------|------------------------|-------|
| 12.02.2026 | Оснастка (LLM) | ОК | Фикстуры регенерируются через `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102...1f`, создаются дескрипторы canonico + списки псевдонимов и новый дайджест манифеста `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. Проверено с `cargo test -p sorafs_chunker` и `ci/check_sorafs_fixtures.sh` в подвешенном состоянии (приборы подготовлены для проверки). Passo 5 pendente съел o helper de paridade Node chegar. |
| 20 февраля 2026 г. | Инструменты для хранения данных CI | ОК | Конверт парламента (`fixtures/sorafs_chunker/manifest_signatures.json`) доступен через `ci/check_sorafs_fixtures.sh`; o скрипт регенерирует фикстуры, подтверждает дайджест манифеста `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21` и повторно выполняет o Rust-жгут (passos Go/Node executam quando disponiveis) с учетом различий. |

РГ по инструментам разработала дополнительные данные для выполнения или контрольного списка. Se водоросль
Passo Falhar, Abra um Issue ligado aqui e inclua detalhes de Remediation Antes
одобрение новых светильников или перфисов.
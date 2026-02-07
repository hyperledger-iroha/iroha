---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
заголовок: SoraFS SF1 Детерминизм, пробный прогон
Краткое описание: Контрольный список и дайджесты для проверки канонического профиля `sorafs.sf1@1.0.0`.
---

# SoraFS Детерминизм SF1, пробный прогон

Ce взаимопонимание захватывает основу для пробного прогона для канонического профиля
`sorafs.sf1@1.0.0`. Tooling WG doit relancer le checklist ci-dessous lors de la
проверка обновлений светильников или новых трубопроводов потребителей.
Отправьте результат командной работы в таблицу для дальнейшего обслуживания.
трассировка проверяемая.

## Контрольный список

| Этап | Командир | Результат посещения | Заметки |
|------|---------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | Все тесты пройдены; le test de parité `vectors` повторно. | Подтвердите, что канонические светильники скомпилированы и соответствуют реализации Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | Сортировка сценария в 0 ; раппорт ле дайджест де манифест ци-десус. | Убедитесь, что светильники правильно настроены и что подписи остались у атташе. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | Вход в `sorafs.sf1@1.0.0` соответствует дескриптору реестра (`profile_id=1`). | Убедитесь, что метаданные реестра синхронизированы. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | Регенерация без `--allow-unsigned` ; фичи манифеста и подписи остаются неизменными. | Fournit une preuve de determinisme для ограничений фрагмента и проявлений. |
| 5 | `node scripts/check_sf1_vectors.mjs` | Не существует различий между приспособлениями TypeScript и JSON Rust. | Вспомогательная опция ; Гарантия равенства между средами выполнения (сценарий, поддерживаемый Tooling WG). |

## Посетитель дайджестов

- Дайджест фрагмента (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- И18НИ00000018Х: И18НИ00000019Х
- `sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## Журнал подписания

| Дата | Инженер | Результат контрольного списка | Заметки |
|------|----------|-----------------------|-------|
| 12.02.2026 | Оснастка (LLM) | ✅ Реусси | Светильники создаются через `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f`, создавая канонический список + псевдонимы и манифест дайджеста от `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. Проверьте наличие `cargo test -p sorafs_chunker` и `ci/check_sorafs_fixtures.sh` (этапы для проверки). На 5-м этапе прибудет помощник по паритеному узлу. |
| 20 февраля 2026 г. | Инструменты для хранения данных CI | ✅ Реусси | Конверт парламента (`fixtures/sorafs_chunker/manifest_signatures.json`) восстановлен через `ci/check_sorafs_fixtures.sh` ; сценарий регенерирует фикстуры, подтверждает дайджест манифеста `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21` и восстанавливает обвязку Rust (этапы Go/Node s'exécutent quand disponibles) без различий. |

WG по инструментарию добавляется к дате после выполнения контрольного списка. Si une
étape échoue, оставьте эту проблему и включите детали исправления
авангардное одобрение новых светильников или профилей.
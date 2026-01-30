---
lang: ja
direction: ltr
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/sorafs/reports/sf1-determinism.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3aa98b59ecab60f874425bc4a1d9a4e222cc423ff5c8abc4962c6c294e2780db
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Russian (ru) translation. Replace this content with the full translation. -->

---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# SoraFS SF1 Determinism Dry-Run

Этот отчет фиксирует базовый dry-run для канонического профиля chunker
`sorafs.sf1@1.0.0`. Tooling WG должна повторять чеклист ниже при проверке
обновлений fixtures или новых consumer pipelines. Записывайте результат каждой
команды в таблицу, чтобы сохранить auditable trail.

## Чеклист

| Шаг | Команда | Ожидаемый результат | Примечания |
|------|---------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | Все тесты проходят; `vectors` parity test успешен. | Подтверждает, что канонические fixtures компилируются и совпадают с Rust реализацией. |
| 2 | `ci/check_sorafs_fixtures.sh` | Скрипт завершается 0; сообщает digests manifests ниже. | Проверяет, что fixtures регенерируются чисто и подписи остаются прикреплены. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | Запись для `sorafs.sf1@1.0.0` совпадает с registry descriptor (`profile_id=1`). | Гарантирует, что metadata registry остается синхронной. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | Регенерация проходит без `--allow-unsigned`; файлы manifest и signature не меняются. | Дает доказательство детерминизма для chunk boundaries и manifests. |
| 5 | `node scripts/check_sf1_vectors.mjs` | Сообщает отсутствие diff между TypeScript fixtures и Rust JSON. | Опциональный helper; обеспечьте паритет между runtime (script поддерживает Tooling WG). |

## Ожидаемые digests

- Chunk digest (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## Sign-Off Log

| Дата | Engineer | Результат чеклиста | Примечания |
|------|----------|--------------------|-------|
| 2026-02-12 | Tooling (LLM) | ✅ Passed | Fixtures регенерированы через `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f`, с каноническим handle + списком aliases и новым manifest digest `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. Проверено `cargo test -p sorafs_chunker` и чистым прогоном `ci/check_sorafs_fixtures.sh` (fixtures подготовлены для проверки). Шаг 5 ожидается до появления Node parity helper. |
| 2026-02-20 | Storage Tooling CI | ✅ Passed | Parliament envelope (`fixtures/sorafs_chunker/manifest_signatures.json`) получен через `ci/check_sorafs_fixtures.sh`; скрипт регенерировал fixtures, подтвердил manifest digest `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`, и повторно запустил Rust harness (шаги Go/Node выполняются при наличии) без diffs. |

Tooling WG должна добавить датированную строку после выполнения чеклиста. Если
какой-либо шаг падает, заведите issue со ссылкой здесь и добавьте детали
remediation до утверждения новых fixtures или профилей.

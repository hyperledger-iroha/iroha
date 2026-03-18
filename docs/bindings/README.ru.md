---
lang: ru
direction: ltr
source: docs/bindings/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb91ce03aee552c65d15ed1c019da4b3b3db9d48d299b3374ca78b4a8c6c1781
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# Управление SDK-биндингами и фикстурами

WP1-E в дорожной карте указывает “docs/bindings” как каноничное место для
ведения статуса межъязыковых биндингов. Этот документ фиксирует инвентарь
биндингов, команды регенерации, защиту от дрейфа и места хранения доказательств,
чтобы ворота GPU-паритета (WP1-E/F/G) и совет по меж-SDK каденции имели единый
источник истины.

## Общие защитные меры
- **Канонический плейбук:** `docs/source/norito_binding_regen_playbook.md` описывает
  политику ротации, ожидаемые доказательства и эскалационный процесс для Android,
  Swift, Python и будущих биндингов.
- **Паритет схем Norito:** `scripts/check_norito_bindings_sync.py` (запускается через
  `scripts/check_norito_bindings_sync.sh` и блокируется в CI скриптом
  `ci/check_norito_bindings_sync.sh`) останавливает сборки при дрейфе артефактов
  схем Rust, Java или Python.
- **Наблюдатель каденции:** `scripts/check_fixture_cadence.py` читает файлы
  `artifacts/*_fixture_regen_state.json` и применяет окна Вт/Пт (Android, Python)
  и Ср (Swift), чтобы ворота дорожной карты имели аудируемые временные отметки.

## Матрица биндингов

| Биндинг | Точки входа | Команда фикстур/регенерации | Защита от дрейфа | Доказательства |
|---------|-------------|------------------------------|------------------|----------------|
| Android (Java) | `java/iroha_android/` (`java/iroha_android/README.md`) | `scripts/android_fixture_regen.sh` → `artifacts/android_fixture_regen_state.json` | `scripts/check_android_fixtures.py`, `ci/check_android_fixtures.sh`, `java/iroha_android/run_tests.sh` | `artifacts/android/fixture_runs/` |
| Swift (iOS/macOS) | `IrohaSwift/` (`IrohaSwift/README.md`) | `scripts/swift_fixture_regen.sh` (опционально `SWIFT_FIXTURE_ARCHIVE`) → `artifacts/swift_fixture_regen_state.json` | `scripts/check_swift_fixtures.py`, `ci/check_swift_fixtures.sh`, `scripts/swift_fixture_archive.py` | `docs/source/swift_parity_triage.md`, `docs/source/sdk/swift/ios2_fixture_cadence_brief.md` |
| Python | `python/iroha_python/` (`python/iroha_python/README.md`) | `scripts/python_fixture_regen.sh` → `artifacts/python_fixture_regen_state.json` | `scripts/check_python_fixtures.py`, `python/iroha_python/scripts/run_checks.sh` | `docs/source/norito_binding_regen_playbook.md`, `docs/source/sdk/python/connect_end_to_end.md` |
| JavaScript | `javascript/iroha_js/` (`docs/source/sdk/js/publishing.md`) | `npm run release:provenance`, `scripts/js_sbom_provenance.sh`, `scripts/js_signed_staging.sh` | `npm run test`, `javascript/iroha_js/scripts/verify-release-tarball.mjs`, `javascript/iroha_js/scripts/record-release-provenance.mjs` | `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`, `artifacts/js/verification/`, `artifacts/js/sbom/` |

## Детали по биндингам

### Android (Java)
Android SDK находится в `java/iroha_android/` и использует канонические фикстуры
Norito, сгенерированные `scripts/android_fixture_regen.sh`. Этот helper экспортирует
свежие `.norito` блобы из Rust-тулчейна, обновляет
`artifacts/android_fixture_regen_state.json` и фиксирует метаданные каденции,
которые используют `scripts/check_fixture_cadence.py` и дашборды управления.
Дрейф обнаруживается `scripts/check_android_fixtures.py` (также подключен к
`ci/check_android_fixtures.sh`) и `java/iroha_android/run_tests.sh`, который
проверяет JNI-биндинги, воспроизведение очереди WorkManager и fallback'и StrongBox.
Доказательства ротаций, заметки о сбоях и протоколы повторных запусков находятся
в `artifacts/android/fixture_runs/`.

### Swift (macOS/iOS)
`IrohaSwift/` отражает те же payloads Norito через `scripts/swift_fixture_regen.sh`.
Скрипт фиксирует владельца ротации, метку каденции и источник (`live` vs `archive`)
в `artifacts/swift_fixture_regen_state.json` и передает данные в cadence checker.
`scripts/swift_fixture_archive.py` позволяет мейнтейнерам импортировать архивы,
сгенерированные Rust; `scripts/check_swift_fixtures.py` и `ci/check_swift_fixtures.sh`
обеспечивают побайтовый паритет и SLA-ограничения по возрасту, а
`scripts/swift_fixture_regen.sh` поддерживает `SWIFT_FIXTURE_EVENT_TRIGGER` для
ручных ротаций. Эскалационный процесс, KPI и дашборды описаны в
`docs/source/swift_parity_triage.md` и брифах каденции под `docs/source/sdk/swift/`.

### Python
Python-клиент (`python/iroha_python/`) использует фикстуры Android. Запуск
`scripts/python_fixture_regen.sh` подтягивает последние `.norito` payloads,
обновляет `python/iroha_python/tests/fixtures/` и будет писать метаданные каденции
в `artifacts/python_fixture_regen_state.json` после первой ротации по дорожной карте.
`scripts/check_python_fixtures.py` и `python/iroha_python/scripts/run_checks.sh`
блокируют pytest, mypy, ruff и паритет фикстур локально и в CI. Документация
end-to-end (`docs/source/sdk/python/…`) и плейбук регенерации описывают, как
координировать ротации с владельцами Android.

### JavaScript
`javascript/iroha_js/` не зависит от локальных `.norito` файлов, но WP1-E отслеживает
доказательства релизов, чтобы GPU CI lanes наследовали полную provenance. Каждый
релиз фиксирует provenance через `npm run release:provenance` (на базе
`javascript/iroha_js/scripts/record-release-provenance.mjs`), генерирует и
подписывает SBOM-бандлы через `scripts/js_sbom_provenance.sh`, выполняет подписанный
staging (`scripts/js_signed_staging.sh`) и проверяет артефакт registry через
`javascript/iroha_js/scripts/verify-release-tarball.mjs`. Итоговые метаданные
попадают в `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`,
`artifacts/js/sbom/` и `artifacts/js/verification/`, обеспечивая детерминированные
доказательства для запусков roadmap JS5/JS6 и WP1-F. Плейбук публикации в
`docs/source/sdk/js/` связывает всю автоматизацию воедино.

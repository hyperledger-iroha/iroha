---
lang: ru
direction: ltr
source: docs/automation/android/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 27b5ac3c7adb19a87f0b3d076f3c9618b188602898ed3954808ac9f7a52b3a62
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# Бейзлайн автоматизации документации Android (AND5)

Пункт AND5 дорожной карты требует, чтобы автоматизация документации,
локализации и публикации была аудируемой до старта AND6 (CI & Compliance).
Эта папка фиксирует команды, артефакты и структуру доказательств, на которые
ссылаются AND5/AND6, отражая планы в
`docs/source/sdk/android/developer_experience_plan.md` и
`docs/source/sdk/android/parity_dashboard_plan.md`.

## Пайплайны и команды

| Задача | Команда(ы) | Ожидаемые артефакты | Примечания |
|--------|------------|---------------------|------------|
| Синхронизация i18n-заглушек | `python3 scripts/sync_docs_i18n.py` (опционально `--lang <code>` на запуск) | Лог в `docs/automation/android/i18n/<timestamp>-sync.log` плюс коммиты переведенных заглушек | Поддерживает `docs/i18n/manifest.json` в актуальном состоянии; лог фиксирует коды языков и коммит, вошедший в бейзлайн. |
| Проверка фикстур + паритет Norito | `ci/check_android_fixtures.sh` (оборачивает `python3 scripts/check_android_fixtures.py --json-out artifacts/android/parity/<stamp>/summary.json`) | Скопировать итоговый JSON в `docs/automation/android/parity/<stamp>-summary.json` | Проверяет payloads из `java/iroha_android/src/test/resources`, хеши манифестов и длины подписанных фикстур. Приложите сводку вместе с доказательствами каденции в `artifacts/android/fixture_runs/`. |
| Манифест образцов и подтверждение публикации | `scripts/publish_android_sdk.sh --version <semver> [--repo-url …]` (запускает тесты + SBOM + provenance) | Метаданные provenance-бандла и `sample_manifest.json` из `docs/source/sdk/android/samples/`, сохраненные в `docs/automation/android/samples/<version>/` | Связывает примерные приложения AND5 с автоматизацией релизов: сохраните сгенерированный манифест, хеш SBOM и лог provenance для бета-ревью. |
| Фид дашборда паритета | `python3 scripts/check_android_fixtures.py … --json-out artifacts/android/parity/<stamp>/summary.json` затем `python3 scripts/android_parity_metrics.py --summary <summary> --output artifacts/android/parity/<stamp>/metrics.prom` | Скопировать snapshot `metrics.prom` или JSON-экспорт Grafana в `docs/automation/android/parity/<stamp>-metrics.prom` | Питает план дашборда, чтобы AND5/AND7 могли проверять счетчики недопустимых отправок и внедрение телеметрии. |

## Фиксация доказательств

1. **Все отмечайте временем.** Именуйте файлы по UTC-штампу
   (`YYYYMMDDTHHMMSSZ`), чтобы дашборды паритета, протоколы управления и
   опубликованные документы ссылались на одну и ту же сессию.
2. **Указывайте коммиты.** Каждый лог должен включать хеш коммита запуска и
   релевантную конфигурацию (например, `ANDROID_PARITY_PIPELINE_METADATA`). При
   необходимости редактирования из-за приватности добавьте заметку и ссылку на
   защищенное хранилище.
3. **Архивируйте минимум контекста.** В git попадают только структурированные
   сводки (JSON, `.prom`, `.log`). Тяжелые артефакты (APK-бандлы, скриншоты)
   остаются в `artifacts/` или объектном хранилище с подписанным хешем в логе.
4. **Обновляйте статус.** Когда этапы AND5 продвигаются в `status.md`, указывайте
   соответствующий файл (например,
   `docs/automation/android/parity/20260324T010203Z-summary.json`), чтобы аудиторы
   могли проверить бейзлайн без просмотра CI-логов.

Следование этой структуре удовлетворяет требованию AND6 о «доступных для аудита
бейзлайнах docs/automation» и удерживает Android-документацию в синхронизации с
опубликованными планами.

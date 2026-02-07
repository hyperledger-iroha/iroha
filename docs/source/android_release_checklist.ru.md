---
lang: ru
direction: ltr
source: docs/source/android_release_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ee3613b544a847953f5ec152092cb2fe1da35279c5482486513d6b8d6dddf02
source_last_modified: "2026-01-04T11:42:43.398592+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Контрольный список выпуска Android (AND6)

В этом контрольном списке отражены элементы **AND6 — CI & Compliance Hardening** из
`roadmap.md` (§Приоритет 5). Он согласовывает выпуски Android SDK с Rust.
оправдать ожидания RFC, описав задания CI, артефакты соответствия,
доказательства, подтверждающие устройство, и пакеты происхождения, которые должны быть прикреплены до GA,
LTS, или поезд исправлений, движется вперед.

Используйте этот документ вместе с:

- `docs/source/android_support_playbook.md` — календарь выпусков, соглашения об уровне обслуживания и
  дерево эскалации.
- `docs/source/android_runbook.md` — инструкции по повседневной работе.
- `docs/source/compliance/android/and6_compliance_checklist.md` — регулятор
  инвентарь артефактов.
- `docs/source/release_dual_track_runbook.md` — двухканальное управление выпуском.

## 1. Краткий обзор ворот сцены

| Этап | Требуются ворота | Доказательства |
|-------|----------------|----------|
| **Т-7 дней (до замораживания)** | Ночной `ci/run_android_tests.sh` зеленый на 14 дней; `ci/check_android_fixtures.sh`, `ci/check_android_samples.sh` и `ci/check_android_docs_i18n.sh` проходят; Сканирование lint/зависимостей поставлено в очередь. | Панели мониторинга Buildkite, отчет о различиях в приборах, примеры снимков экрана. |
| **Т-3 дня (акция RC)** | Резервирование устройства-лаборатории подтверждено; Аттестационный запуск StrongBox CI (`scripts/android_strongbox_attestation_ci.sh`); Робоэлектрические/инструментальные комплекты, работающие на запланированном оборудовании; `./gradlew lintRelease ktlintCheck detekt dependencyGuard` чистый. | Матрица устройств в формате CSV, манифест пакета аттестации, отчеты Gradle, заархивированные под `artifacts/android/lint/<version>/`. |
| **Т-1 день (годен/не годен)** | Обновлен пакет статуса редактирования телеметрии (`scripts/telemetry/check_redaction_status.py --write-cache`); артефакты соответствия обновлены согласно `and6_compliance_checklist.md`; Репетиция происхождения завершена (`scripts/android_sbom_provenance.sh --dry-run`). | `docs/source/compliance/android/evidence_log.csv`, статус телеметрии в формате JSON, журнал пробных прогонов происхождения. |
| **T0 (переключение GA/LTS)** | `scripts/publish_android_sdk.sh --dry-run` завершено; провенанс + подпись СБОМ; контрольный список выпуска экспортируется и прикрепляется к минутам «годен/не годен»; `ci/sdk_sorafs_orchestrator.sh` дымит зеленым цветом. | Выпуск вложений RFC, пакета Sigstore, артефактов внедрения под `artifacts/android/`. |
| **Т+1 день (после перехода)** | Готовность исправления подтверждена (`scripts/publish_android_sdk.sh --validate-bundle`); рассмотрены различия приборной панели (`ci/check_android_dashboard_parity.sh`); Пакет доказательств загружен на `status.md`. | Экспорт различий информационной панели, ссылка на запись `status.md`, архивный пакет выпуска. |

## 2. Матрица CI и качества| Ворота | Команда(ы) / Сценарий | Заметки |
|------|--------------------|-------|
| Модульные + интеграционные тесты | `ci/run_android_tests.sh` (обертывает `ci/run_android_tests.sh`) | Выдает `artifacts/android/tests/test-summary.json` + журнал испытаний. Включает кодек Norito, очередь, резервный вариант StrongBox и тесты подключения клиента Torii. Требуется каждую ночь и перед маркировкой. |
| Светильник паритет | `ci/check_android_fixtures.sh` (обертывает `scripts/check_android_fixtures.py`) | Гарантирует, что восстановленные фикстуры Norito соответствуют каноническому набору Rust; прикрепите разницу JSON в случае сбоя шлюза. |
| Примеры приложений | `ci/check_android_samples.sh` | Создает `examples/android/{operator-console,retail-wallet}` и проверяет локализованные снимки экрана через `scripts/android_sample_localization.py`. |
| Документы/I18N | `ci/check_android_docs_i18n.sh` | Guards README + локализованные краткие руководства. Запустите еще раз после того, как изменения документа попадут в ветку выпуска. |
| Паритет приборной панели | `ci/check_android_dashboard_parity.sh` | Подтверждает соответствие CI/экспортированных метрик аналогам в Rust; требуется во время проверки Т+1. |
| SDK принятие дыма | `ci/sdk_sorafs_orchestrator.sh` | Осуществляет привязки оркестратора Sorafs с несколькими источниками с текущим SDK. Требуется перед загрузкой поэтапных артефактов. |
| Проверка аттестации | `scripts/android_strongbox_attestation_ci.sh --summary-out artifacts/android/attestation/ci-summary.json` | Объединяет пакеты аттестации StrongBox/TEE под `artifacts/android/attestation/**`; прикрепите сводку к пакетам GA. |
| Проверка слота в лаборатории устройства | `scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab/<slot> --json-out artifacts/android/device_lab/summary.json` | Проверяет комплекты инструментов перед присоединением доказательств к пакетам выпуска; CI выполняется с использованием слота образца в `fixtures/android/device_lab/slot-sample` (телеметрия/аттестация/очередь/журналы + `sha256sum.txt`). |

> **Совет.** добавьте эти задания в конвейер `android-release` Buildkite, чтобы
> Недели замораживания автоматически перезапускают каждые ворота с помощью кончика ветки выпуска.

Консолидированное задание `.github/workflows/android-and6.yml` запускает проверку,
проверка набора тестов, сводки аттестации и слотов лаборатории устройств при каждом запросе запроса/публикации
касаемся исходников Android, загружаем доказательства под `artifacts/android/{lint,tests,attestation,device_lab}/`.

## 3. Сканирование ворса и зависимостей

Запустите `scripts/android_lint_checks.sh --version <semver>` из корня репо.
скрипт выполняет:

```
lintRelease ktlintCheck detekt dependencyGuardBaseline \
:operator-console:lintRelease :retail-wallet:lintRelease
```

- Отчеты и выходные данные защиты зависимостей архивируются в папке
  `artifacts/android/lint/<label>/` и символическая ссылка `latest/` для выпуска
  трубопроводы.
- Неудачные выводы требуют либо исправления, либо внесения записи в релиз.
  RFC, документирующий принятый риск (утвержден Release Engineering + Programme).
  Ведущий).
- `dependencyGuardBaseline` восстанавливает блокировку зависимостей; прикрепи разницу
  к пакету «годен/не годен».

## 4. Лаборатория устройств и покрытие StrongBox

1. Зарезервируйте устройства Pixel + Galaxy, используя трекер емкости, указанный в
   `docs/source/compliance/android/device_lab_contingency.md`. Блокирует выпуски
   если доступность `, чтобы обновить отчет об аттестации.
3. Запустите матрицу инструментирования (задокументируйте список комплектов/ABI в устройстве).
   трекер). Фиксируйте сбои в журнале инцидентов, даже если повторные попытки успешны.
4. Подайте заявку, если требуется возврат к Firebase Test Lab; связать билет
   в контрольном списке ниже.

## 5. Соответствие требованиям и артефакты телеметрии- Следуйте `docs/source/compliance/android/and6_compliance_checklist.md` для ЕС.
  и материалы JP. Обновление `docs/source/compliance/android/evidence_log.csv`
  с хэшами + URL-адресами заданий Buildkite.
- Обновить доказательства редактирования телеметрии через
  `scripts/telemetry/check_redaction_status.py --write-cache \
   --status-url https://android-observability.example/status.json`.
  Сохраните полученный JSON под
  `artifacts/android/telemetry/<version>/status.json`.
- Запишите вывод разницы схемы из
  `scripts/telemetry/run_schema_diff.sh --android-config ... --rust-config ...`
  доказать паритет с экспортерами Rust.

## 6. Происхождение, SBOM и издательство

1. Пробный запуск конвейера публикации:

   ```bash
   scripts/publish_android_sdk.sh \
     --version <semver> \
     --repo-dir artifacts/android/maven/<semver> \
     --dry-run
   ```

2. Сгенерируйте SBOM + Sigstore происхождение:

   ```bash
   scripts/android_sbom_provenance.sh \
     --version <semver> \
     --out artifacts/android/provenance/<semver>
   ```

3. Прикрепляем `artifacts/android/provenance/<semver>/manifest.json` и подписываем
   `checksums.sha256` к выпуску RFC.
4. При переходе в настоящий репозиторий Maven перезапустите
   `scripts/publish_android_sdk.sh` без `--dry-run`, захватить консоль
   log и загрузите полученные артефакты в `artifacts/android/maven/<semver>`.

## 7. Шаблон пакета отправки

Каждый выпуск GA/LTS/исправления должен включать:

1. **Завершенный контрольный список** — скопируйте таблицу из этого файла, отметьте каждый пункт и добавьте ссылку.
   для поддержки артефактов (запуск Buildkite, журналы, различия документов).
2. **Лабораторные данные устройства** — сводка отчета об аттестации, журнал резервирования и
   любые непредвиденные активации.
3. **Пакет телеметрии** — статус редактирования JSON, разница в схеме, ссылка на
   Обновления `docs/source/sdk/android/telemetry_redaction.md` (если есть).
4. **Артефакты соответствия** — записи, добавленные/обновленные в папке соответствия.
   плюс обновленный журнал доказательств в формате CSV.
5. **Пакет Provenance** — SBOM, подпись Sigstore и `checksums.sha256`.
6. **Сводка выпуска** — одностраничный обзор, прикрепленный к сводному документу `status.md`.
   вышеуказанное (дата, версия, выделение любых отмененных шлюзов).

Сохраните пакет под именем `artifacts/android/releases/<version>/` и укажите на него ссылку.
в `status.md` и выпуске RFC.

- `scripts/run_release_pipeline.py --publish-android-sdk ...` автоматически
  копирует последний архив lint (`artifacts/android/lint/latest`) и
  доказательства соответствия войдите в `artifacts/android/releases/<version>/`, чтобы
  пакет отправки всегда имеет каноническое местоположение.

---

**Напоминание:** обновляйте этот контрольный список при появлении новых заданий CI, артефактов соответствия,
или добавляются требования к телеметрии. Пункт дорожной карты AND6 остается открытым до тех пор, пока
контрольный список и связанная с ним автоматизация остаются стабильными в течение двух выпусков подряд
поезда.
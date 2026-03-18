---
lang: ru
direction: ltr
source: docs/source/compliance/android/device_lab_instrumentation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9d384e21d09f3c4f57b7fc5181d69dc0da739dd6ed4dcb89a57ea58fd29bb898
source_last_modified: "2026-01-03T18:07:59.259775+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Хуки для инструментов лаборатории устройств Android (AND6)

Эта ссылка завершает действие дорожной карты «стадия оставшегося устройства-лаборатории /
инструментальные крючки перед началом AND6». Это объясняет, как каждый зарезервированный
Слот устройства-лаборатории должен захватывать телеметрию, очередь и артефакты аттестации, чтобы
Контрольный список соответствия требованиям AND6, журнал доказательств и пакеты управления используют одни и те же
детерминированный рабочий процесс. Соедините это примечание с процедурой бронирования.
(`device_lab_reservation.md`) и книгу аварийного переключения при планировании репетиций.

## Цели и объем

- **Детерминистические данные** – все результаты приборов соответствуют
  `artifacts/android/device_lab/<slot-id>/` с SHA-256 проявляется так аудиторы
  может различать пакеты без повторного запуска зондов.
- **Рабочий процесс на основе сценариев** – повторное использование существующих помощников.
  (`ci/run_android_telemetry_chaos_prep.sh`,
  И18НИ00000006Х, И18НИ00000007Х)
  вместо специальных команд adb.
- **Контрольные списки остаются синхронизированными**: каждый запуск ссылается на этот документ из
  Контрольный список соответствия AND6 и добавляет артефакты к
  `docs/source/compliance/android/evidence_log.csv`.

## Макет артефакта

1. Выберите уникальный идентификатор слота, соответствующий билету бронирования, например.
   `2026-05-12-slot-a`.
2. Заполните стандартные каталоги:

   ```bash
   export ANDROID_DEVICE_LAB_SLOT=2026-05-12-slot-a
   export ANDROID_DEVICE_LAB_ROOT="artifacts/android/device_lab/${ANDROID_DEVICE_LAB_SLOT}"
   mkdir -p "${ANDROID_DEVICE_LAB_ROOT}"/{telemetry,attestation,queue,logs}
   ```

3. Сохраните каждый журнал команд в соответствующей папке (например,
   `telemetry/status.ndjson`, `attestation/pixel8pro.log`).
4. Захват SHA-256 проявляется после закрытия слота:

   ```bash
   find "${ANDROID_DEVICE_LAB_ROOT}" -type f -print0 | sort -z \
     | xargs -0 shasum -a 256 > "${ANDROID_DEVICE_LAB_ROOT}/sha256sum.txt"
   ```

## Матрица инструментов

| Поток | Команда(ы) | Расположение выхода | Заметки |
|------|------------|-----------------|-------|
| Редактирование телеметрии + пакет статуса | `scripts/telemetry/check_redaction_status.py --status-url <collector> --json-out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/status.ndjson` | `telemetry/status.ndjson`, `telemetry/status.log` | Запуск в начале и конце слота; прикрепите стандартный вывод CLI к `status.log`. |
| Ожидающая очередь + подготовка к хаосу | `ANDROID_PENDING_QUEUE_EXPORTS="pixel8=${ANDROID_DEVICE_LAB_ROOT}/queue/pixel8.bin" ci/run_android_telemetry_chaos_prep.sh --status-only` | `queue/*.bin`, `queue/*.json`, `queue/*.sha256` | Зеркала ScenarioD от `readiness/labs/telemetry_lab_01.md`; расширьте переменную env для каждого устройства в слоте. |
| Переопределить дайджест бухгалтерской книги | `scripts/android_override_tool.sh digest --out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/override_digest.json` | `telemetry/override_digest.json` | Требуется, даже если никакие переопределения не активны; доказать нулевое состояние. |
| Аттестация StrongBox/TEE | `scripts/android_keystore_attestation.sh --device pixel8pro-strongbox-a --out "${ANDROID_DEVICE_LAB_ROOT}/attestation/pixel8pro"` | `attestation/<device>/*.{json,zip,log}` | Повторите эти действия для каждого зарезервированного устройства (соответствуйте именам в `android_strongbox_device_matrix.md`). |
| Регрессия аттестации CI | `scripts/android_strongbox_attestation_ci.sh --output "${ANDROID_DEVICE_LAB_ROOT}/attestation/ci"` | `attestation/ci/*` | Собирает те же доказательства, которые загружает CI; включить в ручные прогоны для симметрии. |
| Базовый уровень Lint/зависимостей | `ANDROID_LINT_SUMMARY_OUT="${ANDROID_DEVICE_LAB_ROOT}/logs/jdeps-summary.txt" make android-lint` | `logs/jdeps-summary.txt`, `logs/lint.log` | Запускать один раз за окно заморозки; цитируйте резюме в пакетах соответствия. |

## Стандартная процедура слота1. **Предполетная подготовка (T-24h)** – подтвердите, что это указано в билете на бронирование.
   документ, обновите запись матрицы устройств и заполните корень артефакта.
2. **Во время слота**
   - Сначала запустите пакет телеметрии + команды экспорта очереди. Пройти
     `--note <ticket>` — `ci/run_android_telemetry_chaos_prep.sh`, поэтому журнал
     ссылается на идентификатор инцидента.
   - Запуск сценариев аттестации для каждого устройства. Когда жгут производит
     `.zip`, скопируйте его в корень артефакта и запишите Git SHA, напечатанный по адресу
     конец сценария.
   - Выполните `make android-lint` с переопределенным суммарным путем, даже если CI
     уже побежал; аудиторы ожидают журнал для каждого слота.
3. **После запуска**
   - Сгенерируйте `sha256sum.txt` и `README.md` (заметки произвольной формы) внутри слота.
     папка со сводкой выполненных команд.
   - Добавьте строку к `docs/source/compliance/android/evidence_log.csv` с помощью
     идентификатор слота, путь хеш-манифеста, ссылки на Buildkite (если есть) и последнюю версию
     Процент мощности лаборатории устройств из экспорта календаря резервирования.
   - Свяжите папку слота в билете `_android-device-lab`, AND6.
     контрольный список и отчет о выпуске `docs/source/android_support_playbook.md`.

## Обработка и эскалация сбоев

- Если какая-либо команда завершится неудачно, запишите выходные данные stderr под `logs/` и следуйте инструкциям.
  лестница эскалации в `device_lab_reservation.md` §6.
- При недостатке очереди или телеметрии следует немедленно отметить статус переопределения в
  `docs/source/sdk/android/telemetry_override_log.md` и укажите идентификатор слота.
  чтобы руководство могло отслеживать ход учений.
- Регрессии аттестации должны регистрироваться в
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  с неисправными серийными номерами устройств и путями пакетов, записанными выше.

## Контрольный список отчетности

Прежде чем пометить слот как завершенный, убедитесь, что обновлены следующие ссылки:

- `docs/source/compliance/android/and6_compliance_checklist.md` — отметьте
  строка инструментов завершена и запишите идентификатор слота.
- `docs/source/compliance/android/evidence_log.csv` — добавить/обновить запись с помощью
  хэш слота и чтение емкости.
- Билет `_android-device-lab` — прикрепите ссылки на артефакты и идентификаторы заданий Buildkite.
- `status.md` — включите краткое примечание в следующий дайджест готовности Android, чтобы
  Читатели дорожной карты знают, какой слот предоставил последние доказательства.

Следуя этому процессу, AND6 сохраняет «привязки устройств-лабораторий + инструментов».
контрольные этапы проверяются и предотвращают расхождения вручную между резервированием, исполнением,
и отчетность.
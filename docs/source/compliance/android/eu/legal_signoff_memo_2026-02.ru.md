---
lang: ru
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo_2026-02.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eb92b77765ced36213a0bde55581f29d59c262f398c658f35a1fb43a182fe296
source_last_modified: "2026-01-03T18:07:59.201100+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 Юридическая записка ЕС — 2026.1 GA (Android SDK)

## Резюме

- **Выпуск/обработка:** 2026.1 GA (Android SDK)
- **Дата проверки:** 15 апреля 2026 г.
- **Советник/Рецензент:** София Мартинс, отдел нормативно-правового регулирования.
- **Объем:** Целевой показатель безопасности ETSI EN 319 401, сводка GDPR DPIA, аттестация SBOM, доказательства непредвиденных обстоятельств лаборатории устройства AND6.
- **Связанные билеты:** `_android-device-lab` / AND6-DR-202602, трекер управления AND6 (`GOV-AND6-2026Q1`)

## Контрольный список артефактов

| Артефакт | ША-256 | Местоположение / Ссылка | Заметки |
|----------|---------|-----------------|-------|
| `security_target.md` | `385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0` | `docs/source/compliance/android/eu/security_target.md` | Соответствует идентификаторам выпуска общедоступной версии 2026.1 и изменениям модели угроз (дополнения Torii NRPC). |
| `gdpr_dpia_summary.md` | `8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5` | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | Ссылки на политику телеметрии AND7 (`docs/source/sdk/android/telemetry_redaction.md`). |
| `sbom_attestation.md` | `c2e0de176d4bb8c8e09329e2b9ee5dd93228d3f0def78225c1d8b777a5613f2d` | Комплект `docs/source/compliance/android/eu/sbom_attestation.md` + Sigstore (`android-sdk-release#4821`). | CycloneDX + проверено происхождение; соответствует заданию Buildkite `android-sdk-release#4821`. |
| Журнал доказательств | `0b2d2f9eddada06faa70620f608c3ad1ec38f378d2cbddc24b15d0a83fcc381d` | `docs/source/compliance/android/evidence_log.csv` (строка `android-device-lab-failover-20260220`) | Подтверждает записанные в журнале хэши пакетов + снимок емкости + запись примечания. |
| Пакет действий на случай непредвиденных обстоятельств «устройство-лаборатория» | `faf32356dfc0bbca1459b14d75f3306ea1c10cb40f3180fe1758ac5105016f85` | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | Хэш взят из `bundle-manifest.json`; в билете AND6-DR-202602 записана передача в юридический отдел/соответственный отдел. |

## Выводы и исключения

- Проблем с блокировкой не выявлено. Артефакты соответствуют требованиям ETSI/GDPR; Четность телеметрии AND7 отмечена в сводке DPIA, и никаких дополнительных мер по снижению не требуется.
- Рекомендация: отслеживать запланированное обучение DR-2026-05-Q2 (заявка AND6-DR-202605) и добавлять полученный пакет в журнал доказательств перед следующей контрольной точкой управления.

## Одобрение

- **Решение:** одобрено.
- **Подпись/временная метка:** _София Мартинс (цифровая подпись через портал управления, 15 апреля 2026 г., 14:32 UTC)_
- **Последующие владельцы:** Отдел эксплуатации лаборатории устройств (доставить пакет доказательств DR-2026-05-Q2 до 31 мая 2026 г.).
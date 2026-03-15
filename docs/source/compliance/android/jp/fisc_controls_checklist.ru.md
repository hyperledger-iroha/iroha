---
lang: ru
direction: ltr
source: docs/source/compliance/android/jp/fisc_controls_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2d8b4c90c94dddd8118fcb9c55f07c25000c6dab1f8d239570402023ab89e844
source_last_modified: "2026-01-03T18:07:59.237724+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Контрольный список мер безопасности FISC — Android SDK

| Поле | Значение |
|-------|-------|
| Версия | 0,1 (12 февраля 2026 г.) |
| Область применения | Android SDK + инструменты оператора, используемые в финансовых проектах в Японии |
| Владельцы | Комплаенс и юридические вопросы (Дэниел Парк), руководитель программы Android |

## Управляющая матрица

| ФИСК Контроль | Детали реализации | Доказательства/ссылки | Статус |
|--------------|-----------------------|-----------------------|--------|
| **Целостность конфигурации системы** | `ClientConfig` обеспечивает хеширование манифеста, проверку схемы и доступ во время выполнения только для чтения. При сбоях перезагрузки конфигурации выдаются события `android.telemetry.config.reload`, описанные в модуле Runbook. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2. | ✅ Реализовано |
| **Контроль доступа и аутентификация** | SDK учитывает политики TLS Torii и подписанные запросы `/v1/pipeline`; Справочник по рабочим процессам оператора. Руководство по поддержке §4–5 для эскалации и отмены шлюзования с помощью подписанных артефактов Norito. | `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_redaction.md` (переопределить рабочий процесс). | ✅ Реализовано |
| **Управление криптографическими ключами** | Поставщики, предпочитаемые StrongBox, проверка аттестации и покрытие матрицы устройств обеспечивают соответствие KMS. Результаты аттестации архивируются под номером `artifacts/android/attestation/` и отслеживаются в матрице готовности. | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`. | ✅ Реализовано |
| **Журналирование, мониторинг и хранение** | Политика редактирования телеметрии хэширует конфиденциальные данные, группирует атрибуты устройств и обеспечивает принудительное хранение (7/30/90/365-дневные окна). В §8 руководства по поддержке описаны пороговые значения информационной панели; переопределения записаны в `telemetry_override_log.md`. | `docs/source/sdk/android/telemetry_redaction.md`; `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_override_log.md`. | ✅ Реализовано |
| **Операции и управление изменениями** | Процедура переключения общедоступной версии (Справочник поддержки §7.2) и обновления `status.md` отслеживают готовность к выпуску. Доказательства выпуска (SBOM, пакеты Sigstore), связанные через `docs/source/compliance/android/eu/sbom_attestation.md`. | `docs/source/android_support_playbook.md`; `status.md`; `docs/source/compliance/android/eu/sbom_attestation.md`. | ✅ Реализовано |
| **Реагирование на инциденты и отчеты** | Playbook определяет матрицу серьезности, окна ответа SLA и шаги уведомления о соответствии; отмена телеметрии + репетиции хаоса обеспечивают воспроизводимость перед пилотами. | `docs/source/android_support_playbook.md` §§4–9; `docs/source/sdk/android/telemetry_chaos_checklist.md`. | ✅ Реализовано |
| **Размещение/локализация данных** | Сборщики телеметрии для развертываний JP работают в утвержденном регионе Токио; Пакеты аттестации StrongBox хранятся в регионе и на них ссылаются партнерские заявки. План локализации гарантирует, что документы будут доступны на японском языке до бета-тестирования (AND5). | `docs/source/android_support_playbook.md` §9; `docs/source/sdk/android/developer_experience_plan.md` §5; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`. | 🈺 В процессе (идет локализация) |

## Примечания рецензента

- Проверьте записи матрицы устройств для Galaxy S23/S24 перед подключением регулируемого партнера (см. строки документации о готовности `s23-strongbox-a`, `s24-strongbox-a`).
— Убедитесь, что сборщики телеметрии в развертываниях JP применяют ту же логику хранения/переопределения, определенную в DPIA (`docs/source/compliance/android/eu/gdpr_dpia_summary.md`).
- Получите подтверждение от внешних аудиторов после того, как банковские партнеры ознакомятся с этим контрольным списком.
---
lang: ru
direction: ltr
source: docs/source/compliance/android/jp/strongbox_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b8cc2e9de0c4183b51d011f5106a62b212da620d628cfc3b1cb74fe500b95b2
source_last_modified: "2026-01-03T18:07:59.238062+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Свидетельство аттестации StrongBox — развертывания в Японии

| Поле | Значение |
|-------|-------|
| Окно оценки | 10.02.2026 – 12.02.2026 |
| Местоположение артефакта | `artifacts/android/attestation/<device-tag>/<date>/` (формат пакета согласно `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`) |
| Инструменты для захвата | `scripts/android_keystore_attestation.sh`, `scripts/android_strongbox_attestation_ci.sh`, `scripts/android_strongbox_attestation_report.py` |
| Рецензенты | Руководитель лаборатории аппаратного обеспечения, нормативно-правовое обеспечение (JP) |

## 1. Процедура захвата

1. На каждом устройстве, указанном в матрице StrongBox, создайте задачу и запишите пакет аттестации:
   ```bash
   adb shell am instrument -w \
     org.hyperledger.iroha.android/.attestation.CaptureStrongBoxInstrumentation
   scripts/android_keystore_attestation.sh \
     --bundle-dir artifacts/android/attestation/${DEVICE_TAG}/2026-02-12 \
     --trust-root trust-roots/google-strongbox.pem \
     --require-strongbox \
     --output artifacts/android/attestation/${DEVICE_TAG}/2026-02-12/result.json
   ```
2. Зафиксируйте метаданные пакета (`result.json`, `chain.pem`, `challenge.hex`, `alias.txt`) в дереве доказательств.
3. Запустите помощник CI, чтобы повторно проверить все пакеты в автономном режиме:
   ```bash
   scripts/android_strongbox_attestation_ci.sh \
     --root artifacts/android/attestation
   scripts/android_strongbox_attestation_report.py \
     --input artifacts/android/attestation \
     --output artifacts/android/attestation/report_20260212.txt
   ```

## 2. Сводная информация об устройстве (12 февраля 2026 г.)

| Тег устройства | Модель / StrongBox | Путь пакета | Результат | Заметки |
|------------|-------------------|-------------|--------|-------|
| `pixel6-strongbox-a` | Пиксель 6/Тензор G1 | `artifacts/android/attestation/pixel6-strongbox-a/2026-02-12/result.json` | ✅ Пройдено (аппаратно) | Вызов связан, исправление ОС от 05 марта 2025 г. |
| `pixel7-strongbox-a` | Пиксель 7/Тензор G2 | `.../pixel7-strongbox-a/2026-02-12/result.json` | ✅ Пройдено | Кандидат на первичную полосу CI; температура в пределах спец. |
| `pixel8pro-strongbox-a` | Пиксель 8 Про / Тензор G3 | `.../pixel8pro-strongbox-a/2026-02-13/result.json` | ✅ Пройдено (повторное тестирование) | заменен концентратор USB-C; Buildkite `android-strongbox-attestation#221` захватил проходящий бандл. |
| `s23-strongbox-a` | Галактика S23 / Snapdragon 8 Gen 2 | `.../s23-strongbox-a/2026-02-12/result.json` | ✅ Пройдено | Профиль аттестации Knox импортирован 9 февраля 2026 г. |
| `s24-strongbox-a` | Галактика S24 / Snapdragon 8 3-го поколения | `.../s24-strongbox-a/2026-02-13/result.json` | ✅ Пройдено | Импортирован профиль аттестации Knox; Переулок CI теперь зеленый. |

Теги устройств соответствуют `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`.

## 3. Контрольный список рецензента

- [x] Убедитесь, что `result.json` показывает `strongbox_attestation: true` и цепочку сертификатов к доверенному корню.
- [x] Подтвердите, что байты запроса соответствуют запускам Buildkite `android-strongbox-attestation#219` (начальная развертка) и `#221` (повторное тестирование Pixel 8 Pro + захват S24).
- [x] Повторный запуск захвата Pixel 8 Pro после аппаратного исправления (владелец: руководитель аппаратной лаборатории, завершено 13 февраля 2026 г.).
- [x] Завершите захват Galaxy S24 после получения одобрения профиля Knox (владелец: Device Lab Ops, завершено 13 февраля 2026 г.).

## 4. Распространение

- Прикрепите это резюме, а также последний текстовый файл отчета к пакетам соответствия партнерам (контрольный список FISC §Размещение данных).
- Пути справочного пакета при реагировании на проверки регулирующих органов; не передавайте необработанные сертификаты за пределы зашифрованных каналов.

## 5. Журнал изменений

| Дата | Изменить | Автор |
|------|--------|--------|
| 12.02.2026 | Первоначальный захват пакета JP + отчет. | Операции лаборатории устройств |
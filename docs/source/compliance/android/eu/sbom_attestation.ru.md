---
lang: ru
direction: ltr
source: docs/source/compliance/android/eu/sbom_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d7eb66e5ba171d5c06aefa06ba9bd3e866596bc4efdbe16cb594990f46b5cb7
source_last_modified: "2026-01-04T11:42:43.493867+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SBOM и подтверждение происхождения — Android SDK

| Поле | Значение |
|-------|-------|
| Область применения | Android SDK (`java/iroha_android`) + примеры приложений (`examples/android/*`) |
| Владелец рабочего процесса | Release Engineering (Алексей Морозов) |
| Последняя проверка | 11 февраля 2026 г. (Buildkite `android-sdk-release#4821`) |

## 1. Рабочий процесс генерации

Запустите вспомогательный скрипт (добавлен для автоматизации AND6):

```bash
scripts/android_sbom_provenance.sh <sdk-version>
```

Скрипт выполняет следующее:

1. Выполняется `ci/run_android_tests.sh` и `scripts/check_android_samples.sh`.
2. Вызывает оболочку Gradle под `examples/android/` для создания SBOM CycloneDX для
   `:android-sdk`, `:operator-console` и `:retail-wallet` с входящим в комплект поставки
   `-PversionName`.
3. Копирует каждый SBOM в `artifacts/android/sbom/<sdk-version>/` с каноническими именами.
   (`iroha-android.cyclonedx.json` и т. д.).

## 2. Происхождение и подписание

Тот же сценарий подписывает каждый SBOM с помощью `cosign sign-blob --bundle <file>.sigstore --yes`.
и выдает `checksums.txt` (SHA-256) в каталоге назначения. Установите `COSIGN`.
переменная среды, если двоичный файл находится за пределами `$PATH`. После завершения сценария
запишите пути пакета/контрольной суммы плюс идентификатор запуска Buildkite в
`docs/source/compliance/android/evidence_log.csv`.

## 3. Проверка

Чтобы проверить опубликованный SBOM:

```bash
COSIGN_EXPERIMENTAL=1 cosign verify-blob \
  --bundle artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json.sigstore \
  --yes artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json
```

Сравните выходной SHA со значением, указанным в `checksums.txt`. Рецензенты также сравнивают SBOM с предыдущим выпуском, чтобы убедиться, что различия в зависимостях являются преднамеренными.

## 4. Снимок доказательств (11 февраля 2026 г.)

| Компонент | СБОМ | ША-256 | Sigstore Комплект |
|-----------|------|---------|-----------------|
| Android SDK (`java/iroha_android`) | `artifacts/android/sbom/0.9.0/iroha-android.cyclonedx.json` | `0fd522b78f9a43b5fd1d6c8ec8b2d980adff5d3c31e30c3c7e1f0f9d7f187a2d` | Пакет `.sigstore` хранится рядом с SBOM |
| Образец консоли оператора | `artifacts/android/sbom/0.9.0/operator-console.cyclonedx.json` | `e3e236350adcb5ee4c0a9a4a98c7166c308ebe1d2d5d9ec0a79251afd8c7e1e4` | `.sigstore` |
| Образец розничного кошелька | `artifacts/android/sbom/0.9.0/retail-wallet.cyclonedx.json` | `4d81352eec6b0f33811f87ec219a3f88949770b8c820035446880b1a1aaed1cc` | `.sigstore` |

*(Хеши, полученные из Buildkite, запускают `android-sdk-release#4821`; воспроизведите с помощью приведенной выше команды проверки.)*

## 5. Выдающаяся работа

- Автоматизируйте этапы SBOM + cosign внутри конвейера выпуска перед общедоступной версией.
- Отразите SBOM в общедоступную корзину артефактов, как только AND6 отметит контрольный список как завершенный.
- Координируйте свои действия с документацией, чтобы связать места загрузки SBOM с примечаниями к выпуску для партнеров.
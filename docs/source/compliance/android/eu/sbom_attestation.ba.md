---
lang: ba
direction: ltr
source: docs/source/compliance/android/eu/sbom_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7d7eb66e5ba171d5c06aefa06ba9bd3e866596bc4efdbe16cb594990f46b5cb7
source_last_modified: "2026-01-05T09:28:12.002687+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SBOM & Провенанс аттестация — Android SDK

| Ялан | Ҡиммәте |
|------|-------|
| Скоп | Android SDK (`java/iroha_android`) + өлгө ҡушымталар (`examples/android/*`) |
| Эш ағымы Милек | Релиз инженерияһы (Алексей Морозов) |
| Һуңғы раҫланған | 2026-02-11 (Buildkite `android-sdk-release#4821`) |

## 1. Быуындар эш ағымы

Ярҙамсы сценарийы (AND6 автоматлаштырыу өсөн өҫтәлгән):

```bash
scripts/android_sbom_provenance.sh <sdk-version>
```

Сценарий түбәндәгеләрҙе башҡара:

1. `ci/run_android_tests.sh` һәм `scripts/check_android_samples.sh` башҡара.
.
   `:android-sdk`, `:operator-console`, һәм `:retail-wallet` менән тәьмин ителгән
   `-PversionName`.
3. Һәр SBOM күсермәләр `artifacts/android/sbom/<sdk-version>/` канон исемдәре менән
   (`iroha-android.cyclonedx.json` һ.б.).

## 2. Провенанс һәм ҡул ҡуйыу

Шул уҡ сценарий һәр SBOM менән `cosign sign-blob --bundle <file>.sigstore --yes` менән билдәләре
һәм `checksums.txt` (SHA-256) сығарылыш каталогында сыға. `COSIGN` й.
мөхит үҙгәртеүсән, әгәр бинар йәшәй тыш `$PATH`. Сценарий тамамланғандан һуң,
яҙып алыу өйөм/чексум юлдары плюс artionkite йүгерә id .
`docs/source/compliance/android/evidence_log.csv`.

## 3. Тикшереү

Баҫылған СБОМ-ды раҫлау өсөн:

```bash
COSIGN_EXPERIMENTAL=1 cosign verify-blob \
  --bundle artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json.sigstore \
  --yes artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json
```

SHA сығышын `checksums.txt`-та күрһәтелгән ҡиммәт менән сағыштырығыҙ. Рецензенттар шулай уҡ айыра SBOM ҡаршы алдағы релиз өсөн тәьмин итеү өсөн бәйлелек дельта ниәтле.

## 4. Дәлилдәр Снэпшот (2026-02-11)

| Компонент | СБОМ | SHA-256 | Sigstore Бандл |
|---------|-------|-----------|-----------------|
| Android SDK (`java/iroha_android`) | `artifacts/android/sbom/0.9.0/iroha-android.cyclonedx.json` | `0fd522b78f9a43b5fd1d6c8ec8b2d980adff5d3c31e30c3c7e1f0f9d7f187a2d` | `.sigstore` өйөмө SBOM эргәһендә һаҡланған |
| Оператор консоль өлгөһө | `artifacts/android/sbom/0.9.0/operator-console.cyclonedx.json` | `e3e236350adcb5ee4c0a9a4a98c7166c308ebe1d2d5d9ec0a79251afd8c7e1e4` | `.sigstore` |
| Ваҡлап һатыу янсыҡ өлгөһө | `artifacts/android/sbom/0.9.0/retail-wallet.cyclonedx.json` | `4d81352eec6b0f33811f87ec219a3f88949770b8c820035446880b1a1aaed1cc` | `.sigstore` |

*(Хаштар Buildkite йүгерә `android-sdk-release#4821` йүгерә; өҫтәге тикшерелгән команда аша үрсетергә.)*

## 5. Күренекле эш

- Автоматлаштырыу SBOM + косигнация аҙымдары эсендә релиз торбаһы алдынан GA.
- Көҙгө SBOMs йәмәғәт артефакт биҙрәгә бер тапҡыр AND6 тикшерелгән исемлекте тамамлай.
- Docs менән координациялау өсөн һылтанма SBOM скачать урындары партнер-йөҙөндә релиз иҫкәрмәләр.
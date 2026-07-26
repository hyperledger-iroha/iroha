---
lang: kk
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

# SBOM & Provenance Attestation — Android SDK

| Өріс | Мән |
|-------|-------|
| Қолдану аясы | Android SDK (`java/iroha_android`) + үлгі қолданбалар (`examples/android/*`) |
| Жұмыс үрдісінің иесі | Шығарылым инженериясы (Алексей Морозов) |
| Соңғы расталған | 11.02.2026 (Buildkite `android-sdk-release#4821`) |

## 1. Генерацияның жұмыс процесі

Көмекші сценарийді іске қосыңыз (AND6 автоматтандыру үшін қосылған):

```bash
scripts/android_sbom_provenance.sh <sdk-version>
```

Сценарий келесі әрекеттерді орындайды:

1. `ci/run_android_tests.sh` және `scripts/check_android_samples.sh` орындайды.
2. CycloneDX SBOM құрастыру үшін `examples/android/` астындағы Gradle орауышын шақырады.
   `:android-sdk`, `:operator-console` және `:retail-wallet` жеткізілген
   `-PversionName`.
3. Әрбір SBOM файлын канондық атаулары бар `artifacts/android/sbom/<sdk-version>/` ішіне көшіреді.
   (`iroha-android.cyclonedx.json`, т.б.).

## 2. Шығу және қол қою

Бірдей сценарий әрбір SBOM-ға `cosign sign-blob --bundle <file>.sigstore --yes` арқылы қол қояды
және тағайындалған каталогта `checksums.txt` (SHA-256) шығарады. `COSIGN` орнатыңыз
егер екілік `$PATH` сыртында өмір сүрсе, орта айнымалысы. Сценарий аяқталғаннан кейін,
бума/бақылау сомасы жолдарын және Buildkite іске қосу идентификаторын жазыңыз
`docs/source/compliance/android/evidence_log.csv`.

## 3. Тексеру

Жарияланған SBOM растау үшін:

```bash
COSIGN_EXPERIMENTAL=1 cosign verify-blob \
  --bundle artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json.sigstore \
  --yes artifacts/android/sbom/${SDK_VERSION}/operator-console.cyclonedx.json
```

SHA шығысын `checksums.txt` ішінде тізімделген мәнмен салыстырыңыз. Тексерушілер сонымен қатар тәуелділік дельталарының әдейі жасалғанына көз жеткізу үшін SBOM-ды алдыңғы шығарылымнан ажыратады.

## 4. Дәлелдер суреті (11.02.2026)

| Құрамдас | SBOM | SHA-256 | Sigstore жинағы |
|----------|------|---------|-----------------|
| Android SDK (`java/iroha_android`) | `artifacts/android/sbom/0.9.0/iroha-android.cyclonedx.json` | `0fd522b78f9a43b5fd1d6c8ec8b2d980adff5d3c31e30c3c7e1f0f9d7f187a2d` | `.sigstore` бумасы SBOM | жанында сақталады
| Оператор консолінің үлгісі | `artifacts/android/sbom/0.9.0/operator-console.cyclonedx.json` | `e3e236350adcb5ee4c0a9a4a98c7166c308ebe1d2d5d9ec0a79251afd8c7e1e4` | `.sigstore` |
| Бөлшек әмиян үлгісі | `artifacts/android/sbom/0.9.0/retail-wallet.cyclonedx.json` | `4d81352eec6b0f33811f87ec219a3f88949770b8c820035446880b1a1aaed1cc` | `.sigstore` |

*(Buildkite-тен түсірілген хэштер `android-sdk-release#4821` іске қосылады; жоғарыдағы тексеру пәрмені арқылы қайта жасаңыз.)*

## 5. Үздік жұмыс

- GA алдында босату құбырының ішіндегі SBOM + косинг қадамдарын автоматтандыру.
- ЖӘНЕ6 бақылау тізімінің аяқталғанын белгілегеннен кейін SBOM файлдарын жалпыға ортақ артефакт шелегіне айналдырыңыз.
- Серіктес шығарылым жазбаларынан SBOM жүктеп алу орындарын байланыстыру үшін Docs қолданбасымен үйлестіріңіз.
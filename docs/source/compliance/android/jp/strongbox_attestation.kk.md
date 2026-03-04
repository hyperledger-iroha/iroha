---
lang: kk
direction: ltr
source: docs/source/compliance/android/jp/strongbox_attestation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8b8cc2e9de0c4183b51d011f5106a62b212da620d628cfc3b1cb74fe500b95b2
source_last_modified: "2025-12-29T18:16:35.929201+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# StrongBox Attestation Evidence — Жапонияның орналастырулары

| Өріс | Мән |
|-------|-------|
| Бағалау терезесі | 2026-02-10 – 2026-02-12 |
| Артефакттың орны | `artifacts/android/attestation/<device-tag>/<date>/` (`docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md` үшін пакет пішімі) |
| Түсіру құралдары | `scripts/android_keystore_attestation.sh`, `scripts/android_strongbox_attestation_ci.sh`, `scripts/android_strongbox_attestation_report.py` |
| Рецензенттер | Жабдық зертханасының жетекші, сәйкестік және заң (JP) |

## 1. Түсіру процедурасы

1. StrongBox матрицасында тізімделген әрбір құрылғыда тапсырма жасаңыз және аттестаттау жинағын алыңыз:
   ```bash
   adb shell am instrument -w \
     org.hyperledger.iroha.android/.attestation.CaptureStrongBoxInstrumentation
   scripts/android_keystore_attestation.sh \
     --bundle-dir artifacts/android/attestation/${DEVICE_TAG}/2026-02-12 \
     --trust-root trust-roots/google-strongbox.pem \
     --require-strongbox \
     --output artifacts/android/attestation/${DEVICE_TAG}/2026-02-12/result.json
   ```
2. Бума метадеректерін (`result.json`, `chain.pem`, `challenge.hex`, `alias.txt`) дәлелдер ағашына бекітіңіз.
3. Барлық бумаларды желіден тыс қайта тексеру үшін CI көмекшісін іске қосыңыз:
   ```bash
   scripts/android_strongbox_attestation_ci.sh \
     --root artifacts/android/attestation
   scripts/android_strongbox_attestation_report.py \
     --input artifacts/android/attestation \
     --output artifacts/android/attestation/report_20260212.txt
   ```

## 2. Құрылғының қысқаша мазмұны (2026-02-12)

| Құрылғы тегі | Үлгі / StrongBox | Бума жолы | Нәтиже | Ескертпелер |
|------------|-------------------|-------------|--------|-------|
| `pixel6-strongbox-a` | Pixel 6 / Tensor G1 | `artifacts/android/attestation/pixel6-strongbox-a/2026-02-12/result.json` | ✅ Өтті (аппараттық қамтамасыз етілген) | Сынақ орындалды, OS патч 2025-03-05. |
| `pixel7-strongbox-a` | Pixel 7 / Tensor G2 | `.../pixel7-strongbox-a/2026-02-12/result.json` | ✅ Өтті | Негізгі CI жолағына үміткер; спецификациядағы температура. |
| `pixel8pro-strongbox-a` | Pixel 8 Pro / Tensor G3 | `.../pixel8pro-strongbox-a/2026-02-13/result.json` | ✅ Өтті (қайта тестілеу) | USB-C хабы ауыстырылды; Buildkite `android-strongbox-attestation#221` өтетін буманы басып алды. |
| `s23-strongbox-a` | Galaxy S23 / Snapdragon 8 Gen 2 | `.../s23-strongbox-a/2026-02-12/result.json` | ✅ Өтті | Нокс аттестаттау профилі импортталған 2026-02-09. |
| `s24-strongbox-a` | Galaxy S24 / Snapdragon 8 Gen 3 | `.../s24-strongbox-a/2026-02-13/result.json` | ✅ Өтті | Knox аттестаттау профилі импортталды; CI жолағы қазір жасыл. |

Құрылғы тегтері `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` сәйкес келеді.

## 3. Рецензенттің бақылау тізімі

- [x] `result.json` `strongbox_attestation: true` және сертификаттар тізбегін сенімді түбірге көрсететінін тексеріңіз.
- [x] Buildkite `android-strongbox-attestation#219` (бастапқы тазалау) және `#221` (Pixel 8 Pro қайта сынау + S24 түсіру) Buildkite іске қосу байттарының сәйкестігін растау.
- [x] Аппараттық құрал түзетілгеннен кейін Pixel 8 Pro түсірілімін қайта іске қосыңыз (иесі: Hardware Lab Lead, 2026-02-13 аяқталды).
- [x] Knox профилін мақұлдағаннан кейін Galaxy S24 түсіруді аяқтаңыз (иесі: Device Lab Ops, 2026-02-13 аяқталды).

## 4. Тарату

- Осы қорытындыны және ең соңғы есеп мәтіндік файлын серіктес сәйкестік пакеттеріне тіркеңіз (FISC тексеру тізімі §Деректердің резиденттігі).
- Реттеуші аудиттерге жауап беру кезіндегі анықтамалық десте жолдары; шифрланған арналардан тыс шикі сертификаттарды жібермеңіз.

## 5. Журналды өзгерту

| Күні | Өзгерту | Автор |
|------|--------|--------|
| 12.02.2026 | Бастапқы JP жинағы түсіру + есеп. | Device Lab Ops |
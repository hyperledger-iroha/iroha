---
lang: hy
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

# StrongBox ատեստավորման ապացույց — Ճապոնիայի տեղակայումներ

| Դաշտային | Արժեք |
|-------|-------|
| Գնահատման պատուհան | 2026-02-10 – 2026-02-12 |
| Արտեֆակտի գտնվելու վայրը | `artifacts/android/attestation/<device-tag>/<date>/` (փաթեթի ձևաչափը ըստ `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`-ի) |
| Նկարահանման գործիքակազմ | `scripts/android_keystore_attestation.sh`, `scripts/android_strongbox_attestation_ci.sh`, `scripts/android_strongbox_attestation_report.py` |
| Գրախոսներ | Hardware Lab առաջատար, համապատասխանության և իրավական (JP) |

## 1. Գրավման կարգը

1. StrongBox մատրիցայում թվարկված յուրաքանչյուր սարքի վրա ստեղծեք մարտահրավեր և գրավեք ատեստավորման փաթեթը.
   ```bash
   adb shell am instrument -w \
     org.hyperledger.iroha.android/.attestation.CaptureStrongBoxInstrumentation
   scripts/android_keystore_attestation.sh \
     --bundle-dir artifacts/android/attestation/${DEVICE_TAG}/2026-02-12 \
     --trust-root trust-roots/google-strongbox.pem \
     --require-strongbox \
     --output artifacts/android/attestation/${DEVICE_TAG}/2026-02-12/result.json
   ```
2. Հանձնարարեք փաթեթի մետատվյալները (`result.json`, `chain.pem`, `challenge.hex`, `alias.txt`) ապացույցների ծառին:
3. Գործարկեք CI helper-ը՝ բոլոր փաթեթները անցանց ռեժիմում նորից ստուգելու համար:
   ```bash
   scripts/android_strongbox_attestation_ci.sh \
     --root artifacts/android/attestation
   scripts/android_strongbox_attestation_report.py \
     --input artifacts/android/attestation \
     --output artifacts/android/attestation/report_20260212.txt
   ```

## 2. Սարքի ամփոփում (2026-02-12)

| Սարքի պիտակ | Մոդել / StrongBox | Փաթեթի ուղի | Արդյունք | Ծանոթագրություններ |
|-----------------------------------------------|--------|-------|
| `pixel6-strongbox-a` | Pixel 6 / Tensor G1 | `artifacts/android/attestation/pixel6-strongbox-a/2026-02-12/result.json` | ✅ Անցած (ապարատային ապահովված) | Մարտահրավերի սահմանափակում, ՕՀ-ի կարկատել 2025-03-05: |
| `pixel7-strongbox-a` | Pixel 7 / Tensor G2 | `.../pixel7-strongbox-a/2026-02-12/result.json` | ✅ Անցել է | Առաջնային CI գոտի թեկնածու; ջերմաստիճանի սահմաններում |
| `pixel8pro-strongbox-a` | Pixel 8 Pro / Tensor G3 | `.../pixel8pro-strongbox-a/2026-02-13/result.json` | ✅ Անցել է (վերափորձարկում) | USB-C հանգույցը փոխված; Buildkite `android-strongbox-attestation#221`-ը գրավել է անցնող փաթեթը: |
| `s23-strongbox-a` | Galaxy S23 / Snapdragon 8 Gen 2 | `.../s23-strongbox-a/2026-02-12/result.json` | ✅ Անցել է | Knox ատեստավորման պրոֆիլը ներմուծվել է 2026-02-09: |
| `s24-strongbox-a` | Galaxy S24 / Snapdragon 8 Gen 3 | `.../s24-strongbox-a/2026-02-13/result.json` | ✅ Անցել է | Knox ատեստավորման պրոֆիլը ներմուծվել է; CI գոտի այժմ կանաչ: |

Սարքի պիտակները քարտեզ են `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`-ում:

## 3. Գրախոսների ստուգաթերթ

- [x] Ստուգեք, որ `result.json`-ը ցույց է տալիս `strongbox_attestation: true`-ը և վկայագրերի շղթան վստահելի արմատին:
- [x] Հաստատեք մարտահրավեր բայթերի համընկնում Buildkite-ի `android-strongbox-attestation#219` (նախնական մաքրում) և `#221` (Pixel 8 Pro վերստուգում + S24 նկարահանում):
- [x] Վերագործարկեք Pixel 8 Pro-ի նկարահանումը ապարատային շտկումից հետո (սեփականատեր՝ Hardware Lab Lead, ավարտված 2026-02-13):
- [x] Լրացրեք Galaxy S24-ի նկարահանումը Knox-ի պրոֆիլի հաստատումից հետո (սեփականատեր՝ Device Lab Ops, ավարտված 2026-02-13):

## 4. Բաշխում

- Կցեք այս ամփոփագիրը, ինչպես նաև վերջին հաշվետվության տեքստային ֆայլը գործընկերների համապատասխանության փաթեթներին (FISC ստուգաթերթ §Տվյալների ռեզիդենտություն):
- Կարգավորիչի աուդիտներին արձագանքելիս տեղեկանք փաթեթի ուղիները. մի փոխանցեք չմշակված վկայագրերը կոդավորված ալիքներից դուրս:

## 5. Փոփոխության մատյան

| Ամսաթիվ | Փոփոխություն | Հեղինակ |
|------|--------|--------|
| 2026-02-12 | Նախնական JP փաթեթի հավաքում + հաշվետվություն: | Սարքի լաբորատորիա |
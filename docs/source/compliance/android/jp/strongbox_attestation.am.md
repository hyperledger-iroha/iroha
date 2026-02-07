---
lang: am
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

# StrongBox የማረጋገጫ ማስረጃ - የጃፓን ማሰማራቶች

| መስክ | ዋጋ |
|-------|------|
| የግምገማ መስኮት | 2026-02-10 - 2026-02-12 |
| Artefact አካባቢ | `artifacts/android/attestation/<device-tag>/<date>/` (ጥቅል ቅርጸት በ `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`) |
| ቀረጻ መሳሪያ | `scripts/android_keystore_attestation.sh`፣ `scripts/android_strongbox_attestation_ci.sh`፣ `scripts/android_strongbox_attestation_report.py` |
| ገምጋሚዎች | የሃርድዌር ላብ አመራር፣ ተገዢነት እና ህጋዊ (ጄፒ) |

## 1. የመቅረጽ ሂደት

1. በ StrongBox ማትሪክስ ውስጥ በተዘረዘረው እያንዳንዱ መሳሪያ ላይ ፈተና ይፍጠሩ እና የማረጋገጫ ቅርቅቡን ይያዙ፡-
   ```bash
   adb shell am instrument -w \
     org.hyperledger.iroha.android/.attestation.CaptureStrongBoxInstrumentation
   scripts/android_keystore_attestation.sh \
     --bundle-dir artifacts/android/attestation/${DEVICE_TAG}/2026-02-12 \
     --trust-root trust-roots/google-strongbox.pem \
     --require-strongbox \
     --output artifacts/android/attestation/${DEVICE_TAG}/2026-02-12/result.json
   ```
2. የጥቅል ሜታዳታ (`result.json`፣ `chain.pem`፣ `challenge.hex`፣ `alias.txt`) ወደ ማስረጃው ዛፍ አስገባ።
3. ሁሉንም ጥቅሎች ከመስመር ውጭ እንደገና ለማረጋገጥ የCI አጋዥን ያሂዱ፡-
   ```bash
   scripts/android_strongbox_attestation_ci.sh \
     --root artifacts/android/attestation
   scripts/android_strongbox_attestation_report.py \
     --input artifacts/android/attestation \
     --output artifacts/android/attestation/report_20260212.txt
   ```

## 2. የመሣሪያ ማጠቃለያ (2026-02-12)

| የመሣሪያ መለያ | ሞዴል / StrongBox | የጥቅል መንገድ | ውጤት | ማስታወሻ |
|--------|------------|--------|
| `pixel6-strongbox-a` | Pixel 6 / Tensor G1 | `artifacts/android/attestation/pixel6-strongbox-a/2026-02-12/result.json` | ✅ አልፏል (በሃርድዌር የተደገፈ) | ፈታኝ ገደብ፣ OS patch 2025-03-05። |
| `pixel7-strongbox-a` | Pixel 7 / Tensor G2 | `.../pixel7-strongbox-a/2026-02-12/result.json` | ✅ አለፈ | ዋና የ CI ሌይን እጩ; በ spec ውስጥ የሙቀት መጠን. |
| `pixel8pro-strongbox-a` | Pixel 8 Pro / Tensor G3 | `.../pixel8pro-strongbox-a/2026-02-13/result.json` | ✅ አልፏል (ሙከራ) | የዩኤስቢ-ሲ ማእከል ተተክቷል; Buildkite `android-strongbox-attestation#221` የማለፊያ ቅርቅቡን ያዘ። |
| `s23-strongbox-a` | ጋላክሲ S23 / Snapdragon 8 Gen 2 | `.../s23-strongbox-a/2026-02-12/result.json` | ✅ አለፈ | የኖክስ ማረጋገጫ መገለጫ 2026-02-09 መጥቷል። |
| `s24-strongbox-a` | ጋላክሲ S24 / Snapdragon 8 Gen 3 | `.../s24-strongbox-a/2026-02-13/result.json` | ✅ አለፈ | የኖክስ ማረጋገጫ መገለጫ ከውጪ ገብቷል; CI ሌይን አሁን አረንጓዴ። |

የመሣሪያ መለያዎች ካርታ ወደ `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`።

## 3. የገምጋሚ ዝርዝር

- [x] አረጋግጥ `result.json` `strongbox_attestation: true` እና የምስክር ወረቀቶች ሰንሰለት የታመነ ስር ያሳያል።
- [x] የግጥሚያ ባይት ግጥሚያ ያረጋግጡ Buildkite `android-strongbox-attestation#219` (የመጀመሪያ መጥረግ) እና `#221` (Pixel 8 Pro retest + S24 ቀረጻ)።
- [x] Pixel 8 Pro ቀረጻን ከሃርድዌር ጥገና በኋላ እንደገና ያሂዱ (ባለቤት፡ የሃርድዌር ላብ መሪ፣ 2026-02-13 የተጠናቀቀ)።
- [x] የኖክስ ፕሮፋይል ማጽደቁ አንዴ ከደረሰ ጋላክሲ S24 ቀረጻን ያጠናቅቁ (ባለቤት፡ Device Lab Ops፣ የተጠናቀቀው 2026-02-13)።

## 4. ስርጭት

- ይህን ማጠቃለያ እና የቅርብ ጊዜውን የጽሁፍ ፋይል ከአጋር ተገዢነት ፓኬቶች ጋር ያያይዙ (FISC ማረጋገጫ ዝርዝር §የውሂብ ነዋሪነት)።
- ለተቆጣጣሪ ኦዲቶች ምላሽ ሲሰጡ የማጣቀሻ ጥቅል መንገዶች; ከተመሰጠሩ ቻናሎች ውጭ ጥሬ የምስክር ወረቀቶችን አያስተላልፉ።

## 5. ሎግ ለውጥ

| ቀን | ለውጥ | ደራሲ |
|------|--------|----|
| 2026-02-12 | የመጀመሪያ JP ጥቅል ቀረጻ + ሪፖርት። | Device Lab Ops |
---
lang: uz
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

# StrongBox attestatsiyasi dalillari - Yaponiyani joylashtirish

| Maydon | Qiymat |
|-------|-------|
| Baholash oynasi | 2026-02-10 - 2026-02-12 |
| Artefakt joylashuvi | `artifacts/android/attestation/<device-tag>/<date>/` (`docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md` uchun to'plam formati) |
| Capture Tooling | `scripts/android_keystore_attestation.sh`, `scripts/android_strongbox_attestation_ci.sh`, `scripts/android_strongbox_attestation_report.py` |
| Taqrizchilar | Hardware Lab Lead, Compliance & Legal (JP) |

## 1. Rasmga olish tartibi

1. StrongBox matritsasi roʻyxatidagi har bir qurilmada muammo yarating va attestatsiya toʻplamini oling:
   ```bash
   adb shell am instrument -w \
     org.hyperledger.iroha.android/.attestation.CaptureStrongBoxInstrumentation
   scripts/android_keystore_attestation.sh \
     --bundle-dir artifacts/android/attestation/${DEVICE_TAG}/2026-02-12 \
     --trust-root trust-roots/google-strongbox.pem \
     --require-strongbox \
     --output artifacts/android/attestation/${DEVICE_TAG}/2026-02-12/result.json
   ```
2. Toʻplam metamaʼlumotlarini (`result.json`, `chain.pem`, `challenge.hex`, `alias.txt`) dalillar daraxtiga topshiring.
3. Barcha paketlarni oflayn rejimda qayta tekshirish uchun CI yordamchisini ishga tushiring:
   ```bash
   scripts/android_strongbox_attestation_ci.sh \
     --root artifacts/android/attestation
   scripts/android_strongbox_attestation_report.py \
     --input artifacts/android/attestation \
     --output artifacts/android/attestation/report_20260212.txt
   ```

## 2. Qurilma haqida qisqacha maʼlumot (2026-02-12)

| Qurilma yorlig'i | Model / StrongBox | Toʻplam yoʻli | Natija | Eslatmalar |
|------------|-------------------|-------------|--------|-------|
| `pixel6-strongbox-a` | Pixel 6 / Tensor G1 | `artifacts/android/attestation/pixel6-strongbox-a/2026-02-12/result.json` | ✅ O'tdi (apparat bilan ta'minlangan) | Challenjga bog‘liq, OS yamog‘i 2025-03-05. |
| `pixel7-strongbox-a` | Pixel 7 / Tensor G2 | `.../pixel7-strongbox-a/2026-02-12/result.json` | ✅ O'tdi | Boshlang'ich CI qatoriga nomzod; spetsifikatsiya doirasidagi harorat. |
| `pixel8pro-strongbox-a` | Pixel 8 Pro / Tensor G3 | `.../pixel8pro-strongbox-a/2026-02-13/result.json` | ✅ O'tgan (qayta sinov) | USB-C uyasi almashtirildi; Buildkite `android-strongbox-attestation#221` o'tgan to'plamni qo'lga kiritdi. |
| `s23-strongbox-a` | Galaxy S23 / Snapdragon 8 Gen 2 | `.../s23-strongbox-a/2026-02-12/result.json` | ✅ O'tdi | Knox attestatsiya profili 2026-02-09 import qilingan. |
| `s24-strongbox-a` | Galaxy S24 / Snapdragon 8 Gen 3 | `.../s24-strongbox-a/2026-02-13/result.json` | ✅ O'tdi | Knox attestatsiya profili import qilindi; CI qator endi yashil. |

Qurilma teglari `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` ga mos keladi.

## 3. Ko'rib chiquvchining nazorat ro'yxati

- [x] `result.json` `strongbox_attestation: true` va sertifikatlar zanjirini ishonchli ildizga koʻrsatganligini tekshiring.
- [x] Buildkite ishlayotgan `android-strongbox-attestation#219` (dastlabki tozalash) va `#221` (Pixel 8 Pro qayta sinovi + S24 suratga olish) sinov baytlari mos kelishini tasdiqlang.
- [x] Uskuna tuzatilgandan keyin Pixel 8 Pro suratga olishni qayta ishga tushiring (egasi: Hardware Lab Lead, 2026-02-13 tugallangan).
- [x] Knox profili tasdiqlangach, Galaxy S24-ni toʻliq suratga oling (egasi: Device Lab Ops, 2026-02-13 tugallangan).

## 4. Tarqatish

- Ushbu xulosa va so'nggi hisobot matn faylini hamkorlar muvofiqlik paketlariga (FISC nazorat ro'yxati §Ma'lumotlar rezidentligi) ilova qiling.
- Regulyator tekshiruvlariga javob berishda ma'lumot to'plami yo'llari; xom sertifikatlarni shifrlangan kanallardan tashqariga uzatmang.

## 5. Jurnalni o'zgartirish

| Sana | O'zgartirish | Muallif |
|------|--------|--------|
| 2026-02-12 | Dastlabki JP paketini yozib olish + hisobot. | Device Lab Operations |
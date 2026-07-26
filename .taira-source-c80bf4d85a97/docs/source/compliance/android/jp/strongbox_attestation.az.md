---
lang: az
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

# StrongBox Attestasiya Sübutları - Yaponiya Yerləşdirmələri

| Sahə | Dəyər |
|-------|-------|
| Qiymətləndirmə Pəncərəsi | 2026-02-10 – 2026-02-12 |
| Artefakt Yeri | `artifacts/android/attestation/<device-tag>/<date>/` (`docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md` üçün paket formatı) |
| Capture Tooling | `scripts/android_keystore_attestation.sh`, `scripts/android_strongbox_attestation_ci.sh`, `scripts/android_strongbox_attestation_report.py` |
| Rəyçilər | Hardware Lab Lead, Compliance & Legal (JP) |

## 1. Çəkmə Proseduru

1. StrongBox matrisində sadalanan hər bir cihazda problem yaradın və attestasiya paketini əldə edin:
   ```bash
   adb shell am instrument -w \
     org.hyperledger.iroha.android/.attestation.CaptureStrongBoxInstrumentation
   scripts/android_keystore_attestation.sh \
     --bundle-dir artifacts/android/attestation/${DEVICE_TAG}/2026-02-12 \
     --trust-root trust-roots/google-strongbox.pem \
     --require-strongbox \
     --output artifacts/android/attestation/${DEVICE_TAG}/2026-02-12/result.json
   ```
2. Paket metadatasını (`result.json`, `chain.pem`, `challenge.hex`, `alias.txt`) sübut ağacına həvalə edin.
3. Bütün paketləri oflayn olaraq yenidən yoxlamaq üçün CI köməkçisini işə salın:
   ```bash
   scripts/android_strongbox_attestation_ci.sh \
     --root artifacts/android/attestation
   scripts/android_strongbox_attestation_report.py \
     --input artifacts/android/attestation \
     --output artifacts/android/attestation/report_20260212.txt
   ```

## 2. Cihaz Xülasəsi (2026-02-12)

| Cihaz etiketi | Model / StrongBox | Paket Yolu | Nəticə | Qeydlər |
|------------|-------------------|-------------|--------|-------|
| `pixel6-strongbox-a` | Pixel 6 / Tensor G1 | `artifacts/android/attestation/pixel6-strongbox-a/2026-02-12/result.json` | ✅ Keçildi (aparat dəstəyi ilə) | Çağırış bağlandı, OS yaması 2025-03-05. |
| `pixel7-strongbox-a` | Pixel 7 / Tensor G2 | `.../pixel7-strongbox-a/2026-02-12/result.json` | ✅ Keçdi | Əsas CI zolağı namizədi; spesifikasiyalar daxilində temperatur. |
| `pixel8pro-strongbox-a` | Pixel 8 Pro / Tensor G3 | `.../pixel8pro-strongbox-a/2026-02-13/result.json` | ✅ Keçildi (yenidən test) | USB-C hub dəyişdirildi; Buildkite `android-strongbox-attestation#221` keçən paketi ələ keçirdi. |
| `s23-strongbox-a` | Galaxy S23 / Snapdragon 8 Gen 2 | `.../s23-strongbox-a/2026-02-12/result.json` | ✅ Keçdi | Knox attestasiya profili idxal edilib 2026-02-09. |
| `s24-strongbox-a` | Galaxy S24 / Snapdragon 8 Gen 3 | `.../s24-strongbox-a/2026-02-13/result.json` | ✅ Keçdi | Knox attestasiya profili idxal edildi; CI zolağı indi yaşıldır. |

Cihaz teqləri `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md` ilə əlaqələndirilir.

## 3. Rəyçinin Yoxlama Siyahısı

- [x] `result.json`-in `strongbox_attestation: true` və sertifikatların etibarlı kökə zəncirini göstərdiyini yoxlayın.
- [x] Buildkite `android-strongbox-attestation#219` (ilkin tarama) və `#221` (Pixel 8 Pro təkrar testi + S24 ələ keçirmə) ilə uyğun gələn çağırış baytlarını təsdiqləyin.
- [x] Aparat düzəldildikdən sonra Pixel 8 Pro çəkilişini yenidən işə salın (sahibi: Hardware Lab Lead, 2026-02-13 tamamlandı).
- [x] Knox profilinin təsdiqi gələn kimi tam Galaxy S24 çəkilişini tamamlayın (sahibi: Cihaz Laboratoriyası, 2026-02-13 tamamlandı).

## 4. Paylanma

- Bu xülasə və ən son hesabat mətn faylını tərəfdaş uyğunluq paketlərinə əlavə edin (FISC yoxlama siyahısı §Data rezidentliyi).
- Tənzimləyici auditlərə cavab verərkən istinad paketinin yolları; xam sertifikatları şifrələnmiş kanallardan kənara ötürməyin.

## 5. Qeydləri dəyişdirin

| Tarix | Dəyişiklik | Müəllif |
|------|--------|--------|
| 2026-02-12 | İlkin JP paketinin tutulması + hesabat. | Cihaz Laboratoriyası Əməliyyatları |
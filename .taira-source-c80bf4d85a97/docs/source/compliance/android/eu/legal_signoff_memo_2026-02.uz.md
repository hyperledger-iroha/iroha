---
lang: uz
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo_2026-02.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eb92b77765ced36213a0bde55581f29d59c262f398c658f35a1fb43a182fe296
source_last_modified: "2025-12-29T18:16:35.926476+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 EI huquqiy roʻyxatdan oʻtish memosi — 2026.1 GA (Android SDK)

## Xulosa

- **Reliz / Poezd:** 2026.1 GA (Android SDK)
- **Ko'rib chiqish sanasi:** 2026-04-15
- **Maslahatchi / Sharhlovchi:** Sofiya Martins — Muvofiqlik va huquq
- **Qo'llanilish doirasi:** ETSI EN 319 401 xavfsizlik maqsadi, GDPR DPIA xulosasi, SBOM attestatsiyasi, AND6 qurilma-laboratoriya favqulodda holatlari dalillari
- **Aloqador chiptalar:** `_android-device-lab` / AND6-DR-202602, AND6 boshqaruv kuzatuvchisi (`GOV-AND6-2026Q1`)

## Artefaktni tekshirish ro'yxati

| Artefakt | SHA-256 | Joylashuv / Havola | Eslatmalar |
|----------|---------|-----------------|-------|
| `security_target.md` | `385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0` | `docs/source/compliance/android/eu/security_target.md` | 2026.1 GA reliz identifikatorlari va tahdid modeli deltalariga mos keladi (Torii NRPC qoʻshimchalari). |
| `gdpr_dpia_summary.md` | `8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5` | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | Ma'lumotnomalar AND7 telemetriya siyosati (`docs/source/sdk/android/telemetry_redaction.md`). |
| `sbom_attestation.md` | `c2e0de176d4bb8c8e09329e2b9ee5dd93228d3f0def78225c1d8b777a5613f2d` | `docs/source/compliance/android/eu/sbom_attestation.md` + Sigstore to'plami (`android-sdk-release#4821`). | CycloneDX + kelib chiqishi ko'rib chiqildi; Buildkite ishiga mos keladi `android-sdk-release#4821`. |
| Dalillar jurnali | `0b2d2f9eddada06faa70620f608c3ad1ec38f378d2cbddc24b15d0a83fcc381d` | `docs/source/compliance/android/evidence_log.csv` (qator `android-device-lab-failover-20260220`) | Jurnaldan olingan toʻplam xeshlarini + sigʻimli oniy tasvirni + eslatma yozuvini tasdiqlaydi. |
| Qurilma-laboratoriya favqulodda vaziyatlar to'plami | `faf32356dfc0bbca1459b14d75f3306ea1c10cb40f3180fe1758ac5105016f85` | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | Xesh `bundle-manifest.json` dan olingan; AND6-DR-202602 chiptasi Huquqiy/Muvofiqlik uchun topshirilgan. |

## Topilmalar va istisnolar

- Bloklash muammolari aniqlanmagan. Artefaktlar ETSI/GDPR talablariga mos keladi; AND7 telemetriya pariteti DPIA xulosasida qayd etilgan va qo‘shimcha yumshatilishlar talab qilinmaydi.
- Tavsiya: rejalashtirilgan DR-2026-05-Q2 mashg'ulotini kuzatib boring (chipta AND6-DR-202605) va natijada olingan to'plamni keyingi boshqaruv nazorat punktidan oldin dalillar jurnaliga qo'shing.

## Tasdiqlash

- **Qaror:** Tasdiqlangan
- **Imzo/vaqt tamg‘asi:** _Sofia Martins (boshqaruv portali orqali raqamli imzolangan, 2026-04-15 14:32 UTC)_
- **Kuzatuvchi egalari:** Device Lab Ops (DR-2026-05-Q2 dalillar to‘plamini 2026-05-31 gacha yetkazib berish)
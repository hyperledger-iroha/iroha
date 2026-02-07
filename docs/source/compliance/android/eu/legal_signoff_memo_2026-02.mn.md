---
lang: mn
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

# AND6 ЕХ-ны эрх зүйн гарын үсэг зурах санамж — 2026.1 GA (Android SDK)

## Дүгнэлт

- **Хувилбар / Галт тэрэг:** 2026.1 GA (Android SDK)
- **Хянах огноо:** 2026-04-15
- **Зөвлөгч / Шүүмжлэгч:** София Мартинс — Дагаж мөрдөх, хууль эрх зүй
- **Хамрах хүрээ:** ETSI EN 319 401 аюулгүй байдлын зорилт, GDPR DPIA хураангуй, SBOM баталгаажуулалт, AND6 төхөөрөмжийн лабораторийн болзошгүй байдлын нотолгоо
- **Холбоотой тасалбарууд:** `_android-device-lab` / AND6-DR-202602, AND6 засаглалын мөрдөгч (`GOV-AND6-2026Q1`)

## Олдворыг шалгах хуудас

| Олдвор | SHA-256 | Байршил / Холбоос | Тэмдэглэл |
|----------|---------|-----------------|-------|
| `security_target.md` | `385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0` | `docs/source/compliance/android/eu/security_target.md` | 2026.1 GA хувилбарын танигч болон аюулын загварын дельтатай таарч байна (Torii NRPC нэмэлт). |
| `gdpr_dpia_summary.md` | `8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5` | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | Лавлагаа AND7 телеметрийн бодлого (`docs/source/sdk/android/telemetry_redaction.md`). |
| `sbom_attestation.md` | `c2e0de176d4bb8c8e09329e2b9ee5dd93228d3f0def78225c1d8b777a5613f2d` | `docs/source/compliance/android/eu/sbom_attestation.md` + Sigstore багц (`android-sdk-release#4821`). | CycloneDX + гарал үүслийг хянаж үзсэн; Buildkite ажил `android-sdk-release#4821` таарч байна. |
| Нотлох баримтын бүртгэл | `0b2d2f9eddada06faa70620f608c3ad1ec38f378d2cbddc24b15d0a83fcc381d` | `docs/source/compliance/android/evidence_log.csv` (мөр `android-device-lab-failover-20260220`) | Бүртгэлд авсан багцын хэш + багтаамжийн агшин зуурын зураг + санах ойн оруулгыг баталгаажуулна. |
| Төхөөрөмжийн лабораторийн ослын багц | `faf32356dfc0bbca1459b14d75f3306ea1c10cb40f3180fe1758ac5105016f85` | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | `bundle-manifest.json`-аас авсан хэш; тасалбар AND6-DR-202602-д Хуулийн/ Нийцлийн хэсэгт хүлээлгэн өгөхийг бүртгэсэн. |

## Дүгнэлт ба үл хамаарах зүйлүүд

- Блоклох асуудал илрээгүй. Олдворууд ETSI/GDPR шаардлагад нийцдэг; AND7 телеметрийн паритетийг DPIA хураангуйд тэмдэглэсэн бөгөөд нэмэлт бууруулах шаардлагагүй.
- Зөвлөмж: Төлөвлөсөн DR-2026-05-Q2 дасгал сургуулилтыг (тасалбар AND6-DR-202605) хянаж, үр дүнгийн багцыг дараагийн засаглалын хяналтын цэгээс өмнө нотлох баримтын бүртгэлд хавсаргана уу.

## Зөвшөөрөл

- **Шийдвэр:** Батлагдсан
- **Гарын үсэг / Цагийн тэмдэг:** _София Мартинс (засаглалын порталаар цахимаар гарын үсэг зурсан, 2026-04-15 14:32 UTC)_
- **Мэргэж буй эзэд:** Төхөөрөмжийн лабораторийн үйл ажиллагаа (DR-2026-05-2026-05-31-ээс өмнө нотлох баримтын багцыг хүргэх)
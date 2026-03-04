---
lang: ka
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

# AND6 ევროკავშირის იურიდიული გაფორმების მემორანდუმი — 2026.1 GA (Android SDK)

## რეზიუმე

- **გამოშვება / მატარებელი:** 2026.1 GA (Android SDK)
- ** განხილვის თარიღი: ** 2026-04-15
- ** მრჩეველი / მიმომხილველი: ** სოფია მარტინსი - შესაბამისობა და იურიდიული
- **ფარგლები:** ETSI EN 319 401 უსაფრთხოების სამიზნე, GDPR DPIA შეჯამება, SBOM ატესტაცია, AND6 მოწყობილობა-ლაბორატორია საგანგებო მტკიცებულება
- **ასოცირებული ბილეთები:** `_android-device-lab` / AND6-DR-202602, AND6 მართვის ტრეკერი (`GOV-AND6-2026Q1`)

## არტეფაქტის ჩამონათვალი

| არტეფაქტი | SHA-256 | მდებარეობა / ბმული | შენიშვნები |
|----------|---------|----------------|------|
| `security_target.md` | `385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0` | `docs/source/compliance/android/eu/security_target.md` | შეესაბამება 2026.1 GA გამოშვების იდენტიფიკატორებს და საფრთხის მოდელის დელტას (Torii NRPC დამატებები). |
| `gdpr_dpia_summary.md` | `8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5` | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | მითითებები AND7 ტელემეტრიის პოლიტიკა (`docs/source/sdk/android/telemetry_redaction.md`). |
| `sbom_attestation.md` | `c2e0de176d4bb8c8e09329e2b9ee5dd93228d3f0def78225c1d8b777a5613f2d` | `docs/source/compliance/android/eu/sbom_attestation.md` + Sigstore პაკეტი (`android-sdk-release#4821`). | CycloneDX + წარმოშობის განხილვა; შეესაბამება Buildkite სამუშაო `android-sdk-release#4821`. |
| მტკიცებულებათა ჟურნალი | `0b2d2f9eddada06faa70620f608c3ad1ec38f378d2cbddc24b15d0a83fcc381d` | `docs/source/compliance/android/evidence_log.csv` (რიგი `android-device-lab-failover-20260220`) | ადასტურებს ჟურნალის დაფიქსირებული ნაკრების ჰეშებს + ტევადობის სნეპშოტს + შენიშვნის ჩანაწერს. |
| მოწყობილობა-ლაბორატორია გაუთვალისწინებელი პაკეტი | `faf32356dfc0bbca1459b14d75f3306ea1c10cb40f3180fe1758ac5105016f85` | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | ჰეში აღებულია `bundle-manifest.json`-დან; ბილეთი AND6-DR-202602 ჩაწერილი გადაცემა Legal/Compliance-ში. |

## აღმოჩენები და გამონაკლისები

- არ არის გამოვლენილი დაბლოკვის საკითხები. არტეფაქტები შეესაბამება ETSI/GDPR მოთხოვნებს; AND7 ტელემეტრიის პარიტეტი მითითებულია DPIA-ს შეჯამებაში და არ არის საჭირო დამატებითი შემარბილებელი ღონისძიებები.
- რეკომენდაცია: თვალყური ადევნეთ დაგეგმილ DR-2026-05-Q2 სავარჯიშოს (ბილეთი AND6-DR-202605) და დაურთეთ მიღებული ნაკრები მტკიცებულებების ჟურნალში მომდევნო საკონტროლო პუნქტამდე.

## დამტკიცება

- **გადაწყვეტილება:** დამტკიცდა
- **ხელმოწერა / დროის შტამპი:** _სოფია მარტინსი (ციფრულად ხელმოწერილი მმართველობის პორტალით, 2026-04-15 14:32 UTC)_
- ** შემდგომი მფლობელები: ** Device Lab Ops (მიწოდება DR-2026-05-Q2 მტკიცებულებათა ნაკრები 2026-05-31 წლამდე)
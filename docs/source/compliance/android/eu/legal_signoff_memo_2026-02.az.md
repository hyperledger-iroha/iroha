---
lang: az
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

# AND6 AB Legal Sign-off Memo — 2026.1 GA (Android SDK)

## Xülasə

- ** Buraxılış / Qatar:** 2026.1 GA (Android SDK)
- **İnceləmə tarixi:** 2026-04-15
- **Məsləhətçi / Rəyçi:** Sofia Martins — Uyğunluq və Hüquq
- **Əhatə dairəsi:** ETSI EN 319 401 təhlükəsizlik hədəfi, GDPR DPIA xülasəsi, SBOM attestasiyası, AND6 cihaz-laboratoriya fövqəladə hal sübutu
- **Əlaqədar biletlər:** `_android-device-lab` / AND6-DR-202602, AND6 idarəetmə izləyicisi (`GOV-AND6-2026Q1`)

## Artefakt Yoxlama Siyahısı

| Artefakt | SHA-256 | Məkan / Link | Qeydlər |
|----------|---------|-----------------|-------|
| `security_target.md` | `385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0` | `docs/source/compliance/android/eu/security_target.md` | 2026.1 GA buraxılış identifikatorlarına və təhlükə modeli deltalarına uyğun gəlir (Torii NRPC əlavələri). |
| `gdpr_dpia_summary.md` | `8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5` | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | İstinadlar AND7 telemetriya siyasəti (`docs/source/sdk/android/telemetry_redaction.md`). |
| `sbom_attestation.md` | `c2e0de176d4bb8c8e09329e2b9ee5dd93228d3f0def78225c1d8b777a5613f2d` | `docs/source/compliance/android/eu/sbom_attestation.md` + Sigstore paketi (`android-sdk-release#4821`). | CycloneDX + mənşəyə baxıldı; Buildkite işinə `android-sdk-release#4821` uyğun gəlir. |
| Sübut jurnalı | `0b2d2f9eddada06faa70620f608c3ad1ec38f378d2cbddc24b15d0a83fcc381d` | `docs/source/compliance/android/evidence_log.csv` (sətir `android-device-lab-failover-20260220`) | Qeydiyyatdan keçmiş paket hashlərini + tutumlu snapshot + yaddaş girişini təsdiqləyir. |
| Cihaz laboratoriyası ehtiyat paketi | `faf32356dfc0bbca1459b14d75f3306ea1c10cb40f3180fe1758ac5105016f85` | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | `bundle-manifest.json`-dən götürülmüş hash; bilet AND6-DR-202602 Qanuna/Uyğunluğa təhvil verilməsini qeyd etdi. |

## Tapıntılar və İstisnalar

- Heç bir bloklama problemi müəyyən edilmədi. Artefaktlar ETSI/GDPR tələblərinə uyğundur; AND7 telemetriya pariteti DPIA xülasəsində qeyd edildi və heç bir əlavə azalma tələb olunmur.
- Tövsiyə: planlaşdırılan DR-2026-05-Q2 təliminə (bilet AND6-DR-202605) nəzarət edin və nəticədə əldə olunan paketi növbəti idarəetmə yoxlama məntəqəsindən əvvəl sübut jurnalına əlavə edin.

## Təsdiq

- **Qərar:** Təsdiq edildi
- **İmza / Vaxt möhürü:** _Sofia Martins (idarəetmə portalı vasitəsilə rəqəmsal imzalanıb, 2026-04-15 14:32 UTC)_
- **İzləyici sahiblər:** Cihaz Laboratoriyası Əməliyyatları (DR-2026-05-Q2 sübut paketini 31-05-2026 tarixindən əvvəl çatdırın)
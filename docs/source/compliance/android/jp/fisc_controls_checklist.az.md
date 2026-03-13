---
lang: az
direction: ltr
source: docs/source/compliance/android/jp/fisc_controls_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2d8b4c90c94dddd8118fcb9c55f07c25000c6dab1f8d239570402023ab89e844
source_last_modified: "2025-12-29T18:16:35.928660+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# FISC Təhlükəsizlik Nəzarətləri Yoxlama Siyahısı — Android SDK

| Sahə | Dəyər |
|-------|-------|
| Versiya | 0,1 (2026-02-12) |
| Əhatə dairəsi | Yapon maliyyə yerləşdirmələrində istifadə edilən Android SDK + operator alətləri |
| Sahiblər | Uyğunluq və Hüquq (Daniel Park), Android Proqram Rəhbəri |

## Nəzarət Matrisi

| FISC Nəzarəti | İcra Təfərrüatı | Sübut / İstinadlar | Status |
|-------------|-----------------------|-----------------------|--------|
| **Sistem konfiqurasiyasının bütövlüyü** | `ClientConfig` manifest hashing, şemanın yoxlanılması və yalnız oxumaq üçün iş vaxtı girişini tətbiq edir. Konfiqurasiyanın yenidən yüklənməsi uğursuzluqları runbook-da sənədləşdirilmiş `android.telemetry.config.reload` hadisələrini yayır. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2. | ✅ Həyata keçirilən |
| **Giriş nəzarəti və autentifikasiyası** | SDK Torii TLS siyasətlərini və `/v2/pipeline` imzalanmış sorğuları qəbul edir; operator iş axınları arayışı eskalasiya üçün Dəstək Playbook §4–5 və imzalanmış Norito artefaktları vasitəsilə qapını ləğv edin. | `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_redaction.md` (iş axını ləğv edin). | ✅ Həyata keçirilən |
| **Kriptoqrafik açarların idarə edilməsi** | StrongBox-un üstünlük verdiyi provayderlər, attestasiyanın yoxlanılması və cihaz matrisinin əhatə dairəsi KMS uyğunluğunu təmin edir. Attestasiya qoşqu çıxışları `artifacts/android/attestation/` altında arxivləşdirilmiş və hazırlıq matrisində izlənilmişdir. | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`. | ✅ Həyata keçirilən |
| **Giriş, monitorinq və saxlama** | Telemetriya redaksiyası siyasəti həssas datanı heşləyir, cihaz atributlarını yığır və saxlanmasını tətbiq edir (7/30/90/365 günlük pəncərələr). Support Playbook §8 tablosunun hədlərini təsvir edir; `telemetry_override_log.md`-də qeydə alınan ləğvetmələr. | `docs/source/sdk/android/telemetry_redaction.md`; `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_override_log.md`. | ✅ Həyata keçirilən |
| **Əməliyyatlar və dəyişikliklərin idarə edilməsi** | GA kəsmə proseduru (Dəstək Playbook §7.2) üstəgəl `status.md` yeniləmələri buraxılışa hazırlığı izləyir. `docs/source/compliance/android/eu/sbom_attestation.md` vasitəsilə əlaqələndirilmiş sübutlar (SBOM, Sigstore paketləri). | `docs/source/android_support_playbook.md`; `status.md`; `docs/source/compliance/android/eu/sbom_attestation.md`. | ✅ Həyata keçirilən |
| **Hadisə cavabı və hesabat** | Playbook ciddilik matrisini, SLA cavab pəncərələrini və uyğunluq bildiriş addımlarını müəyyən edir; telemetriya ləğvi + xaos məşqləri pilotlar qarşısında təkrarlanmanı təmin edir. | `docs/source/android_support_playbook.md` §§4–9; `docs/source/sdk/android/telemetry_chaos_checklist.md`. | ✅ Həyata keçirilən |
| **Data rezidentliyi/lokallaşdırma** | JP yerləşdirmələri üçün telemetriya kollektorları təsdiqlənmiş Tokio bölgəsində işləyir; StrongBox attestasiya paketləri regionda saxlanılır və tərəfdaş biletlərdən istinad edilir. Lokallaşdırma planı sənədlərin betadan əvvəl Yapon dilində mövcud olmasını təmin edir (AND5). | `docs/source/android_support_playbook.md` §9; `docs/source/sdk/android/developer_experience_plan.md` §5; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`. | 🈺 Davam edir (lokallaşdırma davam edir) |

## Rəyçi qeydləri

- Tənzimlənən partnyor işə başlamazdan əvvəl Galaxy S23/S24 üçün cihaz-matris daxiletmələrini yoxlayın (`s23-strongbox-a`, `s24-strongbox-a` hazırlıq sənədi sıralarına baxın).
- JP yerləşdirmələrində telemetriya kollektorlarının DPIA-da (`docs/source/compliance/android/eu/gdpr_dpia_summary.md`) müəyyən edilmiş eyni saxlama/dəyişmə məntiqini tətbiq etməsinə əmin olun.
- Bank tərəfdaşları bu yoxlama siyahısını nəzərdən keçirdikdən sonra kənar auditorlardan təsdiqi əldə edin.
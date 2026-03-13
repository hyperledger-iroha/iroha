---
lang: az
direction: ltr
source: docs/source/compliance/android/eu/security_target.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0
source_last_modified: "2025-12-29T18:16:35.927510+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android SDK Təhlükəsizlik Hədəfi — ETSI EN 319 401 Alignment

| Sahə | Dəyər |
|-------|-------|
| Sənəd Versiya | 0,1 (2026-02-12) |
| Əhatə dairəsi | Android SDK (`java/iroha_android/` altında müştəri kitabxanaları plus dəstəkləyən skriptlər/sənədlər) |
| Sahibi | Uyğunluq və Hüquq (Sofia Martins) |
| Rəyçilər | Android Proqram Rəhbəri, Release Engineering, SRE Governance |

## 1. TOE təsviri

Qiymətləndirmə Hədəfi (TOE) Android SDK kitabxana kodundan (`java/iroha_android/src/main/java`), onun konfiqurasiya səthindən (`ClientConfig` + Norito qəbulu) və `roadmap.md` və miltonlar VƏ67 üçün istinad edilən əməliyyat alətindən ibarətdir.

Əsas komponentlər:

1. **Konfiqurasiya qəbulu** — Yaradılmış `iroha_config` manifestindən `ClientConfig` başlıqları Torii son nöqtələri, TLS siyasətləri, təkrar cəhdlər və telemetriya qarmaqları və başlanğıcdan sonrakı dəyişməzliyi (II108010) tətbiq edir.
2. **Açarların idarə edilməsi / StrongBox** — Aparatla dəstəklənən imzalanma `docs/source/sdk/android/key_management.md` sənədində sənədləşdirilmiş siyasətlərlə `SystemAndroidKeystoreBackend` və `AttestationVerifier` vasitəsilə həyata keçirilir. Attestasiyanın əldə edilməsi/təsdiqlənməsi `scripts/android_keystore_attestation.sh` və CI köməkçisi `scripts/android_strongbox_attestation_ci.sh` istifadə edir.
3. **Telemetriya və redaksiya** — `docs/source/sdk/android/telemetry_redaction.md`-də təsvir edilən paylaşılan sxem vasitəsilə alətləşdirmə huniləri, heşlənmiş səlahiyyətləri, qutuya alınmış cihaz profillərini ixrac edir və Dəstək Kitabı tərəfindən tətbiq edilən audit qarmaqlarını ləğv edir.
4. **Əməliyyat runbooks** — `docs/source/android_runbook.md` (operator cavabı) və `docs/source/android_support_playbook.md` (SLA + eskalasiya) deterministik ləğvetmələr, xaos təlimləri və sübutların əldə edilməsi ilə TOE-nin əməliyyat izini gücləndirir.
5. ** Buraxılış mənşəyi** — Gradle əsaslı konstruksiyalar `docs/source/sdk/android/developer_experience_plan.md` və AND6 uyğunluq yoxlama siyahısında çəkildiyi kimi CycloneDX plaginindən əlavə təkrarlana bilən qurma bayraqlarından istifadə edir. Buraxılış artefaktları `docs/source/release/provenance/android/`-də imzalanır və çarpaz istinad edilir.

## 2. Aktivlər və Fərziyyələr

| Aktiv | Təsvir | Təhlükəsizlik Məqsədi |
|-------|-------------|--------------------|
| Konfiqurasiya göstərir | Tətbiqlərlə paylanmış Norito mənşəli `ClientConfig` snapşotları. | İstirahət zamanı orijinallıq, bütövlük və məxfilik. |
| İmza açarları | StrongBox/TEE provayderləri vasitəsilə yaradılan və ya idxal edilən açarlar. | StrongBox üstünlükləri, attestasiya girişi, əsas ixrac yoxdur. |
| Telemetriya axınları | SDK cihazlarından ixrac edilən OTLP izləri/logları/metrikləri. | Pseudonimization (hashed səlahiyyətlilər), minimuma endirilmiş PII, auditi ləğv edin. |
| Ledger qarşılıqlı əlaqələri | Norito faydalı yüklər, qəbul metadata, Torii şəbəkə trafiki. | Qarşılıqlı identifikasiya, təkrara davamlı sorğular, deterministik təkrar cəhdlər. |

Fərziyyələr:

- Mobil ƏS standart sandboxing + SELinux təmin edir; StrongBox cihazları Google-un keymaster interfeysini həyata keçirir.
- Operatorlar Torii son nöqtələrini şuranın etibar etdiyi CA-lar tərəfindən imzalanmış TLS sertifikatları ilə təmin edir.
- Maven-də dərc etməzdən əvvəl yenidən istehsal oluna bilən tikinti tələblərinə cavab verən infrastruktur qurun.

## 3. Təhdidlər və Nəzarətlər| Təhdid | Nəzarət | Sübut |
|--------|---------|----------|
| Təhlükəli konfiqurasiya | `ClientConfig` tətbiq etməzdən əvvəl manifestləri (hesh + sxem) doğrulayır və `android.telemetry.config.reload` vasitəsilə rədd edilmiş yenidən yükləmələri qeyd edir. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2. |
| İmza açarlarının kompromisi | StrongBox üçün tələb olunan siyasətlər, sertifikatlaşdırma alətləri və cihaz matrisi auditləri sürüşməni müəyyən edir; hadisəyə görə sənədləşdirilən üstələyir. | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`. |
| Telemetriyada PII sızması | Blake2b-hashed səlahiyyətlilər, paketlənmiş cihaz profilləri, daşıyıcının buraxılması, qeydi ləğv edin. | `docs/source/sdk/android/telemetry_redaction.md`; Dəstək Playbook §8. |
| Torii RPC |-də təkrar oxuyun və ya səviyyəsini azaldın `/v2/pipeline` sorğu qurucusu TLS sancma, səs-küy kanalı siyasəti və heş edilmiş səlahiyyət konteksti ilə yenidən cəhd büdcələrini tətbiq edir. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ToriiRequestBuilder.java`; `docs/source/sdk/android/networking.md` (planlaşdırılıb). |
| İmzasız və ya təkrar olunmayan buraxılışlar | CycloneDX SBOM + AND6 yoxlama siyahısı ilə təsdiqlənmiş Sigstore sertifikatları; buraxılış RFC-ləri `docs/source/release/provenance/android/`-də sübut tələb edir. | `docs/source/sdk/android/developer_experience_plan.md`; `docs/source/compliance/android/eu/sbom_attestation.md`. |
| Natamam hadisələrin idarə edilməsi | Runbook + playbook overrides, xaos təlimləri və eskalasiya ağacını müəyyən edir; telemetriya ləğvi üçün imzalanmış Norito sorğuları tələb olunur. | `docs/source/android_runbook.md`; `docs/source/android_support_playbook.md`. |

## 4. Qiymətləndirmə Fəaliyyətləri

1. **Dizayn baxışı** — Uyğunluq + SRE konfiqurasiya, əsas idarəetmə, telemetriya və buraxılış nəzarətlərinin ETSI təhlükəsizlik məqsədlərinə uyğunluğunu yoxlayır.
2. **İcra yoxlamaları** — Avtomatlaşdırılmış testlər:
   - `scripts/android_strongbox_attestation_ci.sh` matrisdə sadalanan hər bir StrongBox cihazı üçün tutulan paketləri yoxlayır.
   - `scripts/check_android_samples.sh` və Managed Device CI nümunə proqramların `ClientConfig`/telemetri müqavilələrinə əməl etməsini təmin edir.
3. **Əməliyyat yoxlaması** — `docs/source/sdk/android/telemetry_chaos_checklist.md` üzrə rüblük xaos məşqləri (redasiya + ləğvetmə məşqləri).
4. **Dəlillərin saxlanması** — `docs/source/compliance/android/` (bu qovluq) altında saxlanılan və `status.md`-dən istinad edilən artefaktlar.

## 5. ETSI EN 319 401 Xəritəçəkmə| EN 319 401 Maddə | SDK Nəzarəti |
|----------------------|-------------|
| 7.1 Təhlükəsizlik siyasəti | Bu təhlükəsizlik hədəfi + Support Playbook-da sənədləşdirilmişdir. |
| 7.2 Təşkilat təhlükəsizliyi | Support Playbook §2-də RACI + zəng üzrə sahiblik. |
| 7.3 Aktivlərin idarə edilməsi | Yuxarıda §2-də müəyyən edilmiş konfiqurasiya, əsas və telemetriya aktivinin məqsədləri. |
| 7.4 Girişə nəzarət | StrongBox siyasətləri + imzalanmış Norito artefaktları tələb edən iş axınını ləğv edin. |
| 7.5 Kriptoqrafik nəzarət | AND2-dən açarın yaradılması, saxlanması və sertifikatlaşdırma tələbləri (əsas idarəetmə təlimatı). |
| 7.6 Əməliyyatların təhlükəsizliyi | Telemetriya heşinqi, xaos məşqləri, insidentlərə reaksiya və sübutların buraxılması. |
| 7.7 Rabitə təhlükəsizliyi | `/v2/pipeline` TLS siyasəti + hashed səlahiyyətlilər (telemetriya redaksiya sənədi). |
| 7.8 Sistemin əldə edilməsi / inkişafı | AND5/AND6 planlarında təkrarlanan Gradle konstruksiyaları, SBOM-lar və mənşəli qapılar. |
| 7.9 Təchizatçı əlaqələri | Buildkite + Sigstore sertifikatları üçüncü tərəfdən asılılıq SBOM-ları ilə birlikdə qeydə alınıb. |
| 7.10 Hadisələrin idarə edilməsi | Runbook/Playbook eskalasiyası, qeydi ləğv etmək, telemetriya uğursuz sayğacları. |

## 6. Baxım

- SDK yeni kriptoqrafik alqoritmlər, telemetriya kateqoriyaları təqdim etdikdə və ya avtomatlaşdırma dəyişikliklərini buraxdıqda bu sənədi yeniləyin.
- İmzalanmış nüsxələri `docs/source/compliance/android/evidence_log.csv`-də SHA-256 həzmləri və rəyçinin imzaları ilə əlaqələndirin.
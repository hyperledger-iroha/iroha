---
lang: az
direction: ltr
source: docs/source/android_release_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ee3613b544a847953f5ec152092cb2fe1da35279c5482486513d6b8d6dddf02
source_last_modified: "2026-01-05T09:28:11.999717+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Buraxılış Yoxlama Siyahısı (AND6)

Bu yoxlama siyahısı **AND6 — CI & Compliance Hardening** qapılarını əhatə edir
`roadmap.md` (§Prioritet 5). Android SDK buraxılışlarını Rust ilə uyğunlaşdırır
CI işlərini, uyğunluq artefaktlarını ifadə edərək RFC gözləntilərini buraxın,
GA-dan əvvəl əlavə edilməli olan cihaz-laboratoriya sübutları və mənşə paketləri,
LTS və ya düzəliş qatarı irəliləyir.

Bu sənədlə birlikdə istifadə edin:

- `docs/source/android_support_playbook.md` — buraxılış təqvimi, SLAs və
  eskalasiya ağacı.
- `docs/source/android_runbook.md` — gündəlik əməliyyat kitabçaları.
- `docs/source/compliance/android/and6_compliance_checklist.md` — tənzimləyici
  artefakt inventar.
- `docs/source/release_dual_track_runbook.md` — iki yollu buraxılış idarəetməsi.

## 1. Bir Baxışda Səhnə Qapıları

| Mərhələ | Tələb olunan Qapılar | Sübut |
|-------|----------------|----------|
| **T−7 gün (öncədən dondurma)** | 14 gün ərzində gecə `ci/run_android_tests.sh` yaşıl; `ci/check_android_fixtures.sh`, `ci/check_android_samples.sh` və `ci/check_android_docs_i18n.sh` keçid; lint/asılılıq skanları növbəyə alınıb. | Buildkite idarə panelləri, qurğu fərqi hesabatı, nümunə skrinşot şəkilləri. |
| **T−3 gün (RC təşviqi)** | Cihaz-laboratoriya rezervasiyası təsdiqləndi; StrongBox attestation CI run (`scripts/android_strongbox_attestation_ci.sh`); Planlaşdırılmış avadanlıqda həyata keçirilən robotelektrik/cihazlı dəstlər; `./gradlew lintRelease ktlintCheck detekt dependencyGuard` təmiz. | Cihaz matrisi CSV, attestasiya paketi manifest, `artifacts/android/lint/<version>/` altında arxivləşdirilmiş Gradle hesabatları. |
| **T−1 gün (get/getməz)** | Telemetriya redaksiyası statusu paketi yeniləndi (`scripts/telemetry/check_redaction_status.py --write-cache`); uyğunluq artefaktları `and6_compliance_checklist.md` üzrə yenilənir; mənşəli məşq tamamlandı (`scripts/android_sbom_provenance.sh --dry-run`). | `docs/source/compliance/android/evidence_log.csv`, telemetriya statusu JSON, mənşəli quru iş jurnalı. |
| **T0 (GA/LTS kəsilməsi)** | `scripts/publish_android_sdk.sh --dry-run` tamamlandı; mənşə + SBOM imzalanmışdır; buraxılış yoxlama siyahısı ixrac edilmiş və getmək/no-go dəqiqələrinə əlavə edilmişdir; `ci/sdk_sorafs_orchestrator.sh` tüstü işi yaşıl. | RFC qoşmalarını, Sigstore paketini, `artifacts/android/` altında övladlığa götürmə artefaktlarını buraxın. |
| **T+1 gün (kəsikdən sonra)** | Düzəliş hazırlığı təsdiqləndi (`scripts/publish_android_sdk.sh --validate-bundle`); idarə paneli fərqləri nəzərdən keçirildi (`ci/check_android_dashboard_parity.sh`); sübut paketi `status.md`-ə yüklənmişdir. | Dashboard fərq ixracı, `status.md` girişinə keçid, arxivləşdirilmiş buraxılış paketi. |

## 2. CI & Keyfiyyət Qapısı Matrisi| Qapı | Əmr(lər) / Skript | Qeydlər |
|------|--------------------|-------|
| Vahid + inteqrasiya testləri | `ci/run_android_tests.sh` (`ci/run_android_tests.sh` sarılır) | `artifacts/android/tests/test-summary.json` + test jurnalını buraxır. Norito kodek, növbə, StrongBox ehtiyatı və Torii müştəri qoşqu testləri daxildir. Gecə və etiketləmədən əvvəl tələb olunur. |
| Fikstur pariteti | `ci/check_android_fixtures.sh` (`scripts/check_android_fixtures.py` sarar) | Yenilənmiş Norito qurğularının Rust kanonik dəstinə uyğun olmasını təmin edir; qapı uğursuz olduqda JSON fərqini əlavə edin. |
| Nümunə proqramlar | `ci/check_android_samples.sh` | `examples/android/{operator-console,retail-wallet}` qurur və `scripts/android_sample_localization.py` vasitəsilə lokallaşdırılmış ekran görüntülərini təsdiqləyir. |
| Sənədlər/I18N | `ci/check_android_docs_i18n.sh` | README + lokallaşdırılmış sürətli başlanğıcları qoruyur. Sənəd redaktələri buraxılış filialına düşdükdən sonra yenidən işə salın. |
| Dashboard pariteti | `ci/check_android_dashboard_parity.sh` | CI/ixrac edilmiş ölçülərin Rust analoqları ilə uyğunluğunu təsdiq edir; T+1 yoxlaması zamanı tələb olunur. |
| SDK qəbul tüstü | `ci/sdk_sorafs_orchestrator.sh` | Cari SDK ilə çoxmənbəli Sorafs orkestrator bağlamalarını həyata keçirir. Mərhələli artefaktları yükləməzdən əvvəl tələb olunur. |
| Attestasiyanın yoxlanılması | `scripts/android_strongbox_attestation_ci.sh --summary-out artifacts/android/attestation/ci-summary.json` | StrongBox/TEE attestasiya paketlərini `artifacts/android/attestation/**` altında birləşdirir; xülasəni GA paketlərinə əlavə edin. |
| Cihaz laboratoriya yuvasının yoxlanılması | `scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab/<slot> --json-out artifacts/android/device_lab/summary.json` | Paketləri buraxmaq üçün sübut əlavə etməzdən əvvəl cihaz paketlərini təsdiq edir; CI `fixtures/android/device_lab/slot-sample`-də nümunə yuvasına qarşı işləyir (telemetri/attestasiya/növbə/loqlar + `sha256sum.txt`). |

> **İpucu:** bu işləri `android-release` Buildkite boru kəmərinə əlavə edin ki,
> dondurma həftələri avtomatik olaraq hər qapını buraxma filialının ucu ilə yenidən işə salın.

Konsolidasiya edilmiş `.github/workflows/android-and6.yml` işi tüyləri idarə edir,
hər bir PR/push-da test dəsti, attestasiya-xülasə və cihaz-laboratoriya yuvası yoxlamaları
Android mənbələrinə toxunmaq, `artifacts/android/{lint,tests,attestation,device_lab}/` altında sübut yükləmək.

## 3. Lint & Dependency Scans

Repo kökündən `scripts/android_lint_checks.sh --version <semver>`-i işə salın. The
skript icra edir:

```
lintRelease ktlintCheck detekt dependencyGuardBaseline \
:operator-console:lintRelease :retail-wallet:lintRelease
```

- Hesabatlar və asılılıq-mühafizə çıxışları altında arxivləşdirilir
  `artifacts/android/lint/<label>/` və buraxılış üçün `latest/` simvolik əlaqə
  boru kəmərləri.
- Uğursuz lint tapıntıları ya düzəliş, ya da buraxılışda qeyd tələb edir
  Qəbul edilmiş riski sənədləşdirən RFC (Release Engineering + Proqramı tərəfindən təsdiq edilmişdir
  aparıcı).
- `dependencyGuardBaseline` asılılıq kilidini bərpa edir; fərqi əlavə edin
  get/getmə paketinə.

## 4. Cihaz laboratoriyası və StrongBox əhatə dairəsi

1. Burada istinad edilən tutum izləyicisindən istifadə edərək Pixel + Galaxy cihazlarını ehtiyata qoyun
   `docs/source/compliance/android/device_lab_contingency.md`. Buraxılışları bloklayır
   əlçatanlıq `.
3. Cihaz matrisini işə salın (cihazda paket/ABI siyahısını sənədləşdirin
   izləyici). Yenidən cəhdlər uğurlu olsa belə, insident jurnalında uğursuzluqları qeyd edin.
4. Firebase Test Laboratoriyasına geri qayıtmaq tələb olunarsa, bilet təqdim edin; bileti bağlayın
   aşağıdakı yoxlama siyahısında.

## 5. Uyğunluq və Telemetriya Artefaktları- AB üçün `docs/source/compliance/android/and6_compliance_checklist.md`-i izləyin
  və JP təqdimatları. `docs/source/compliance/android/evidence_log.csv` yeniləyin
  hash + Buildkite iş URL-ləri ilə.
- Vasitəsilə telemetriya redaksiyası sübutlarını yeniləyin
  `scripts/telemetry/check_redaction_status.py --write-cache \
   --status-url https://android-observability.example/status.json`.
  Yaranan JSON-u altında saxlayın
  `artifacts/android/telemetry/<version>/status.json`.
- Sxem fərqinin çıxışını qeyd edin
  `scripts/telemetry/run_schema_diff.sh --android-config ... --rust-config ...`
  Rust ixracatçıları ilə bərabərliyi sübut etmək.

## 6. Mənbə, SBOM və Nəşriyyat

1. Nəşr xəttini qurudan idarə edin:

   ```bash
   scripts/publish_android_sdk.sh \
     --version <semver> \
     --repo-dir artifacts/android/maven/<semver> \
     --dry-run
   ```

2. SBOM + Sigstore mənşəyi yaradın:

   ```bash
   scripts/android_sbom_provenance.sh \
     --version <semver> \
     --out artifacts/android/provenance/<semver>
   ```

3. `artifacts/android/provenance/<semver>/manifest.json` əlavə edin və imzalayın
   `checksums.sha256` buraxılış RFC üçün.
4. Həqiqi Maven repozitoriyasına irəliləyərkən təkrar işə salın
   `scripts/publish_android_sdk.sh`, `--dry-run` olmadan, konsolu çəkin
   daxil edin və əldə edilən artefaktları `artifacts/android/maven/<semver>`-ə yükləyin.

## 7. Göndərmə Paket Şablonu

Hər GA/LTS/düzeltmə buraxılışına aşağıdakılar daxil olmalıdır:

1. **Tamamlanmış yoxlama siyahısı** — bu faylın cədvəlini kopyalayın, hər bir elementi işarələyin və keçid edin
   dəstəkləyici artefaktlara (Buildkite run, logs, doc diffs).
2. **Cihazın laboratoriya sübutu** — attestasiya hesabatının xülasəsi, rezervasiya jurnalı və
   hər hansı fövqəladə aktivləşdirmələr.
3. **Telemetri paketi** — redaksiya statusu JSON, sxem fərqi, keçid
   `docs/source/sdk/android/telemetry_redaction.md` yeniləmələri (əgər varsa).
4. **Uyğunluq artefaktları** — uyğunluq qovluğuna əlavə edilmiş/yenilənilən qeydlər
   üstəlik yenilənmiş sübut jurnalı CSV.
5. **Provenance paketi** — SBOM, Sigstore imzası və `checksums.sha256`.
6. **Buraxılış xülasəsi** — `status.md` xülasəsinə əlavə edilmiş bir səhifəlik icmal
   yuxarıdakılar (tarix, versiya, hər hansı imtina edilmiş qapıların vurğulanması).

Paketi `artifacts/android/releases/<version>/` altında saxlayın və ona istinad edin
`status.md` və buraxılış RFC-də.

- `scripts/run_release_pipeline.py --publish-android-sdk ...` avtomatik
  ən son lint arxivini (`artifacts/android/lint/latest`) və
  uyğunluq sübutu `artifacts/android/releases/<version>/`-ə daxil olun
  təqdim paketi həmişə kanonik bir yerə malikdir.

---

**Xatırlatma:** yeni CI işləri, uyğunluq artefaktları,
və ya telemetriya tələbləri əlavə edilir. Yol xəritəsi elementi AND6 olana qədər açıq qalır
yoxlama siyahısı və əlaqəli avtomatlaşdırma iki ardıcıl buraxılış üçün sabitdir
qatarlar.
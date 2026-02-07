---
lang: az
direction: ltr
source: docs/automation/android/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 676798a4cf7c3e7737a0f80640f3f268a2f625f92afdd359ac528881d2aeb046
source_last_modified: "2025-12-29T18:16:35.060950+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android Documentation Automation Baseline (AND5)

Yol xəritəsinin AND5 bəndi sənədləşdirmə, lokallaşdırma və dərc etməyi tələb edir
avtomatlaşdırma AND6 (CI və Uyğunluq) başlamazdan əvvəl yoxlanıla bilər. Bu qovluq
AND5/AND6-nın istinad etdiyi əmrləri, artefaktları və sübut planını qeyd edir,
ələ keçirilən planları əks etdirir
`docs/source/sdk/android/developer_experience_plan.md` və
`docs/source/sdk/android/parity_dashboard_plan.md`.

## Boru Kəmərləri və Əmrlər

| Tapşırıq | Əmr(lər) | Gözlənilən Artefaktlar | Qeydlər |
|------|------------|--------------------|-------|
| Lokallaşdırma stub sinxronizasiyası | `python3 scripts/sync_docs_i18n.py` (istəyə görə hər qaçış üçün `--lang <code>` keçir) | `docs/automation/android/i18n/<timestamp>-sync.log` altında saxlanılan log faylı üstəgəl tərcümə edilmiş stub öhdəliyi | `docs/i18n/manifest.json`-i tərcümə edilmiş stublarla sinxronlaşdırır; log toxunulan dil kodlarını və bazada ələ keçirilən git öhdəliyini qeyd edir. |
| Norito qurğusu + paritetin yoxlanılması | `ci/check_android_fixtures.sh` (`python3 scripts/check_android_fixtures.py --json-out artifacts/android/parity/<stamp>/summary.json` sarılır) | Yaradılmış xülasə JSON-u `docs/automation/android/parity/<stamp>-summary.json` |-ə kopyalayın `java/iroha_android/src/test/resources` faydalı yükləri, manifest heşlərini və imzalanmış qurğu uzunluqlarını yoxlayır. Xülasəni `artifacts/android/fixture_runs/` altında kadans sübutu ilə birlikdə əlavə edin. |
| Nümunə manifest və nəşr sübutu | `scripts/publish_android_sdk.sh --version <semver> [--repo-url …]` (testləri işlədir + SBOM + mənşəyi) | Provenance paketi metadata üstəgəl `docs/source/sdk/android/samples/`-dən əldə edilən `sample_manifest.json` `docs/automation/android/samples/<version>/` altında saxlanılır | AND5 nümunə proqramlarını birləşdirin və avtomatlaşdırmanı birlikdə buraxın—beta baxışı üçün yaradılan manifest, SBOM hash və mənşə jurnalını çəkin. |
| Paritet tablosunun lenti | `python3 scripts/check_android_fixtures.py … --json-out artifacts/android/parity/<stamp>/summary.json`, ardından `python3 scripts/android_parity_metrics.py --summary <summary> --output artifacts/android/parity/<stamp>/metrics.prom` | `metrics.prom` snapşotunu və ya Grafana ixrac JSON-u `docs/automation/android/parity/<stamp>-metrics.prom`-ə kopyalayın | AND5/AND7 idarəçiliyi etibarsız təqdimetmə sayğaclarını və telemetriya qəbulunu yoxlaya bilməsi üçün idarə paneli planını təqdim edir. |

## Sübut Tutma

1. **Hər şeyin vaxt damğası.** UTC vaxt damğalarından istifadə edərək faylları adlandırın
   (`YYYYMMDDTHHMMSSZ`) beləliklə paritet tabloları, idarəetmə protokolları və dərc edilmişdir
   sənədlər eyni işə deterministik olaraq istinad edə bilər.
2. **İstinad öhdəliyi götürür.** Hər bir jurnala qaçışın git commit hashı daxil edilməlidir.
   üstəgəl hər hansı müvafiq konfiqurasiya (məsələn, `ANDROID_PARITY_PIPELINE_METADATA`).
   Məxfilik redaktə etməyi tələb etdikdə, qeyd daxil edin və təhlükəsiz anbara keçid daxil edin.
3. **Minimal konteksti arxivləşdirin.** Biz yalnız strukturlaşdırılmış xülasələri yoxlayırıq (JSON,
   `.prom`, `.log`). Ağır artefaktlar (APK paketləri, ekran görüntüləri) içində qalmalıdır
   `artifacts/` və ya jurnalda qeyd edilmiş imzalanmış hash ilə obyekt saxlama.
4. **Status qeydlərini yeniləyin.** `status.md`-də AND5 mərhələləri irəlilədikdə, istinad edin
   müvafiq fayl (məsələn, `docs/automation/android/parity/20260324T010203Z-summary.json`)
   beləliklə, auditorlar CI qeydlərini silmədən baza xəttini izləyə bilərlər.

Bu layoutun ardınca “sənədlər/avtomatlaşdırma əsasları üçün mövcud olan
Audit” şərti AND6-nın Android sənədləşdirmə proqramına istinad edir və saxlayır
nəşr planları ilə lockstep.
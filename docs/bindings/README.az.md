---
lang: az
direction: ltr
source: docs/bindings/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf9773ecd75fc31ee89da58a3c5eda846b910eb6e131f1e042b565892e028f16
source_last_modified: "2025-12-29T18:16:35.062011+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SDK Binding & Fixture Governance

Yol xəritəsindəki WP1-E sənədi saxlamaq üçün kanonik yer kimi “sənədlər/bağlamaları” çağırır.
dillər arası bağlanma vəziyyəti. Bu sənəd məcburi inventar qeyd edir,
regenerasiya əmrləri, sürüşmə qoruyucuları və sübut yerləri beləliklə GPU pariteti
qapıları (WP1-E/F/G) və çarpaz SDK kadans şurasının tək istinadı var.

## Birgə qoruyucu barmaqlıqlar
- **Kanonik oyun kitabçası:** `docs/source/norito_binding_regen_playbook.md` açıqlanır
  Android üçün fırlanma siyasəti, gözlənilən sübut və eskalasiya iş axını,
  Swift, Python və gələcək bağlamalar.
- **Norito sxem pariteti:** `scripts/check_norito_bindings_sync.py` (vasitəsilə çağırılır)
  `scripts/check_norito_bindings_sync.sh` və CI-də qapalı
  `ci/check_norito_bindings_sync.sh`) Rust, Java və ya Python istifadə edildikdə bloklar qurur.
  sxem artefaktlarının sürüşməsi.
- ** Cadence gözətçi:** `scripts/check_fixture_cadence.py` oxuyur
  `artifacts/*_fixture_regen_state.json` faylları və Çərşənbə axşamı/Cümə (Android,
  Python) və Wed (Swift) pəncərələri beləliklə, yol xəritəsi qapılarında yoxlanıla bilən vaxt nişanları var.

## Bağlama matrisi

| Bağlama | Giriş nöqtələri | Quraşdırma / regen əmri | Drift qoruyucuları | Sübut |
|---------|--------------|-------------------------|--------------|----------|
| Android (Java) | `java/iroha_android/` (`java/iroha_android/README.md`) | `scripts/android_fixture_regen.sh` → `artifacts/android_fixture_regen_state.json` | `scripts/check_android_fixtures.py`, `ci/check_android_fixtures.sh`, `java/iroha_android/run_tests.sh` | `artifacts/android/fixture_runs/` |
| Swift (iOS/macOS) | `IrohaSwift/` (`IrohaSwift/README.md`) | `scripts/swift_fixture_regen.sh` (istəyə görə `SWIFT_FIXTURE_ARCHIVE`) → `artifacts/swift_fixture_regen_state.json` | `scripts/check_swift_fixtures.py`, `ci/check_swift_fixtures.sh`, `scripts/swift_fixture_archive.py` | `docs/source/swift_parity_triage.md`, `docs/source/sdk/swift/ios2_fixture_cadence_brief.md` |
| Python | `python/iroha_python/` (`python/iroha_python/README.md`) | `scripts/python_fixture_regen.sh` → `artifacts/python_fixture_regen_state.json` | `scripts/check_python_fixtures.py`, `python/iroha_python/scripts/run_checks.sh` | `docs/source/norito_binding_regen_playbook.md`, `docs/source/sdk/python/connect_end_to_end.md` |
| JavaScript | `javascript/iroha_js/` (`docs/source/sdk/js/publishing.md`) | `npm run release:provenance`, `scripts/js_sbom_provenance.sh`, `scripts/js_signed_staging.sh` | `npm run test`, `javascript/iroha_js/scripts/verify-release-tarball.mjs`, `javascript/iroha_js/scripts/record-release-provenance.mjs` | `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`, `artifacts/js/verification/`, `artifacts/js/sbom/` |

## Bağlama detalları

### Android (Java)
Android SDK `java/iroha_android/` altında yaşayır və kanonik Norito istehlak edir
`scripts/android_fixture_regen.sh` tərəfindən istehsal olunan qurğular. Həmin köməkçi ixrac edir
Rust alətlər silsiləsindəki təzə `.norito` ləkələri, yeniləmələr
`artifacts/android_fixture_regen_state.json` və kadans metadatasını qeyd edir
`scripts/check_fixture_cadence.py` və idarəetmə panelləri istehlak edir. Driftdir
`scripts/check_android_fixtures.py` tərəfindən aşkar edilmişdir (həmçinin
`ci/check_android_fixtures.sh`) və `java/iroha_android/run_tests.sh` tərəfindən,
JNI bağlamalarını, WorkManager növbəsinin təkrarını və StrongBox ehtiyatlarını həyata keçirir.
Fırlanma sübutları, uğursuzluq qeydləri və təkrar transkriptlər altında yaşayır
`artifacts/android/fixture_runs/`.

### Swift (macOS/iOS)
`IrohaSwift/` `scripts/swift_fixture_regen.sh` vasitəsilə eyni Norito faydalı yükləri əks etdirir.
Skript fırlanma sahibini, kadans etiketini və mənbəyi qeyd edir (`live` və `archive`)
`artifacts/swift_fixture_regen_state.json` daxilində və metaməlumatları daxil edir
kadans yoxlayıcısı. `scripts/swift_fixture_archive.py` baxıcılara qəbul etməyə imkan verir
Pasla yaradılan arxivlər; `scripts/check_swift_fixtures.py` və
`ci/check_swift_fixtures.sh` bayt səviyyəli paritet və SLA yaş məhdudiyyətlərini tətbiq edir, eyni zamanda
`scripts/swift_fixture_regen.sh` dərslik üçün `SWIFT_FIXTURE_EVENT_TRIGGER` dəstəkləyir
fırlanmalar. Eskalasiya iş axını, KPI-lər və tablosunda sənədləşdirilmişdir
`docs/source/swift_parity_triage.md` və altındakı kadans brifinqləri
`docs/source/sdk/swift/`.

### Python
Python müştərisi (`python/iroha_python/`) Android qurğularını paylaşır. Qaçış
`scripts/python_fixture_regen.sh` ən son `.norito` faydalı yükləri çəkir, yeniləyir
`python/iroha_python/tests/fixtures/` və kadans metadatasını buraxacaq
`artifacts/python_fixture_regen_state.json` bir dəfə yol xəritəsindən sonrakı ilk fırlanma
tutulur. `scripts/check_python_fixtures.py` və
`python/iroha_python/scripts/run_checks.sh` qapı pytest, mypy, ruff və armatur
yerli və CI-də paritet. Başdan-ayağa sənədlər (`docs/source/sdk/python/…`) və
məcburi regen oyun kitabı Android ilə fırlanmaların necə əlaqələndirilməsini təsvir edir
sahibləri.

### JavaScript
`javascript/iroha_js/` yerli `.norito` fayllarına etibar etmir, lakin WP1-E izləyir
onun buraxılış sübutu beləliklə GPU CI zolaqları tam mənşəyi miras alır. Hər buraxılış
mənşəyi `npm run release:provenance` vasitəsilə əldə edir (gücləndirici
`javascript/iroha_js/scripts/record-release-provenance.mjs`), yaradır və işarələyir
SBOM paketləri `scripts/js_sbom_provenance.sh` ilə imzalanmış quru iş rejimində işləyir
(`scripts/js_signed_staging.sh`) və reyestr artefaktını təsdiqləyir
`javascript/iroha_js/scripts/verify-release-tarball.mjs`. Nəticədə metadata
torpaqlar `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`,
Deterministik təmin edən `artifacts/js/sbom/` və `artifacts/js/verification/`
yol xəritəsi JS5/JS6 və WP1-F benchmark sınaqları üçün sübut. Nəşriyyat kitabı
`docs/source/sdk/js/` avtomatlaşdırmanı birləşdirir.
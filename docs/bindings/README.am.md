---
lang: am
direction: ltr
source: docs/bindings/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf9773ecd75fc31ee89da58a3c5eda846b910eb6e131f1e042b565892e028f16
source_last_modified: "2025-12-29T18:16:35.062011+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# ኤስዲኬ አስገዳጅ እና ቋሚ አስተዳደር

WP1-E በመንገድ ካርታው ላይ "ሰነዶች / ማሰሪያዎችን" እንደ ቀኖናዊ ቦታ ይጠራዋል
የቋንቋ ትስስር ሁኔታ. ይህ ሰነድ አስገዳጅ ዕቃዎችን ይመዘግባል ፣
የማደስ ትእዛዞች፣ ተንሳፋፊ ጠባቂዎች እና የማስረጃ ቦታዎች ስለዚህ የጂፒዩ እኩልነት
በሮች (WP1-E/F/G) እና መስቀል-ኤስዲኬ ካዳንስ ምክር ቤት አንድ ማጣቀሻ አላቸው።

## የጋራ መከላከያ መንገዶች
- ** ቀኖናዊ የመጫወቻ መጽሐፍ:** `docs/source/norito_binding_regen_playbook.md` ይጽፋል
  የማዞሪያ ፖሊሲው፣ የሚጠበቀው ማስረጃ እና የማሳደግ የስራ ሂደት ለአንድሮይድ፣
  ስዊፍት፣ Python እና የወደፊት ማሰሪያዎች።
- ** Norito የመርሃግብር እኩልነት፡** `scripts/check_norito_bindings_sync.py` (የተጠራው በ
  `scripts/check_norito_bindings_sync.sh` እና በ CI የተከለለ በ
  `ci/check_norito_bindings_sync.sh`) ዝገቱ፣ ጃቫ ወይም ፓይዘን ሲገነቡ ያግዳል።
  schema artefacts ተንሳፋፊ.
- ** Cadence ጠባቂ: ** `scripts/check_fixture_cadence.py` ያነባል
  `artifacts/*_fixture_regen_state.json` ፋይሎች እና ማክሰኞ/አርብ (አንድሮይድ፣
  ፓይዘን) እና ዊድ (ስዊፍት) መስኮቶች ስለዚህ የመንገድ ካርታ በሮች ሊመረመሩ የሚችሉ የጊዜ ማህተሞች አሏቸው።

## አስገዳጅ ማትሪክስ

| ማሰር | የመግቢያ ነጥቦች | ቋሚ / regen ትዕዛዝ | ተንሸራታች ጠባቂዎች | ማስረጃ |
|--------|-------------|------------|------------|
| አንድሮይድ (ጃቫ) | `java/iroha_android/` (`java/iroha_android/README.md`) | `scripts/android_fixture_regen.sh` → `artifacts/android_fixture_regen_state.json` | `scripts/check_android_fixtures.py`፣ `ci/check_android_fixtures.sh`፣ `java/iroha_android/run_tests.sh` | `artifacts/android/fixture_runs/` |
| ስዊፍት (iOS/macOS) | `IrohaSwift/` (`IrohaSwift/README.md`) | `scripts/swift_fixture_regen.sh` (በአማራጭ I18NI0000020X) → I18NI0000021X | `scripts/check_swift_fixtures.py`፣ `ci/check_swift_fixtures.sh`፣ `scripts/swift_fixture_archive.py` | `docs/source/swift_parity_triage.md`, `docs/source/sdk/swift/ios2_fixture_cadence_brief.md` |
| Python | `python/iroha_python/` (`python/iroha_python/README.md`) | `scripts/python_fixture_regen.sh` → `artifacts/python_fixture_regen_state.json` | `scripts/check_python_fixtures.py`, `python/iroha_python/scripts/run_checks.sh` | `docs/source/norito_binding_regen_playbook.md`, `docs/source/sdk/python/connect_end_to_end.md` |
| JavaScript | `javascript/iroha_js/` (`docs/source/sdk/js/publishing.md`) | `npm run release:provenance`፣ `scripts/js_sbom_provenance.sh`፣ `scripts/js_signed_staging.sh` | `npm run test`፣ `javascript/iroha_js/scripts/verify-release-tarball.mjs`፣ `javascript/iroha_js/scripts/record-release-provenance.mjs` | `artifacts/js-sdk-provenance/`፣ `artifacts/js/npm_staging/`፣ `artifacts/js/verification/`፣ `artifacts/js/sbom/` |

## አስገዳጅ ዝርዝሮች

### አንድሮይድ (ጃቫ)
አንድሮይድ ኤስዲኬ በ`java/iroha_android/` ስር ይኖራል እና ቀኖናዊውን Norito ይበላል
በ I18NI0000048X የተሰሩ እቃዎች. ያ ረዳት ወደ ውጭ ይልካል።
ትኩስ `.norito` ብሎብስ ከዝገት መሣሪያ ሰንሰለት፣ ዝማኔዎች
`artifacts/android_fixture_regen_state.json`፣ እና ያንን የcadance ሜታዳታ ይመዘግባል
`scripts/check_fixture_cadence.py` እና የአስተዳደር ዳሽቦርዶች ይበላሉ. መንሸራተት ነው።
በ`scripts/check_android_fixtures.py` የተገኘ (በተጨማሪም ወደ ውስጥ ተሽሯል
`ci/check_android_fixtures.sh`) እና በ `java/iroha_android/run_tests.sh`፣ ይህም
የJNI ማሰሪያዎችን፣ WorkManager ወረፋ መልሶ ማጫወትን፣ እና StrongBox ውድቀትን ይለማመዳል።
የማሽከርከር ማስረጃዎች፣ የውድቀት ማስታወሻዎች እና የድጋሚ ግልባጮች በቀጥታ ስርጭት
`artifacts/android/fixture_runs/`.

### ስዊፍት (macOS/iOS)
`IrohaSwift/` በ I18NI0000057X በኩል ተመሳሳይ የ I18NT0000002X መስታወቶች.
ስክሪፕቱ የማሽከርከር ባለቤትን፣ የድጋፍ መለያን እና ምንጩን (`live` vs I18NI0000059X) ይመዘግባል
በ `artifacts/swift_fixture_regen_state.json` ውስጥ እና ሜታዳታውን ወደ ውስጥ ይመገባል።
ግልጽነት ማረጋገጫ. `scripts/swift_fixture_archive.py` ጠባቂዎች ወደ ውስጥ እንዲገቡ ያስችላቸዋል
ዝገት-የተፈጠሩ ማህደሮች; `scripts/check_swift_fixtures.py` እና
`ci/check_swift_fixtures.sh` ባይት-ደረጃ እኩልነት እና SLA የዕድሜ ገደቦችን ያስፈጽማል
`scripts/swift_fixture_regen.sh` ለመመሪያው `SWIFT_FIXTURE_EVENT_TRIGGER` ይደግፋል
ሽክርክሪቶች. የማደግ የስራ ፍሰቱ፣ KPIs እና ዳሽቦርዶች ተመዝግበዋል።
`docs/source/swift_parity_triage.md` እና የ cadence አጭር መግለጫዎች ስር
`docs/source/sdk/swift/`.

### Python
የፓይዘን ደንበኛ (`python/iroha_python/`) የአንድሮይድ ዕቃዎችን ይጋራል። መሮጥ
`scripts/python_fixture_regen.sh` የቅርብ ጊዜውን የI18NI0000070X ጭነት ይጎትታል፣ ያድሳል
`python/iroha_python/tests/fixtures/`፣ እና የcadance ሜታዳታ ወደ ውስጥ ይለቃል
`artifacts/python_fixture_regen_state.json` አንዴ የመጀመሪያው የድህረ-መንገድ ካርታ መሽከርከር
ተይዟል። `scripts/check_python_fixtures.py` እና
`python/iroha_python/scripts/run_checks.sh` በር ፓይትስት፣ ማይፒ፣ ሩፍ እና ቋሚ
በአካባቢው እና በ CI ውስጥ እኩልነት. ከጫፍ እስከ ጫፍ ሰነዶች (`docs/source/sdk/python/…`) እና
የ binding regen playbook ከአንድሮይድ ጋር ሽክርክሮችን እንዴት ማቀናጀት እንደሚቻል ይገልጻል
ባለቤቶች.

### ጃቫስክሪፕት
`javascript/iroha_js/` በአካባቢያዊ I18NI0000077X ፋይሎች ላይ የተመካ አይደለም፣ ነገር ግን WP1-E ይከታተላል
የተለቀቀው ማስረጃው ስለዚህ የጂፒዩ CI መስመሮች ሙሉ ማረጋገጫን ይወርሳሉ። እያንዳንዱ ልቀት
በI18NI0000078X (የተጎላበተው በ
`javascript/iroha_js/scripts/record-release-provenance.mjs`) ያመነጫል እና ይፈርማል
SBOM ከ`scripts/js_sbom_provenance.sh` ጋር ይጣመራል፣ የተፈረመውን የደረቅ ሩጫ ያካሂዳል
(`scripts/js_signed_staging.sh`)፣ እና የመመዝገቢያውን አርቲፊኬት ያረጋግጣል
`javascript/iroha_js/scripts/verify-release-tarball.mjs`. የተገኘው ሜታዳታ
መሬቶች በ `artifacts/js-sdk-provenance/`፣ `artifacts/js/npm_staging/`፣
`artifacts/js/sbom/` እና `artifacts/js/verification/`
የመንገድ ካርታ JS5/JS6 እና WP1-F ቤንችማርክ ሩጫዎች ማስረጃ። የሕትመት መጫወቻ መጽሐፍ በ
`docs/source/sdk/js/` አውቶማቲክን አንድ ላይ ያጣምራል።
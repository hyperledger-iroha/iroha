---
lang: hy
direction: ltr
source: docs/bindings/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf9773ecd75fc31ee89da58a3c5eda846b910eb6e131f1e042b565892e028f16
source_last_modified: "2025-12-29T18:16:35.062011+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SDK-ի պարտադիր և հարմարանքների կառավարում

WP1-E-ն ճանապարհային քարտեզի վրա կոչում է «փաստաթղթեր/կապող փաստաթղթեր»՝ որպես կանոնական վայր՝ պահելու համար
միջլեզու պարտադիր վիճակ. Այս փաստաթուղթը գրանցում է պարտադիր գույքագրումը,
վերածնման հրամաններ, դրեյֆ պաշտպաններ և ապացույցների տեղակայումներ՝ GPU-ի հավասարության համար
դարպասները (WP1-E/F/G) և խաչաձեւ SDK կադենսային խորհուրդն ունեն մեկ հղում:

## Համատեղ պահակաձողեր
- **Կանոնական խաղագիրք.** `docs/source/norito_binding_regen_playbook.md` գրված է
  ռոտացիայի քաղաքականությունը, ակնկալվող ապացույցները և ընդլայնման աշխատանքային հոսքը Android-ի համար,
  Swift, Python և ապագա կապեր:
- **Norito սխեմայի հավասարություն.** `scripts/check_norito_bindings_sync.py` (կանչվել է միջոցով
  `scripts/check_norito_bindings_sync.sh` և փակված է CI-ով
  `ci/check_norito_bindings_sync.sh`) արգելափակում է, երբ Rust, Java կամ Python
  սխեմայի արտեֆակտները շեղվում են:
- **Կադենս պահակ.** `scripts/check_fixture_cadence.py` կարդում է
  `artifacts/*_fixture_regen_state.json` ֆայլերը և պարտադրում է երեք/ուրբաթ (Android,
  Python) և Wed (Swift) պատուհանները, այնպես որ ճանապարհային քարտեզի դարպասներն ունեն ստուգվող ժամանակային դրոշմանիշներ:

## Կապող մատրիցա

| Պարտադիր | Մուտքի կետեր | Fixture / regen հրաման | Դրիֆտ պահակներ | Ապացույցներ |
|---------|--------------------------------------------------------|----------|
| Android (Java) | `java/iroha_android/` (`java/iroha_android/README.md`) | `scripts/android_fixture_regen.sh` → `artifacts/android_fixture_regen_state.json` | `scripts/check_android_fixtures.py`, `ci/check_android_fixtures.sh`, `java/iroha_android/run_tests.sh` | `artifacts/android/fixture_runs/` |
| Swift (iOS/macOS) | `IrohaSwift/` (`IrohaSwift/README.md`) | `scripts/swift_fixture_regen.sh` (ըստ ցանկության `SWIFT_FIXTURE_ARCHIVE`) → `artifacts/swift_fixture_regen_state.json` | `scripts/check_swift_fixtures.py`, `ci/check_swift_fixtures.sh`, `scripts/swift_fixture_archive.py` | `docs/source/swift_parity_triage.md`, `docs/source/sdk/swift/ios2_fixture_cadence_brief.md` |
| Պիթոն | `python/iroha_python/` (`python/iroha_python/README.md`) | `scripts/python_fixture_regen.sh` → `artifacts/python_fixture_regen_state.json` | `scripts/check_python_fixtures.py`, `python/iroha_python/scripts/run_checks.sh` | `docs/source/norito_binding_regen_playbook.md`, `docs/source/sdk/python/connect_end_to_end.md` |
| JavaScript | `javascript/iroha_js/` (`docs/source/sdk/js/publishing.md`) | `npm run release:provenance`, `scripts/js_sbom_provenance.sh`, `scripts/js_signed_staging.sh` | `npm run test`, `javascript/iroha_js/scripts/verify-release-tarball.mjs`, `javascript/iroha_js/scripts/record-release-provenance.mjs` | `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`, `artifacts/js/verification/`, `artifacts/js/sbom/` |

## Պարտադիր մանրամասներ

### Android (Java)
Android SDK-ն աշխատում է `java/iroha_android/`-ի տակ և օգտագործում է կանոնական Norito
հարմարանքներ արտադրված `scripts/android_fixture_regen.sh`-ի կողմից: Այդ օգնականը արտահանում է
Թարմ `.norito` բլիթներ Rust գործիքների շղթայից, թարմացումներ
`artifacts/android_fixture_regen_state.json` և գրանցում է կադենսային մետատվյալներ, որոնք
`scripts/check_fixture_cadence.py` և կառավարման վահանակները սպառում են: Դրեյֆն է
հայտնաբերված `scripts/check_android_fixtures.py`-ի կողմից (նաև միացված է
`ci/check_android_fixtures.sh`) և `java/iroha_android/run_tests.sh`-ի կողմից, որը
իրականացնում է JNI կապերը, WorkManager հերթի վերարտադրումը և StrongBox-ի հետադարձ կապերը:
Պտտման ապացույցները, ձախողման նշումները և կրկնվող վերարտադրումները ապրում են տակ
`artifacts/android/fixture_runs/`.

### Swift (macOS/iOS)
`IrohaSwift/`-ը արտացոլում է նույն Norito օգտակար բեռները `scripts/swift_fixture_regen.sh`-ի միջոցով:
Սցենարը գրանցում է ռոտացիայի սեփականատիրոջը, արագության պիտակը և աղբյուրը (`live` vs `archive`)
ներսում `artifacts/swift_fixture_regen_state.json` և սնուցում է մետատվյալները
cadence checker. `scripts/swift_fixture_archive.py`-ը թույլ է տալիս սպասարկողներին կուլ տալ
Ժանգից առաջացած արխիվներ; `scripts/check_swift_fixtures.py` և
`ci/check_swift_fixtures.sh`-ը պարտադրում է բայթ մակարդակի հավասարությունը գումարած SLA տարիքային սահմանները, մինչդեռ
`scripts/swift_fixture_regen.sh`-ն աջակցում է `SWIFT_FIXTURE_EVENT_TRIGGER`-ին ձեռնարկի համար
ռոտացիաներ. Էսկալացիայի աշխատանքային հոսքը, KPI-ները և վահանակները փաստաթղթավորված են
`docs/source/swift_parity_triage.md` և կադենսային համառոտագրերը տակ
`docs/source/sdk/swift/`.

### Python
Python հաճախորդը (`python/iroha_python/`) կիսում է Android սարքերը: Վազում
`scripts/python_fixture_regen.sh`-ը քաշում է վերջին `.norito` բեռները, թարմացնում է
`python/iroha_python/tests/fixtures/` և կթողարկի կադենսային մետատվյալներ
`artifacts/python_fixture_regen_state.json` մեկ անգամ ճանապարհային քարտեզից հետո առաջին ռոտացիան
գրավված է. `scripts/check_python_fixtures.py` և
`python/iroha_python/scripts/run_checks.sh` դարպասի pytest, mypy, ruff և հարմարանք
հավասարություն տեղական և CI-ում: Վերջից մինչև վերջ փաստաթղթերը (`docs/source/sdk/python/…`) և
պարտադիր regen խաղագիրքը նկարագրում է, թե ինչպես համակարգել պտույտները Android-ի հետ
սեփականատերերը.

### JavaScript
`javascript/iroha_js/`-ը չի հիմնվում տեղական `.norito` ֆայլերի վրա, այլ WP1-E հետքերը
դրա թողարկման ապացույցը, այնպես որ GPU CI ուղիները ժառանգում են ամբողջական ծագում: Յուրաքանչյուր թողարկում
գրավում է ծագումը `npm run release:provenance`-ի միջոցով (սնուցվում է
`javascript/iroha_js/scripts/record-release-provenance.mjs`), առաջացնում և նշանավորում
SBOM-ը փաթեթավորվում է `scripts/js_sbom_provenance.sh`-ով, գործարկում է ստորագրված փուլային չոր վազքը
(`scripts/js_signed_staging.sh`) և ստուգում է ռեեստրի արտեֆակտը
`javascript/iroha_js/scripts/verify-release-tarball.mjs`. Ստացված մետատվյալները
հողեր `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/` տակ,
`artifacts/js/sbom/` և `artifacts/js/verification/`՝ ապահովելով դետերմինիստական
Ճանապարհային քարտեզի JS5/JS6 և WP1-F հենանիշի գործարկման ապացույցներ: Հրատարակչության գրքույկը
`docs/source/sdk/js/`-ը կապում է ավտոմատացումը:
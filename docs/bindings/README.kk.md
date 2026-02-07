---
lang: kk
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

Жол картасындағы WP1-E құжатты сақтау үшін канондық орын ретінде «құжаттар/байланыстырулар» деп атайды.
тіларалық байланыстыру күйі. Бұл құжат міндетті түгендеуді жазады,
регенерация пәрмендері, дрейф-қорғаулар және дәлелдеу орындары GPU паритеті үшін
қақпаларының (WP1-E/F/G) және SDK арасындағы каденциялық кеңестің бір анықтамасы бар.

## Ортақ қоршаулар
- **Канондық ойын кітабы:** `docs/source/norito_binding_regen_playbook.md` анық жазылған
  ротация саясаты, күтілетін дәлелдер және Android үшін эскалация жұмыс процесі,
  Swift, Python және болашақ байланыстар.
- **Norito схема паритеті:** `scripts/check_norito_bindings_sync.py` (арқылы шақырылады)
  `scripts/check_norito_bindings_sync.sh` және CI арқылы жабылған
  `ci/check_norito_bindings_sync.sh`) Rust, Java немесе Python болған кезде құрастыруды блоктайды.
  схема артефактілерінің дрейфі.
- **Каденс бақылаушысы:** `scripts/check_fixture_cadence.py` оқиды
  `artifacts/*_fixture_regen_state.json` файлдары және сей/жм (Android,
  Python) және Wed (Swift) терезелері, сондықтан жол картасы қақпаларында тексерілетін уақыт белгілері болады.

## Байланыстырушы матрицасы

| Байланыстыру | Кіру нүктелері | Бекіту / реген командасы | Дрейфті қорғаушылар | Дәлелдер |
|---------|--------------|-------------------------|--------------|----------|
| Android (Java) | `java/iroha_android/` (`java/iroha_android/README.md`) | `scripts/android_fixture_regen.sh` → `artifacts/android_fixture_regen_state.json` | `scripts/check_android_fixtures.py`, `ci/check_android_fixtures.sh`, `java/iroha_android/run_tests.sh` | `artifacts/android/fixture_runs/` |
| Swift (iOS/macOS) | `IrohaSwift/` (`IrohaSwift/README.md`) | `scripts/swift_fixture_regen.sh` (міндетті емес `SWIFT_FIXTURE_ARCHIVE`) → `artifacts/swift_fixture_regen_state.json` | `scripts/check_swift_fixtures.py`, `ci/check_swift_fixtures.sh`, `scripts/swift_fixture_archive.py` | `docs/source/swift_parity_triage.md`, `docs/source/sdk/swift/ios2_fixture_cadence_brief.md` |
| Python | `python/iroha_python/` (`python/iroha_python/README.md`) | `scripts/python_fixture_regen.sh` → `artifacts/python_fixture_regen_state.json` | `scripts/check_python_fixtures.py`, `python/iroha_python/scripts/run_checks.sh` | `docs/source/norito_binding_regen_playbook.md`, `docs/source/sdk/python/connect_end_to_end.md` |
| JavaScript | `javascript/iroha_js/` (`docs/source/sdk/js/publishing.md`) | `npm run release:provenance`, `scripts/js_sbom_provenance.sh`, `scripts/js_signed_staging.sh` | `npm run test`, `javascript/iroha_js/scripts/verify-release-tarball.mjs`, `javascript/iroha_js/scripts/record-release-provenance.mjs` | `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`, `artifacts/js/verification/`, `artifacts/js/sbom/` |

## Байланыстыру мәліметтері

### Android (Java)
Android SDK `java/iroha_android/` астында жұмыс істейді және канондық Norito пайдаланады
`scripts/android_fixture_regen.sh` шығарған арматура. Бұл көмекші экспорттайды
Rust құралдар тізбегіндегі жаңа `.norito` блоктары, жаңартулар
`artifacts/android_fixture_regen_state.json` және каденс метадеректерін жазады
`scripts/check_fixture_cadence.py` және басқару бақылау тақталары тұтынады. Дрейф - бұл
`scripts/check_android_fixtures.py` арқылы анықталды (сонымен бірге
`ci/check_android_fixtures.sh`) және `java/iroha_android/run_tests.sh` арқылы,
JNI байланыстыруларын, WorkManager кезегін қайталауды және StrongBox резервтерін жаттықтырады.
Айналдыру дәлелдері, сәтсіздік туралы ескертпелер және қайта орындау транскрипттері тікелей астында
`artifacts/android/fixture_runs/`.

### Swift (macOS/iOS)
`IrohaSwift/` `scripts/swift_fixture_regen.sh` арқылы бірдей Norito пайдалы жүктемелерін көрсетеді.
Скрипт айналу иесін, каденс белгісін және көзді жазады (`live` және `archive`)
`artifacts/swift_fixture_regen_state.json` ішінде және метадеректерді ішіне береді
каденс тексерушісі. `scripts/swift_fixture_archive.py` қолдаушыларға қабылдауға мүмкіндік береді
Тоттан жасалған мұрағаттар; `scripts/check_swift_fixtures.py` және
`ci/check_swift_fixtures.sh` байт деңгейіндегі паритет пен SLA жас шектеулерін, ал
`scripts/swift_fixture_regen.sh` қолмен `SWIFT_FIXTURE_EVENT_TRIGGER` қолдайды
айналымдар. Көтеру жұмыс процесі, KPI және бақылау тақталары құжатталады
`docs/source/swift_parity_triage.md` және астындағы каденс қысқашалары
`docs/source/sdk/swift/`.

### Python
Python клиенті (`python/iroha_python/`) Android құрылғыларымен бөліседі. Жүгіру
`scripts/python_fixture_regen.sh` соңғы `.norito` пайдалы жүктемелерін тартады, жаңартады
`python/iroha_python/tests/fixtures/` және каденция метадеректерін шығарады
`artifacts/python_fixture_regen_state.json` бір рет жол картасынан кейінгі бірінші айналым
тұтқынға алынады. `scripts/check_python_fixtures.py` және
`python/iroha_python/scripts/run_checks.sh` қақпасы pytest, mypy, ruff және арматура
жергілікті және CI бойынша паритет. Аяқталған құжаттар (`docs/source/sdk/python/…`) және
байланыстырушы реген ойын кітабы Android жүйесімен айналымдарды қалай үйлестіру керектігін сипаттайды
иелері.

### JavaScript
`javascript/iroha_js/` жергілікті `.norito` файлдарына сенбейді, бірақ WP1-E тректері
оның шығарылымының дәлелі, сондықтан GPU CI жолақтары толық шығу тегін мұра етеді. Әрбір шығарылым
шығу тегін `npm run release:provenance` арқылы түсіреді (қуат
`javascript/iroha_js/scripts/record-release-provenance.mjs`), жасайды және белгілейді
`scripts/js_sbom_provenance.sh` бар SBOM бумалары, қол қойылған кезеңді құрғақ орындауды іске қосады
(`scripts/js_signed_staging.sh`) және тізілім артефакті арқылы тексереді
`javascript/iroha_js/scripts/verify-release-tarball.mjs`. Алынған метадеректер
`artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/` астындағы жерлер,
`artifacts/js/sbom/` және `artifacts/js/verification/`, детерминирленген
жол картасының JS5/JS6 және WP1-F эталондық жұмысының дәлелі. Басып шығару ойын кітапшасы
`docs/source/sdk/js/` автоматтандыруды біріктіреді.
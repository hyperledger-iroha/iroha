---
lang: ba
direction: ltr
source: docs/bindings/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf9773ecd75fc31ee89da58a3c5eda846b910eb6e131f1e042b565892e028f16
source_last_modified: "2025-12-29T18:16:35.062011+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SDK бәйләү & Фикстура идара итеү

WP1-E юл картаһында саҡырыуҙар “өҫтәлдәр/бәйләүҙәр” канонлы урын булараҡ, һаҡлау өсөн
тел араһындағы бәйләүсе хәл. Был документта мотлаҡ инвентарь теркәлә,
регенерация командалары, дрейф һаҡсылары һәм дәлилдәр урындары шулай GPU паритеты
ҡапҡалар (WP1-E/F/G) һәм SDK кросс-каденция советы бер һылтанмаға эйә.

## Дөйөм ҡоршауҙар
- **Каноник пьесалар китабы:** I18NI000000003Х 1990 йылдарҙа
  ротация сәйәсәте, көтөлгән дәлилдәр, һәм Android өсөн эскалация эш ағымы,
  Свифт, Питон һәм киләсәктә бәйләүҙәр.
- **I18NT000000000000Х схемаһы паритеты:** I18NI000000004X
  I18NI000000005X һәм CI-ла ҡапҡалы.
  I18NI000000006X) блоктар төҙөлә, ҡасан тут, Java, йәки Python
  схема артефакттары дрейф.
- **Каденс күҙәтеүсе:** I18NI000000007X уҡый.
  I18NI000000008X файлдар һәм үтәй Tue/Fri (Android,
  Python) һәм туй (Swift) тәҙрәләр шулай юл картаһы ҡапҡалары аудит ваҡыт маркалары бар.

## бәйләү матрицаһы

| Бәйләнеш | Яҙылыу мәрәйҙәре | Фикстура / реген командаһы | Дрейф һаҡсылары | Дәлилдәр |
|-----------------------------------------------------------|---|---------------|
| Андроид (Java) | `java/iroha_android/` X (`java/iroha_android/README.md`) | `scripts/android_fixture_regen.sh` → `artifacts/android_fixture_regen_state.json` | `scripts/check_android_fixtures.py`, `ci/check_android_fixtures.sh`, `java/iroha_android/run_tests.sh` | `artifacts/android/fixture_runs/` |
| Свифт (iOS/macOS) | I18NI000000017X (I18NI000000018X) | `scripts/swift_fixture_regen.sh` X (факультатив `SWIFT_FIXTURE_ARCHIVE`) → `artifacts/swift_fixture_regen_state.json` | I18NI00000022Х, `ci/check_swift_fixtures.sh`, `scripts/swift_fixture_archive.py` | `docs/source/swift_parity_triage.md`, `docs/source/sdk/swift/ios2_fixture_cadence_brief.md` |
| Питон | `python/iroha_python/` (I18NI000000028X) | `scripts/python_fixture_regen.sh` → I18NI000000030X | I18NI000000031X, I18NI000000032X | `docs/source/norito_binding_regen_playbook.md`, I18NI000000034X |
| JavaScript | I18NI000000035X (I18NI000000036X) | I18NI000000037X, I18NI000000038X, `scripts/js_signed_staging.sh` | `npm run test`, `javascript/iroha_js/scripts/verify-release-tarball.mjs`, `javascript/iroha_js/scripts/record-release-provenance.mjs` | `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`, I18NI000000045X, I18NI000000046X |

## Билдәләү реквизиттары

### Android (Ява)
Android SDK йәшәй I18NI000000047X аҫтында һәм канонлы Norito ҡуллана.
I18NI000000048X тарафынан етештерелгән ҡоролмалары. Шул ярҙамсы экспортҡа сыға
Яңы I18NI0000000049X таптар Rust toolchein, яңыртыу
I18NI000000050X, һәм каденция метамағлүмәттәрен теркәй, тип
`scripts/check_fixture_cadence.py` һәм идара итеү таҡталары ҡулланыла. Дрифт -
асыҡланған I18NI0000000052X (шулай уҡ сымлы инә
I18NI000000053X) һәм I18NI000000054X, улар
JNI бәйләүҙәрен, WorkManager сират реплейы һәм StongBox fallbacks күнекмәләр үткәрә.
Ротацион дәлилдәр, уңышһыҙлыҡ билдәләй, һәм перерапечка стенограммалар буйынса йәшәй .
`artifacts/android/fixture_runs/`.

### Свифт (macOS/iOS)
`IrohaSwift/` көҙгө шул уҡ I18NT000000002X файҙалы йөктәр аша I18NI0000000057X.
Сценарий ротация хужаһы, каденция ярлығы һәм сығанаҡ (I18NI0000000058X vs I18NI000000059X) яҙылған.
I18NI000000060X эсендә һәм метамағлүмәттәрҙе ашата.
каденция тикшерелеүе. I18NI000000061X хеҙмәтләндергәндәр ашау мөмкинлеге бирә
Ҡырҡылған генерацияланған архивтар; I18NI00000062Х һәм
I18NI0000000063X байт кимәлендәге паритетты үтәү плюс SLA йәш сиктәре, шул уҡ ваҡытта
Ҡулланма өсөн I18NI00000000064X I18NI00000000065X ярҙам итә
әйләнеше. Эскалляция эш ағымы, KPI һәм приборҙар таҡталары 2012 йылда документлаштырылған.
I18NI000000066X һәм каденция буйынса брифтар .
`docs/source/sdk/swift/`.

### Питон
Python клиент (`python/iroha_python/`) Android ҡоролмалары менән уртаҡлаша. Йүгереүсе
I18NI0000000069X һуңғы тарта I18NI0000000070 Еҫәгеҙ, яңыртыу
`python/iroha_python/tests/fixtures/`, һәм каденция метамағлүмәттәрен сығарасаҡ.
I18NI0000000072X бер тапҡыр беренсе юл картаһы ротацияһы
әсирлеккә алына. `scripts/check_python_fixtures.py` һәм .
I18NI0000000074X ҡапҡаһы питест, mypy, руф, һәм ҡоролма
паритет урындағы һәм CI. Осҡа тиклем docs (`docs/source/sdk/python/…`) һәм
бәйләүсе реген пьеса китабы Android менән әйләнештәрҙе нисек координациялау тураһында һүрәтләй
хужалары.

### JavaScript
I18NI0000000076X урындағы I18NI000000077X файлдарына таянмай, әммә WP1-E тректары .
уның сығарыу дәлилдәре шулай GPU CI һыҙаттары тулы провенанс мираҫҡа ала. Һәр релиз
I18NI000000078X аша провенансты тотоу (2000 й.
I18NI000000079X), генерациялай һәм билдәләр
SBOM өйөмдәре менән I18NI00000000080X, идара итеү стадияһында ҡуйылған ҡоро-йүгерә
(`scripts/js_signed_staging.sh`), һәм реестр артефактын раҫлай.
`javascript/iroha_js/scripts/verify-release-tarball.mjs`. Һөҙөмтәлә барлыҡҡа килгән метамағлүмәттәр
ерҙәре I18NI000000083X, I18NI000000084X,
I18NI000000085X, һәм I18NI000000086X, детерминистик тәьмин итеү
юл картаһы өсөн дәлилдәр JS5/JS6 һәм WP1-F эталон йүгерә. 1990 йылда нәшриәт пьесалары китабы.
`docs/source/sdk/js/` автоматлаштырыуҙы бергә бәйләй.
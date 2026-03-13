---
id: norito-rpc-adoption
lang: kk
direction: ltr
source: docs/portal/docs/devportal/norito-rpc-adoption.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Norito-RPC Adoption Schedule
sidebar_label: Norito-RPC adoption
description: Cross-SDK rollout plan, evidence checklist, and automation hooks for roadmap item NRPC-4.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

> Канондық жоспарлау жазбалары `docs/source/torii/norito_rpc_adoption_schedule.md` ішінде тұрады.  
> Бұл портал көшірмесі SDK авторлары, операторлары және шолушылары үшін шығару күтулерін тазартады.

## Мақсаттар

- Әрбір SDK (Rust CLI, Python, JavaScript, Swift, Android) екілік Norito-RPC тасымалдауындағы AND4 өндірісінің ауыстырып-қосқышынан бұрын туралаңыз.
- Басқару шығарылымды тексере алуы үшін фазалық қақпаларды, дәлелдер жинақтарын және телеметриялық ілмектерді детерминирленген күйде сақтаңыз.
- NRPC-4 жол картасы шақыратын ортақ көмекшілермен арматура мен канар дәлелдерін алуды жеңілдетіңіз.

## Кезең хронологиясы

| кезең | Терезе | Қолдану аясы | Шығу критерийлері |
|-------|--------|-------|---------------|
| **P0 – Зертханалық паритет** | Q22025 | Rust CLI + Python түтін люкстері CI жүйесінде `/v2/norito-rpc` жұмыс істейді, JS көмекшісі бірлік сынақтарынан өтеді, Android жалған трубалары қос тасымалдау жаттығуларын орындайды. | CI ішінде `python/iroha_python/scripts/run_norito_rpc_smoke.sh` және `javascript/iroha_js/test/noritoRpcClient.test.js` жасыл; Android құрылғысы `./gradlew test` жүйесіне қосылған. |
| **P1 – SDK алдын ала қарау** | Q32025 | Ортақ қондырғылар жинағы тексерілді, `scripts/run_norito_rpc_fixtures.sh --sdk <label>` журналдар + JSON `artifacts/norito_rpc/` жүйесінде, SDK үлгілерінде көрсетілген қосымша Norito тасымалдау жалаушалары тіркелді. | Фикстура манифестіне қол қойылды, README жаңартулары қосылуды пайдалануды көрсетеді, Swift алдын ала қарау API IOS2 жалауының артында қолжетімді. |
| **P2 – Қою / AND4 алдын ала қарау** | Q12026 | Torii кезеңдік пулдары Norito, Android AND4 алдын ала қарау клиенттері және Swift IOS2 паритет жинағы әдепкі бойынша екілік тасымалдауға, телеметрия бақылау тақтасы `dashboards/grafana/torii_norito_rpc_observability.json` толтырылған. | `docs/source/torii/norito_rpc_stage_reports.md` канарейканы түсіреді, `scripts/telemetry/test_torii_norito_rpc_alerts.sh` өтеді, Android жалған жабдығын қайталау сәтті/қате жағдайларын түсіреді. |
| **P3 – Өндірістік GA** | Q42026 | Norito барлық SDK үшін әдепкі тасымалдауға айналады; JSON қайталанатын резерв болып қала береді. Әрбір тегпен жұмыс мұрағатының паритет артефактілерін шығарыңыз. | Rust/JS/Python/Swift/Android үшін Norito түтін шығысының бақылау тізімі дестелерін шығарыңыз; Norito және JSON қате жылдамдығының SLO мәндері үшін ескерту шектері орындалды; `status.md` және шығарылым жазбалары GA дәлелдерін келтіреді. |

## SDK жеткізілімдері және CI ілмектері

- **Rust CLI және интеграциялық белдік** – `iroha_cli pipeline` түтін сынақтарын `cargo xtask norito-rpc-verify` жерге қонғаннан кейін Norito тасымалдауға мәжбүрлеу үшін кеңейтіңіз. `artifacts/norito_rpc/` астында артефактілерді сақтай отырып, `cargo test -p integration_tests -- norito_streaming` (зертхана) және `cargo xtask norito-rpc-verify` (сахналау/GA) арқылы күзет.
- **Python SDK** – шығарылатын түтінді (`python/iroha_python/scripts/release_smoke.sh`) Norito RPC мәніне әдепкі етіп қойыңыз, `run_norito_rpc_smoke.sh` CI кіру нүктесі ретінде сақтаңыз және `python/iroha_python/README.md` ішінде құжат паритеті өңделеді. CI мақсаты: `PYTHON_BIN=python3 python/iroha_python/scripts/run_norito_rpc_smoke.sh`.
- **JavaScript SDK** – `NoritoRpcClient` тұрақтандырыңыз, `toriiClientConfig.transport.preferred === "norito_rpc"` кезінде басқару/сұрау көмекшілеріне әдепкі Norito мәніне рұқсат беріңіз және `javascript/iroha_js/recipes/` жүйесінде соңғы үлгілерді түсіріңіз. Жарияламас бұрын CI `npm test` плюс докерленген `npm run test:norito-rpc` тапсырмасын іске қосуы керек; шығу тегі `javascript/iroha_js/artifacts/` астында Norito түтін журналдарын жүктейді.
- **Swift SDK** – Norito көпір транспортын IOS2 жалауының артына өткізіңіз, арматураның каденциясын көрсетіңіз және Connect/Norito паритет жинағының `docs/source/sdk/swift/index.md` құжатында сілтеме жасалған Buildkite жолақтарында жұмыс істейтініне көз жеткізіңіз.
- **Android SDK** – AND4 алдын ала қарау клиенттері мен жалған Torii құрылғысы `docs/source/sdk/android/networking.md` құжатында қайталау/қайта өшіру телеметриясы бар Norito стандартын қабылдайды. Жабдық `scripts/run_norito_rpc_fixtures.sh --sdk android` арқылы басқа SDK құрылғыларымен құрылғыларды бөліседі.

## Дәлелдемелер және автоматтандыру

- `scripts/run_norito_rpc_fixtures.sh` `cargo xtask norito-rpc-verify` орап, stdout/stderr түсіреді және `fixtures.<sdk>.summary.json` шығарады, сондықтан SDK иелері `status.md` файлына тіркейтін детерминирленген артефактқа ие болады. CI бумаларын ұқыпты ұстау үшін `--sdk <label>` және `--out artifacts/norito_rpc/<stamp>/` пайдаланыңыз.
- `cargo xtask norito-rpc-verify` схема хэш паритетін (`fixtures/norito_rpc/schema_hashes.json`) мәжбүрлейді және Torii `X-Iroha-Error-Code: schema_mismatch` қайтарса, сәтсіз аяқталады. Түзету үшін әрбір сәтсіздікті JSON резервтік түсіруімен жұптаңыз.
- `scripts/telemetry/test_torii_norito_rpc_alerts.sh` және `dashboards/grafana/torii_norito_rpc_observability.json` NRPC-2 үшін ескерту келісім-шарттарын анықтайды. Бақылау тақтасының әрбір өңделуінен кейін сценарийді іске қосыңыз және `promtool` шығысын канарлар бумасында сақтаңыз.
- `docs/source/runbooks/torii_norito_rpc_canary.md` кезеңдік және өндірістік жаттығуларды сипаттайды; арматура хэштері немесе ескерту қақпалары өзгерген сайын оны жаңартыңыз.

## Шолушының бақылау тізімі

NRPC-4 кезеңін белгілемес бұрын, мынаны растаңыз:

1. Соңғы арматура дестесінің хэштері `fixtures/norito_rpc/schema_hashes.json` және `artifacts/norito_rpc/<stamp>/` астында жазылған сәйкес CI артефактіге сәйкес келеді.
2. SDK README/портал құжаттары JSON қалпына келтіруді мәжбүрлеу жолын сипаттайды және Norito тасымалдау әдепкі мәніне сілтеме жасайды.
3. Телеметрия бақылау тақталары ескерту сілтемелері бар қос стек қате жылдамдығы панелдерін көрсетеді және Alertmanager құрғақ іске қосуы (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) трекерге бекітілген.
4. Мұнда қабылдау кестесі трекер жазбасына (`docs/source/torii/norito_rpc_tracker.md`) сәйкес келеді және жол картасы (NRPC-4) бірдей дәлелдер жинағына сілтеме жасайды.

Кесте бойынша тәртіпті сақтау SDK арасындағы әрекетті болжауға мүмкіндік береді және басқару аудитін Norito-RPC тапсырыссыз қабылдауға мүмкіндік береді.
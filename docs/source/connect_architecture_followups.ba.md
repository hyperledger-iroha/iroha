---
lang: ba
direction: ltr
source: docs/source/connect_architecture_followups.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 476331772efe169a7a073b561fa9935e314ff89d6bfff440f7246606c1c02669
source_last_modified: "2025-12-29T18:16:35.934525+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Тоташтырыу архитектураһы артынан барыу ғәмәлдәре

Был иҫкәрмәләр инженерлыҡ күҙәтеүҙәрен тота, улар СДК-ның кроссынан сыҡҡан .
Тоташтырыу архитектураһы тикшерелгән. Һәр рәтте бер мәсьәләгә картаға төшөрөргә (Jira билет йәки PR)
бер эш планлаштырылған. Яңыртыу өҫтәл хужалары булараҡ, күҙәтеү билеттары булдырыу.| Элемент | Тасуирлама | Хужа(тар) | Күҙәтеү | Статус |
|-----|-------------|-----------|----------|--------- |
| 1990 йылдарҙа был йүнәлештәге эштәрҙең иң мөһимдәренең береһе булып тора. Экспоненциаль кире-өҫтөндә тормошҡа ашырыу + дрожь ярҙамсылары (`connect_retry::policy`) һәм уларҙы фашлау өсөн Swift/Android/JS SDKs. | Свифт СДК, Андроид селтәре Т.Л., Дж.С. [IOS-CONNECT-001](project_tracker/connect_architecture_followups_ios.md#ios-connect-001) | тамамланған — `connect_retry::policy` детерминистик flummix64 өлгөләре менән төшкән; Swift (`ConnectRetryPolicy`), Android, һәм JS SDKs судно көҙгө ярҙамсылары плюс алтын һынауҙар. |
| Пинг/понгты үтәү | Өҫтәү өсөн конфигурацияланған йөрәк тибеше үтәү менән килешелгән 30-сы йылдарҙағы каденция һәм браузер минималь зажим; ер өҫтө метрикаһы (`connect.ping_miss_total`). | Свифт СДК, Андроид селтәре Т.Л., Дж.С. [IOS-CONNECT-002] (project_tracker/connect_architecture_followups_ios.md#ios-connect-002) | Һуңғыһы — Torii хәҙер конфигурацияланған йөрәк тибеше интервалдарын үтәй (`ping_interval_ms`, `ping_miss_tolerance`, `ping_min_interval_ms` X), `connect.ping_miss_total` метрикаһын фашлай, ә караптар регрессия һынауҙары йөрәк тибешен өҙөүҙе ҡаплай. SDK функцияһы снимоктар өҫтөндә яңы ручкалар клиенттар өсөн. |
| Офлайн сират ныҡышмалылығы | Torii журнал яҙыусылары/уҡыусыларҙы тормошҡа ашырыу өсөн Connect сираттары (Swift `FileManager`, Android шифрланған һаҡлау, JS IndexedDB) дөйөм схема ҡулланып. | Свифт SDK, Android мәғлүмәттәр моделе TL, JS ҡурғаш | [IOS-CONNECT-003] (project_tracker/connect_architecture_followups_ios.md#ios-connect-003) | тамамланған — Swift, Android, һәм JS хәҙер йөк ташыу өсөн дөйөм `ConnectQueueJournal` + диагностика ярҙамсылары менән һаҡлау/өҫтөндә һынауҙар шулай дәлилдәр өйөмдәр ҡала детерминистик . SDKs.【ИрохаСвифт/ИрохаСвифт/Сығанаҡ-Свифт Йорнал.swift:1】【жава/ироха_андроид/срк/төп/java/org/hipe . 1】【javascript/iroha_js/src/cueue Journal.js:1】 |
| Көслө Бокс аттестация файҙалы йөк | `{platform,evidence_b64,statement_hash}` ептәре янсыҡ раҫлауҙары аша һәм dApp SDKs тикшерергә өҫтәү. | Андроид Крипто Т.Л., Дж.С. [IOS-CONNECT-004](project_tracker/connect_architecture_followups_ios.md#ios-connect-004) | Көтөп |
| Капиталь идара итеү рамкаһы | `Control::RotateKeys` тормошҡа ашырыу + `RotateKeysAck` һәм фашлау `cancelRequest(hash)` / бөтә SDKs API-лар. | Свифт СДК, Андроид селтәре Т.Л., Дж.С. [IOS-CONNECT-005](project_tracker/connect_architecture_followups_ios.md#ios-connect-005) | Көтөп |
| Телеметрия экспортерҙары | `connect.queue_depth`, `connect.reconnects_total`, `connect.latency_ms`, һәм ғәмәлдәге телеметрия торбаларына реплей счетчиктары (OpenTelemetrietfer). | Телеметрия WG, SDK хужалары | [IOS-CONNECT-006] (project_tracker/connect_architecture_followups_ios.md#ios-connect-006) | Көтөп |
| Свифт CI ҡапҡаһы | 18NI000000031X-ға бәйле торбалар менән бәйле торбаларҙы тәьмин итеү, шулай итеп, приборҙар панелен каналдары һәм SDK-лар араһында тура килтерелгән метамағлүмәттәрҙе һаҡлау. | Свифт SDK Ҡурғаш, инфра төҙөү | [IOS-CONNECT-007] (project_tracker/connect_architecture_followups_ios.md#ios-connect-007) | Көтөп |
| Фоллбек инцидент тураһында хәбәр | Сым XCFramework төтөн жгут инциденттары (`xcframework_smoke_fallback`, `xcframework_smoke_strongbox_unavailable`) дөйөм күренеш өсөн приборҙар таҡталарына. | Свифт QA ҡурғаш, төҙөү Инфра | [IOS-CONNECT-008] (project_tracker/connect_architecture_followups_ios.md#ios-connect-008) | Көтөп || Ҡабул итеү ҡушымталары үткән-аша | SDKs ҡабул итеү һәм алға опциональ `attachments[]` + project_tracker/connect_architecture_followups_ios.md#ios-connect-005 раҫлау йөкләмәләрендә юғалтыуҙарһыҙ. | Свифт SDK, Android мәғлүмәттәр моделе TL, JS ҡурғаш | [IOS-CONNECT-009] (project_tracker/connect_architecture_followups_ios.md#ios-connect-009) | Көтөп |
| Хата таксономияһы тура килтереп | Карта уртаҡ enum (`Transport`, project_tracker/connect_architecture_followups_ios.md#ios-connect-007, `Authorization`, `Timeout`, `QueueOverflow`, `Internal`) docs/examples менән платформа-конкрет хаталар. | Свифт СДК, Андроид селтәре Т.Л., Дж.С. [IOS-CONNECT-010] (project_tracker/connect_architecture_followups_ios.md#ios-connect-010) | тамамланған — Swift, Android, һәм JS SDKs йөк ташыу өсөн дөйөм `ConnectError` wrapper + телеметрия ярҙамсылары менән README/TypeScript/Java docs һәм регрессия һынауҙары ҡаплау TLS/timeout/HTTP/codec/qeue . Осраҡтар.【доктар/сығанаҡ/бәйләнеш_error_taxonomy.md:1】【ИрохаСвифт/ИрохаСвифт/Коннектив.свифт:1】【жава/ироха_андроид/src . / тест/java/org/гиперледжер/ироха/андроид/тоташтырыу/КоннекторТест.java:1】【javascript/iroha_js/тест/тоташтырыу Хаҡ.тест.js:1】 |
| Оҫтахана ҡарары журналы | Аннотацияланған палуба / ноталар ҡабул ителгән ҡарарҙарҙы дөйөмләштереү совет архивына баҫтырып сығарыу. | SDK программаһы етәксеһе | [IOS-CONNECT-011] (project_tracker/connect_architecture_followups_ios.md#ios-connect-011) | Көтөп |

> Отслеживание идентификаторҙары тултырыласаҡ, сөнки хужалар асыҡ билеттар; яңыртыу `Status` бағана менән бер рәттән сығарылыш прогресс.
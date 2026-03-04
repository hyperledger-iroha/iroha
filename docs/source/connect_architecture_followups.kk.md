---
lang: kk
direction: ltr
source: docs/source/connect_architecture_followups.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 476331772efe169a7a073b561fa9935e314ff89d6bfff440f7246606c1c02669
source_last_modified: "2025-12-29T18:16:35.934525+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Архитектураның кейінгі әрекеттерін қосыңыз

Бұл жазба кросс SDK нәтижесінде пайда болған инженерлік бақылауларды қамтиды
Архитектураны шолу. Әрбір жол мәселеге сәйкес келуі керек (Jira билеті немесе PR)
жұмыс жоспарланғаннан кейін. Иелері бақылау билеттерін жасағанда кестені жаңартыңыз.| Элемент | Сипаттама | Ие(лер) | Бақылау | Күй |
|------|-------------|----------|----------|--------|
| Ортақ кері тұрақтылар | Экспоненциалды кері қайтару + діріл көмекшілерін (`connect_retry::policy`) іске асырыңыз және оларды Swift/Android/JS SDK-ге көрсетіңіз. | Swift SDK, Android Networking TL, JS Lead | [IOS-CONNECT-001](project_tracker/connect_architecture_followups_ios.md#ios-connect-001) | Аяқталды — `connect_retry::policy` детерминирленген splitmix64 іріктеуімен қонды; Swift (`ConnectRetryPolicy`), Android және JS SDK айналанған көмекшілерді және алтын сынақтарды жібереді. |
| Пинг/понгті орындау | Келісілген 30-шы жылдардың каденциясы және шолғыштың ең аз қысқышы бар конфигурацияланатын жүрек соғуын күшейтуді қосыңыз; беттік көрсеткіштер (`connect.ping_miss_total`). | Swift SDK, Android Networking TL, JS Lead | [IOS-CONNECT-002](project_tracker/connect_architecture_followups_ios.md#ios-connect-002) | Аяқталды — Torii енді конфигурацияланатын жүрек соғу аралықтарын (`ping_interval_ms`, `ping_miss_tolerance`, `ping_min_interval_ms`) күшейтеді, `connect.ping_miss_total` көрсеткішін көрсетеді және бақылауды тексеруді қамтиды. SDK мүмкіндігінің суреттері клиенттерге арналған жаңа түймелерді көрсетеді. |
| Офлайн кезектің тұрақтылығы | Ортақ схеманы пайдалана отырып, Norito `.to` журналын жазушыларды/оқушыларды Connect кезектеріне (Swift `FileManager`, Android шифрланған қоймасы, JS IndexedDB) енгізіңіз. | Swift SDK, Android Data Model TL, JS Lead | [IOS-CONNECT-003](project_tracker/connect_architecture_followups_ios.md#ios-connect-003) | Аяқталды — Swift, Android және JS енді ортақ `ConnectQueueJournal` + диагностика көмекшілерін сақтау/толып кету сынақтарымен жібереді, осылайша дәлелдер топтамалары детерминистік болып қалады. SDKs.【IrohaSwift/Sources/IrohaSwift/ConnectQueueJournal.swift:1】【java/iroha_android/src/main/java/org/hype rledger/iroha/android/connect/ConnectQueueJournal.java:1】【javascript/iroha_js/src/connectQueueJournal.js:1】 |
| StrongBox аттестаттау жүктемесі | Әмиянды мақұлдау арқылы `{platform,evidence_b64,statement_hash}` таратып, dApp SDK файлдарына растауды қосыңыз. | Android Crypto TL, JS Lead | [IOS-CONNECT-004](project_tracker/connect_architecture_followups_ios.md#ios-connect-004) | Күтуде |
| Айналуды басқару жақтауы | `Control::RotateKeys` + `RotateKeysAck` іске қосыңыз және `cancelRequest(hash)` / айналдыру API интерфейстерін барлық SDK-де көрсетіңіз. | Swift SDK, Android Networking TL, JS Lead | [IOS-CONNECT-005](project_tracker/connect_architecture_followups_ios.md#ios-connect-005) | Күтуде |
| Телеметрия экспорттаушылар | `connect.queue_depth`, `connect.reconnects_total`, `connect.latency_ms` шығарыңыз және есептегіштерді бар телеметрия құбырларына қайталаңыз (OpenTelemetry). | Telemetry WG, SDK иелері | [IOS-CONNECT-006](project_tracker/connect_architecture_followups_ios.md#ios-connect-006) | Күтуде |
| Swift CI қақпағы | Қосылымға қатысты конвейерлер `make swift-ci` шақыратынына көз жеткізіңіз, осылайша бекіту паритеті, бақылау тақтасы арналары және Buildkite `ci/xcframework-smoke:<lane>:device_tag` метадеректері SDKs бойынша тураланған. | Swift SDK жетекші, Инфра құрастыру | [IOS-CONNECT-007](project_tracker/connect_architecture_followups_ios.md#ios-connect-007) | Күтуде |
| Fallback incident reporting | Ортақ көріну үшін XCFramework түтін сымының оқиғаларын (`xcframework_smoke_fallback`, `xcframework_smoke_strongbox_unavailable`) Connect бақылау тақталарына жалғаңыз. | Swift QA жетекші, Инфра құрастыру | [IOS-CONNECT-008](project_tracker/connect_architecture_followups_ios.md#ios-connect-008) | Күтуде || Сәйкестік тіркемелерінің өтуі | SDK қосымша `attachments[]` + `compliance_manifest_id` өрістерін мақұлдау пайдалы жүктемелерінде жоғалтпай қабылдап, жіберетініне көз жеткізіңіз. | Swift SDK, Android Data Model TL, JS Lead | [IOS-CONNECT-009](project_tracker/connect_architecture_followups_ios.md#ios-connect-009) | Күтуде |
| Қате таксономиясын туралау | Ортақ нөмірді (`Transport`, `Codec`, `Authorization`, `Timeout`, `QueueOverflow`, `Internal`) платформа-спецификалық қателері бар қателермен салыстырыңыз. | Swift SDK, Android Networking TL, JS Lead | [IOS-CONNECT-010](project_tracker/connect_architecture_followups_ios.md#ios-connect-010) | Аяқталды — Swift, Android және JS SDK ортақ `ConnectError` қаптамасын + README/TypeScript/Java құжаттарымен және TLS/тайм-аут/HTTP/кодек/кезекті қамтитын регрессия сынақтары бар телеметрия көмекшілерін жеткізеді. жағдайлар.【docs/source/connect_error_taxonomy.md:1】【IrohaSwift/Sources/IrohaSwift/ConnectError.swift:1】【java/iroha_android/src /test/java/org/hyperledger/iroha/android/connect/ConnectErrorTests.java:1】【javascript/iroha_js/test/connectError.test.js:1】 |
| Семинардың шешімдер журналы | Қабылданған шешімдерді қорытындылайтын аннотацияланған палубаны / жазбаларды кеңес мұрағатына жариялаңыз. | SDK бағдарламасының жетекшісі | [IOS-CONNECT-011](project_tracker/connect_architecture_followups_ios.md#ios-connect-011) | Күтуде |

> Бақылау идентификаторлары иелері ашық билеттер ретінде толтырылады; `Status` бағанын мәселенің орындалу барысымен бірге жаңартыңыз.
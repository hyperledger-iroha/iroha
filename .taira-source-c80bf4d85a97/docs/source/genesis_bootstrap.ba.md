---
lang: ba
direction: ltr
source: docs/source/genesis_bootstrap.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6feb2b03bd8f6a41de693a0c3f3c4ffc058072bc7942e2bc50b3fd9770aa56d4
source_last_modified: "2025-12-29T18:16:35.962003+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Genesis Ышаныслы тиҫтерҙәренән загрузка

Iroha тиҫтерҙәре урындағы `genesis.file`һеҙ ышаныслы тиҫтерҙәрҙән ҡул ҡуйылған генез блогын ала ала
Norito протоколын ҡулланып.

- **Протокол:** тиңдәштәре `GenesisRequest` (`Preflight` метамағлүмәттәр өсөн, `Fetch` өсөн файҙалы йөк өсөн алмашыу) һәм
  `GenesisResponse` кадрҙары `request_id` төймәле. Яуап биргәндәр сылбырлы id, ҡултамға ҡабығы,
  хеш, һәм факультатив ҙурлыҡтағы кәңәш; файҙалы йөктәр тик `Fetch`-та ғына ҡайтарыла, ә дубликаты запрос ids
  `DuplicateRequest` ала.
- **Гардс:** яуап биргәндәр рөхсәт итеү исемлеген үтәй (`genesis.bootstrap_allowlist` йәки ышаныслы тиҫтерҙәре
  сылбыр-ид/пубкей/хэш тап килтереп, ставка сиктәре (`genesis.bootstrap_response_throttle`), һәм а
  ҙурлыҡтағы ҡапҡас (`genesis.bootstrap_max_bytes`). Запростар рөхсәт исемлегенән ситтә ала `NotAllowed`, һәм
  дөрөҫ булмаған асҡыс ҡул ҡуйған файҙалы йөктәр `MismatchedPubkey` ала.
- **Запрос ағымы:** һаҡлау буш булғанда һәм `genesis.file` X (һәм
  `genesis.bootstrap_enabled=true`), төйөндәр осоу алдынан ышаныслы тиҫтерҙәре менән факультатив
  `genesis.expected_hash`, һуңынан файҙалы йөктө ала, `validate_genesis_block` аша ҡултамғаларҙы раҫлай,
  һәм Блокты ҡулланғансы Кура менән бергә `genesis.bootstrap.nrt` һаҡлана. Загрузчик Ретиялар
  `genesis.bootstrap_request_timeout`, `genesis.bootstrap_retry_interval`, һәм
  `genesis.bootstrap_max_attempts`.
- **Уңышһыҙлыҡ режимдары:** запростар өсөн кире ҡағыла рөхсәт исемлеге һағынып, сылбыр/пубкей/хэш тап килмәү, ҙурлыҡ .
  ҡапҡас боҙоуҙар, ставка сиктәре, локаль генез юҡ, йәки дубликаты запрос ids. Ҡаршылыҡлы хеш
  тиҫтерҙәре буйынса фетчты туҡтатыу; бер ниндәй ҙә яуап биргәндәр/тайм-ауттар урындағы конфигурацияға кире төшә.
- **оператор аҙымдары:** тәьмин итеү, кәмендә бер ышаныслы тиҫтере дөрөҫ генез менән етергә мөмкин, конфигурация
  `bootstrap_allowlist`/`bootstrap_max_bytes`/`bootstrap_response_throttle` һәм ҡабаттан ручкалар, һәм
  опциональ штекер `expected_hash` ҡабул итеү өсөн ҡабул итеү тап килмәгән файҙалы йөкләмәләр. Ҡаты файҙалы йөктәр булыуы мөмкин
  артабанғы итектәрҙә `genesis.file` күрһәтеп, ҡабаттан ҡулланылған `genesis.bootstrap.nrt` тиклем.
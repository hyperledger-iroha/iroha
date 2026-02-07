---
lang: ba
direction: ltr
source: docs/source/content_hosting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c0c7f98dbd9f49c573302f0b5cbe2e7a663d7fe35a1a9eea8da4f24c6f9bc8b
source_last_modified: "2026-01-05T18:22:23.402176+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Йөкмәткеле хостинг һыҙаты
% Iroha Ядро

# Йөкмәткеле хостинг һыҙаты

Йөкмәткеһе һыҙатында бәләкәй статик өйөмдәр (тар архивтары) сылбырҙа һаҡлай һәм хеҙмәт итә
айырым файлдар туранан-тура Torii.

- **Баҫма**: `PublishContentBundle` X-ты тар архивы менән тапшырығыҙ, факультатив срогы
  бейеклеге, һәм факультатив манифест. Блендель идентификаторы - был blake2b хеш
  татарбол. Тар яҙмалар даими файлдар булырға тейеш; исемдәре UTF-8 юлдарын нормалаштырылған.
  Ҙурлыҡ/юл/файл-иҫәп ҡапҡастары `content` конфигынан килә (`max_bundle_bytes`,
  `max_files`, `max_path_len`, `max_retention_blocks`, `chunk_size_bytes`).
  Манифесттарҙа Norito хеш, мәғлүмәт киңлеге/һыҙыҡ, кэш сәйәсәте инә.
  (`max_age_seconds`, `immutable`), аут режимы (`public` / `role:<role>` /
  `sponsor:<uaid>`), һаҡлау сәйәсәте урынлаштырыусы, һәм MIME өҫтөнлөк бирә.
- **Дедупинг**: тар файҙалы йөктәр өлөшө ( 64KiB ручка) һәм бер тапҡыр һаҡлана.
  хеш менән белешмәләр һаны; отставкаға өйөм кәмей һәм счетки өлөштәре.
- **Хеҙмәт**: Torii `GET /v1/content/{bundle}/{path}` фашлай. Яуаптар ағымы
  туранан-тура `ETag` = файл хеш, `Accept-Ranges: bytes`, туранан-тура магазиндан,
  Диапазон ярҙам, һәм Кэш-контроль алынған манифест. Уҡый хөрмәт .
  асыҡтан-асыҡ аут режимы: ролле һәм бағыусы ҡапҡалы яуаптар канонлы талап итә
  ҡултамғаһы өсөн үтенес баштары (`X-Iroha-Account`, `X-Iroha-Signature`)
  иҫәп яҙмаһы; юғалған/ваҡытыла өйөмдәр 404-се ҡайта.
- **CLI**: `iroha content publish --bundle <path.tar>` (йәки `--root <dir>`) хәҙер
  автоматик генерациялай, манифест, опциональ `--manifest-out/--bundle-out`, һәм
  Ҡабул итә `--auth`, `--cache-max-age-secs`, `--dataspace`, `--lane`, `--immutable`,
  һәм `--expires-at-height` өҫтөнлөк итә. `iroha content pack --root <dir>` төҙөй
  детерминистик татарбол + бер нәмә лә тапшырмай манифест.
- **Конфиг**: кэш/аут ручкалары `content.*` X-та йәшәй.
  (`default_cache_max_age_secs`, `max_cache_max_age_secs`, `immutable_bundles`,
  `default_auth_mode`) һәм баҫтырыу ваҡытында үтәлә.
- **SLO + сиктәре**: `content.max_requests_per_second` / `request_burst` һәм
  `content.max_egress_bytes_per_second` / `egress_burst_bytes` капты уҡыу яғы
  үткәреүсәнлеге; Torii байт һәм экспортҡа хеҙмәт иткәнсе лә үтәй
  `torii_content_requests_total`, `torii_content_request_duration_seconds`, һәм
  `torii_content_response_bytes_total` метрикаһы һөҙөмтәһе ярлыҡтары менән. Латентлыҡ
  маҡсаттары буйынса йәшәй `content.target_p50_latency_ms` /
  `content.target_p99_latency_ms` / `content.target_availability_bps`.
- **Йәберләү менән идара итеү**: ставкаһы биҙрәләре UAID/API токен/алыҫ IP-дистанцион клавиатура, һәм ан
  опциональ PoW һаҡсыһы (`content.pow_difficulty_bits`, `content.pow_header`) ала
  уҡыу алдынан талап ителә. DA һыҙат макеты ғәҙәттәгесә килә.
  `content.stripe_layout` һәм квитанцияларҙа/махсус хештарҙа яңғырай.
- **Квитанциялар & DA дәлилдәр**: уңышлы яуаптар беркетелә
  `sora-content-receipt` (база64 Norito-кадрлы `ContentDaReceipt` байт) йөрөтөү
  `bundle_id`, `path`, `file_hash`, `served_bytes`, хеҙмәтләндерелгән байт диапазоны,
  `chunk_root` / `stripe_layout`, өҫтәмә PDP йөкләмәһе, һәм ваҡыт тамғаһы шулай
  клиенттар нимә алынған, тип ҡайнатып була, тәнде ҡабаттан уҡымайынса.

Төп һылтанмалар:- Мәғлүмәттәр моделе: `crates/iroha_data_model/src/content.rs`
- Башҡарыу: `crates/iroha_core/src/smartcontracts/isi/content.rs`
- Torii ручкаһы: `crates/iroha_torii/src/content.rs`
- CLI ярҙамсыһы: Torii
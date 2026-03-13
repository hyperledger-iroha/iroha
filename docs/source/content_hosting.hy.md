---
lang: hy
direction: ltr
source: docs/source/content_hosting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c0c7f98dbd9f49c573302f0b5cbe2e7a663d7fe35a1a9eea8da4f24c6f9bc8b
source_last_modified: "2026-01-05T18:22:23.402176+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Content Hosting Lane
% Iroha Core

# Բովանդակության հոսթինգ գոտի

Բովանդակության գիծը պահպանում է փոքր ստատիկ փաթեթներ (tar արխիվներ) շղթայում և սպասարկում
անհատական ֆայլեր անմիջապես Torii-ից:

- **Հրապարակել**. ներկայացնել `PublishContentBundle` tar արխիվով, կամընտիր ժամկետը
  բարձրություն և կամընտիր մանիֆեստ: Փաթեթի ID-ն blake2b-ի հեշն է
  թարբոլ. Tar գրառումները պետք է լինեն սովորական ֆայլեր. անունները նորմալացված UTF-8 ուղիներ են:
  Չափի/ուղու/ֆայլերի քանակի գլխարկները գալիս են `content` կազմաձևից (`max_bundle_bytes`,
  `max_files`, `max_path_len`, `max_retention_blocks`, `chunk_size_bytes`):
  Դրսևորումները ներառում են Norito-ինդեքսի հեշը, տվյալների տարածությունը/գիծը, քեշի քաղաքականությունը
  (`max_age_seconds`, `immutable`), վավերացման ռեժիմ (`public` / `role:<role>` /
  `sponsor:<uaid>`), պահպանման քաղաքականության տեղաբաշխիչ և MIME-ի փոխարինումներ:
- **Հեռացում**. խեժի օգտակար բեռները կտրվում են (կանխադրված 64 ԿԲ) և պահվում մեկ անգամ
  հեշ՝ հղումների քանակով; retiring մի փաթեթ decrements եւ սալորաչիր կտորների.
- **Ծառայել**. Torii-ը բացահայտում է `GET /v2/content/{bundle}/{path}`: Պատասխանների հոսք
  անմիջապես կտորների խանութից՝ `ETag` = ֆայլի հեշ, `Accept-Ranges: bytes`,
  Range աջակցություն և Cache-Control՝ ստացված մանիֆեստից: Կարդում է պատիվ
  մանիֆեստի վավերականության ռեժիմ. դերերի և հովանավորների կողմից փակված պատասխանները պահանջում են կանոնական
  հարցումների վերնագրեր (`X-Iroha-Account`, `X-Iroha-Signature`) ստորագրվածների համար
  հաշիվ; բացակայող/ժամկետանց փաթեթները վերադարձվում են 404.
- **CLI**՝ `iroha content publish --bundle <path.tar>` (կամ `--root <dir>`) հիմա
  ինքնաբերաբար ստեղծում է մանիֆեստ, թողարկում կամընտիր `--manifest-out/--bundle-out` և
  ընդունում է `--auth`, `--cache-max-age-secs`, `--dataspace`, `--lane`, `--immutable`,
  և `--expires-at-height` անտեսում: `iroha content pack --root <dir>` կառուցում է
  դետերմինիստական թարբոլ + մանիֆեստ՝ առանց որևէ բան ներկայացնելու։
- **Կարգավորում**. քեշի/հավաստագրման կոճակները գործում են `content.*`-ի տակ `iroha_config`-ում
  (`default_cache_max_age_secs`, `max_cache_max_age_secs`, `immutable_bundles`,
  `default_auth_mode`) և ուժի մեջ են մտնում հրապարակման պահին:
- **SLO + սահմանաչափեր**՝ `content.max_requests_per_second` / `request_burst` և
  `content.max_egress_bytes_per_second` / `egress_burst_bytes` գլխարկ կարդալու կողմը
  թողունակություն; Torii-ը պարտադրում է ինչպես բայթերի սպասարկումը, այնպես էլ արտահանումը
  `torii_content_requests_total`, `torii_content_request_duration_seconds` և
  `torii_content_response_bytes_total` չափումներ՝ արդյունքների պիտակներով: Լատենտություն
  թիրախները ապրում են `content.target_p50_latency_ms` /
  `content.target_p99_latency_ms` / `content.target_availability_bps`.
- **Չարաշահման վերահսկում**. Գնահատման դույլերը մուտքագրվում են UAID/API նշանով/հեռավոր IP-ով և
  ընտրովի PoW պաշտպանիչ (`content.pow_difficulty_bits`, `content.pow_header`) կարող է
  պահանջվում է կարդալուց առաջ: DA շերտի դասավորության լռելյայնները գալիս են
  `content.stripe_layout` և արձագանքվում են ստացականների/մանիֆեստի հեշերում:
- **Անդորրագրեր և DA ապացույցներ**. հաջողված պատասխանները կցվում են
  `sora-content-receipt` (բազային 64 Norito շրջանակված `ContentDaReceipt` բայթ) կրող
  `bundle_id`, `path`, `file_hash`, `served_bytes`, սպասարկվող բայթերի տիրույթը,
  `chunk_root` / `stripe_layout`, կամընտիր PDP պարտավորություն և ժամանակի դրոշմակնիք
  հաճախորդները կարող են ամրացնել այն, ինչ բերվել է առանց մարմինը նորից կարդալու:

Հիմնական հղումներ.- Տվյալների մոդելը՝ `crates/iroha_data_model/src/content.rs`
- Կատարումը՝ `crates/iroha_core/src/smartcontracts/isi/content.rs`
- Torii կարգավորիչ՝ `crates/iroha_torii/src/content.rs`
- CLI օգնական՝ `crates/iroha_cli/src/content.rs`
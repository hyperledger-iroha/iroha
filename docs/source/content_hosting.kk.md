---
lang: kk
direction: ltr
source: docs/source/content_hosting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c0c7f98dbd9f49c573302f0b5cbe2e7a663d7fe35a1a9eea8da4f24c6f9bc8b
source_last_modified: "2026-01-05T18:22:23.402176+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Мазмұнды хостинг жолы
% Iroha Негізгі

# Контентті хостинг жолы

Мазмұн жолағы шағын статикалық бумаларды (тар архивтері) тізбекте сақтайды және қызмет етеді
жеке файлдар тікелей Torii.

- **Жариялау**: `PublishContentBundle` tar мұрағатымен жіберіңіз, қосымша жарамдылық мерзімі
  биіктігі және қосымша манифест. Бума идентификаторы blake2b хэші болып табылады
  тарбол. Тар жазбалары кәдімгі файлдар болуы керек; атаулары нормаланған UTF-8 жолдары болып табылады.
  Өлшем/жол/файл санауының бас әріптері `content` конфигурациясынан келеді (`max_bundle_bytes`,
  `max_files`, `max_path_len`, `max_retention_blocks`, `chunk_size_bytes`).
  Манифесттерге Norito индексінің хэші, деректер кеңістігі/жолақ, кэш саясаты кіреді.
  (`max_age_seconds`, `immutable`), аутентификация режимі (`public` / `role:<role>` /
  `sponsor:<uaid>`), сақтау саясатының толтырғышы және MIME қайта анықтаулары.
- **Дедупинг**: гудронның пайдалы жүктемелері бөліктерге бөлінеді (әдепкі 64KiB) және бір рет сақталады.
  сілтеме сандары бар хэш; дескрипт және кесінділерді кесіп тастайды.
- **Қызмет көрсету**: Torii `GET /v2/content/{bundle}/{path}` көрсетеді. Жауаптар ағыны
  тікелей `ETag` файлдық хэш, `Accept-Ranges: bytes`,
  Ауқым қолдауы және манифесттен алынған кэш-басқару. Құрметпен оқиды
  манифест аутентификация режимі: рөлдік және демеушілік қақпасы бар жауаптар канондық талап етеді
  қол қойылған сұрау тақырыптары (`X-Iroha-Account`, `X-Iroha-Signature`)
  шот; жоқ/мерзімі өткен бумалар қайтарылады 404.
- **CLI**: `iroha content publish --bundle <path.tar>` (немесе `--root <dir>`) қазір
  манифестті автоматты түрде жасайды, қосымша `--manifest-out/--bundle-out` шығарады және
  қабылдайды `--auth`, `--cache-max-age-secs`, `--dataspace`, `--lane`, `--immutable`,
  және `--expires-at-height` қайта анықтау. `iroha content pack --root <dir>` құрастырады
  детерминистік тарбол + ешнәрсе ұсынбай манифест.
- **Конфигурация**: кэш/аутентификация түймелері `iroha_config` ішіндегі `content.*` астында жұмыс істейді
  (`default_cache_max_age_secs`, `max_cache_max_age_secs`, `immutable_bundles`,
  `default_auth_mode`) және жариялау уақытында күшіне енеді.
- **SLO + шектеулері**: `content.max_requests_per_second` / `request_burst` және
  `content.max_egress_bytes_per_second` / `egress_burst_bytes` қалпақ оқу жағы
  өткізу қабілеті; Torii байттарға және экспорттарға қызмет көрсету алдында күшіне енеді
  `torii_content_requests_total`, `torii_content_request_duration_seconds`, және
  Нәтиже белгілері бар `torii_content_response_bytes_total` көрсеткіштері. Кідіріс
  мақсаттар `content.target_p50_latency_ms` астында өмір сүреді /
  `content.target_p99_latency_ms` / `content.target_availability_bps`.
- **Қолданбалы басқару элементтері**: тариф шелектері UAID/API таңбалауышы/қашықтағы IP және
  қосымша PoW қорғанысы (`content.pow_difficulty_bits`, `content.pow_header`) мүмкін
  оқылмас бұрын қажет. DA жолағы орналасу әдепкі параметрлері осыдан келеді
  `content.stripe_layout` және түбіртектерде/манифест хэштерінде қайталанады.
- **Түбіртектер және DA дәлелдері**: сәтті жауаптар қоса беріледі
  `sora-content-receipt` (негізгі 64 Norito кадрлы `ContentDaReceipt` байт) тасымалдау
  `bundle_id`, `path`, `file_hash`, `served_bytes`, қызмет көрсетілетін байт ауқымы,
  `chunk_root` / `stripe_layout`, қосымша PDP міндеттемесі және уақыт белгісі
  клиенттер денені қайта оқымай-ақ алынған нәрсені түйре алады.

Негізгі сілтемелер:- Деректер үлгісі: `crates/iroha_data_model/src/content.rs`
- Орындалуы: `crates/iroha_core/src/smartcontracts/isi/content.rs`
- Torii өңдеушісі: `crates/iroha_torii/src/content.rs`
- CLI көмекшісі: `crates/iroha_cli/src/content.rs`
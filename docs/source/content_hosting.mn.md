---
lang: mn
direction: ltr
source: docs/source/content_hosting.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c0c7f98dbd9f49c573302f0b5cbe2e7a663d7fe35a1a9eea8da4f24c6f9bc8b
source_last_modified: "2026-01-05T18:22:23.402176+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Агуулга байршуулах эгнээ
% Iroha Үндсэн

# Агуулга байршуулах эгнээ

Агуулгын эгнээ нь жижиг статик багцуудыг (tar архив) сүлжээнд хадгалж, үйлчилдэг
Torii-ээс шууд бие даасан файлууд.

- **Нийтлэх**: `PublishContentBundle`-г tar архивын хамт илгээх, хугацаа нь дуусах
  өндөр, нэмэлт манифест. Багцын ID нь blake2b хэш юм
  тарбол. Tar оруулга нь ердийн файл байх ёстой; нэрс нь хэвийн UTF-8 зам юм.
  Хэмжээ/зам/файлын тоо толгой `content` тохиргооноос (`max_bundle_bytes`,
  `max_files`, `max_path_len`, `max_retention_blocks`, `chunk_size_bytes`).
  Манифестт Norito индексийн хэш, өгөгдлийн орон зай/ эгнээ, кэш бодлого орно.
  (`max_age_seconds`, `immutable`), баталгаажуулах горим (`public` / `role:<role>` /
  `sponsor:<uaid>`), хадгалах бодлогын орлуулагч болон MIME хүчингүй болгох.
- **Deduping**: давирхайн ачааллыг хэсэгчлэн (өгөгдмөл 64KiB) нэг удаа хадгалдаг.
  лавлагааны тоо бүхий хэш; retiring a bondle нь жижиг хэсгүүдийг багасгаж, prunes.
- **Үйлчилгээ**: Torii нь `GET /v1/content/{bundle}/{path}`-г харуулж байна. Хариултын урсгал
  `ETag` = файлын хэш, `Accept-Ranges: bytes` бүхий бөөгнөрсөн дэлгүүрээс шууд
  Мужийн дэмжлэг, манифестээс гаралтай Cache-Control. Хүндэтгэсэн уншдаг
  Манифест баталгаажуулах горим: дүрд хамаарах болон ивээн тэтгэгчтэй холбоотой хариултуудад каноник шаардлагатай
  гарын үсэг зурсан гарчиг (`X-Iroha-Account`, `X-Iroha-Signature`) хүсэлт
  данс; дутуу/хугацаа нь дууссан багцууд 404 буцаана.
- **CLI**: `iroha content publish --bundle <path.tar>` (эсвэл `--root <dir>`) одоо
  манифестийг автоматаар үүсгэж, нэмэлт `--manifest-out/--bundle-out` ялгаруулж,
  `--auth`, `--cache-max-age-secs`, `--dataspace`, `--lane`, `--immutable`,
  болон `--expires-at-height` хүчингүй болно. `iroha content pack --root <dir>` бүтээдэг
  юу ч оруулахгүйгээр тодорхойлогч tarball + манифест.
- **Тохиргоо**: кэш/auth товчлуурууд нь `content.*` доор `iroha_config` дээр ажилладаг
  (`default_cache_max_age_secs`, `max_cache_max_age_secs`, `immutable_bundles`,
  `default_auth_mode`) бөгөөд нийтлэх үед хэрэгжинэ.
- **SLO + хязгаар**: `content.max_requests_per_second` / `request_burst` болон
  `content.max_egress_bytes_per_second` / `egress_burst_bytes` таг унших талдаа
  дамжуулах чадвар; Torii нь байт болон экспортод үйлчлэхийн өмнө хоёуланг нь хэрэгжүүлдэг
  `torii_content_requests_total`, `torii_content_request_duration_seconds`, болон
  Үр дүнгийн шошготой `torii_content_response_bytes_total` хэмжүүрүүд. Хоцролт
  зорилтууд `content.target_p50_latency_ms` дор амьдардаг /
  `content.target_p99_latency_ms` / `content.target_availability_bps`.
- **Уурвуулан ашиглах хяналт**: Үнийн хувинг UAID/API токен/алсын IP болон
  нэмэлт PoW хамгаалагч (`content.pow_difficulty_bits`, `content.pow_header`)
  уншихаас өмнө шаардлагатай. DA зурвасын байршлын өгөгдмөл нь эндээс ирдэг
  `content.stripe_layout` ба хүлээн авалт/манифест хэш-д цуурайтаж байна.
- **Баримт ба DA-ийн нотлох баримт**: амжилттай хариултуудыг хавсаргана
  `sora-content-receipt` (суурь64 Norito хүрээтэй `ContentDaReceipt` байт)
  `bundle_id`, `path`, `file_hash`, `served_bytes`, үйлчлэх байт муж,
  `chunk_root` / `stripe_layout`, нэмэлт PDP амлалт, мөн ийм цагийн тэмдэг
  Үйлчлүүлэгчид биеийг дахин уншихгүйгээр авчирсан зүйлийг зүүх боломжтой.

Гол лавлагаа:- Өгөгдлийн загвар: `crates/iroha_data_model/src/content.rs`
- Гүйцэтгэл: `crates/iroha_core/src/smartcontracts/isi/content.rs`
- Torii зохицуулагч: `crates/iroha_torii/src/content.rs`
- CLI туслах: `crates/iroha_cli/src/content.rs`
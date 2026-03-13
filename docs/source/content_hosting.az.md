---
lang: az
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
% Iroha Əsas

# Məzmun Hosting Lane

Məzmun zolağı kiçik statik paketləri (tar arxivləri) zəncirdə saxlayır və xidmət göstərir
fərdi fayllar birbaşa Torii-dən.

- **Nəşr et**: tar arxivi ilə `PublishContentBundle` təqdim edin, isteğe bağlı istifadə müddəti
  hündürlük və isteğe bağlı manifest. Paket ID-si blake2b hash-dir
  tarbol. Tar girişləri adi fayllar olmalıdır; adlar normallaşdırılmış UTF-8 yollarıdır.
  Ölçü/yol/fayl sayı qapaqları `content` konfiqurasiyasından gəlir (`max_bundle_bytes`,
  `max_files`, `max_path_len`, `max_retention_blocks`, `chunk_size_bytes`).
  Manifestlərə Norito indeks hash, dataspace/zolaq, keş siyasəti daxildir
  (`max_age_seconds`, `immutable`), auth rejimi (`public` / `role:<role>` /
  `sponsor:<uaid>`), saxlama siyasəti yertutanı və MIME ləğv edir.
- **Deduping**: tar yükləri parçalanır (defolt 64KiB) və hər dəfə bir dəfə saxlanılır
  istinad sayıları ilə hash; bir paketin təqaüdə çıxması parçaları azaldır və budaqlanır.
- **Xidmət et**: Torii `GET /v2/content/{bundle}/{path}`-i ifşa edir. Cavab axını
  birbaşa yığın mağazasından `ETag` = fayl hash, `Accept-Ranges: bytes`,
  Range dəstəyi və manifestdən əldə edilən Cache-Control. Şərəf oxuyur
  manifest auth rejimi: rol qapılı və sponsor qapalı cavablar kanonik tələb edir
  imzalanmış üçün sorğu başlıqları (`X-Iroha-Account`, `X-Iroha-Signature`)
  hesab; əskik/müddəti bitmiş paketlər 404 qaytarır.
- **CLI**: indi `iroha content publish --bundle <path.tar>` (və ya `--root <dir>`)
  avtomatik olaraq manifest yaradır, isteğe bağlı `--manifest-out/--bundle-out` yayır və
  qəbul edir `--auth`, `--cache-max-age-secs`, `--dataspace`, `--lane`, `--immutable`,
  və `--expires-at-height` ləğv edir. `iroha content pack --root <dir>` qurur
  deterministik tarball + heç bir şey təqdim etmədən manifest.
- **Konfiqurasiya**: keş/auth düymələri `iroha_config`-də `content.*` altında yaşayır
  (`default_cache_max_age_secs`, `max_cache_max_age_secs`, `immutable_bundles`,
  `default_auth_mode`) və dərc zamanı tətbiq edilir.
- **SLO + limitlər**: `content.max_requests_per_second` / `request_burst` və
  `content.max_egress_bytes_per_second` / `egress_burst_bytes` qapaq oxu tərəfi
  ötürmə qabiliyyəti; Torii həm baytlara, həm də ixraclara xidmət etməzdən əvvəl tətbiq edir
  `torii_content_requests_total`, `torii_content_request_duration_seconds` və
  Nəticə etiketləri ilə `torii_content_response_bytes_total` göstəriciləri. Gecikmə
  hədəflər `content.target_p50_latency_ms` altında yaşayır /
  `content.target_p99_latency_ms` / `content.target_availability_bps`.
- **Sui-istifadəyə nəzarət**: Qiymət kovaları UAID/API işarəsi/uzaq IP və
  isteğe bağlı PoW qoruyucusu (`content.pow_difficulty_bits`, `content.pow_header`) bilər
  oxumadan əvvəl tələb olunur. DA zolaq düzümü defoltları buradan gəlir
  `content.stripe_layout` və qəbzlərdə/manifest heşlərində əks olunur.
- ** Qəbzlər və DA sübutları**: uğurlu cavablar əlavə olunur
  `sora-content-receipt` (base64 Norito çərçivəli `ContentDaReceipt` bayt) daşıyan
  `bundle_id`, `path`, `file_hash`, `served_bytes`, xidmət edilən bayt diapazonu,
  `chunk_root` / `stripe_layout`, isteğe bağlı PDP öhdəliyi və buna görə vaxt damğası
  müştərilər cəsədi yenidən oxumadan gətirilənləri pin edə bilərlər.

Əsas istinadlar:- Məlumat modeli: `crates/iroha_data_model/src/content.rs`
- İcra: `crates/iroha_core/src/smartcontracts/isi/content.rs`
- Torii işləyicisi: `crates/iroha_torii/src/content.rs`
- CLI köməkçisi: `crates/iroha_cli/src/content.rs`
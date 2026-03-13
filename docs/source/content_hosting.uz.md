---
lang: uz
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
% Iroha yadrosi

# Kontent hosting yo'li

Kontent qatori kichik statik to'plamlarni (tar arxivlari) zanjirda saqlaydi va xizmat qiladi
individual fayllar to'g'ridan-to'g'ri Torii dan.

- **Publish**: `PublishContentBundle` tar arxivi bilan yuboring, ixtiyoriy muddati
  balandligi va ixtiyoriy manifest. To'plam identifikatori blake2b xeshi hisoblanadi
  tarbol. Tar yozuvlari oddiy fayllar bo'lishi kerak; nomlari normallashtirilgan UTF-8 yo'llari.
  Oʻlcham/yoʻl/fayllarni hisoblash cheklari `content` konfiguratsiyasidan (`max_bundle_bytes`,
  `max_files`, `max_path_len`, `max_retention_blocks`, `chunk_size_bytes`).
  Manifestlarga Norito indeksli xesh, maʼlumotlar maydoni/yoʻlak, kesh siyosati kiradi.
  (`max_age_seconds`, `immutable`), autentifikatsiya rejimi (`public` / `role:<role>` /
  `sponsor:<uaid>`), saqlash siyosati to'ldiruvchisi va MIME bekor qiladi.
- **Deduping**: smolaning foydali yuklari qismlarga bo'linadi (standart 64KiB) va har bir marta saqlanadi
  mos yozuvlar soni bilan xesh; bir to'plamni pensiyaga chiqarish bo'laklarni kamaytiradi va olxo'ri kesadi.
- **Xizmat ko'rsatish**: Torii `GET /v2/content/{bundle}/{path}`ni ko'rsatadi. Javoblar oqimi
  to'g'ridan-to'g'ri parcha do'konidan `ETag` = fayl xeshi, `Accept-Ranges: bytes`,
  Manifestdan olingan diapazonni qo'llab-quvvatlash va kesh-nazorat. hurmat bilan o'qiydi
  manifest auth rejimi: rolga va homiyga bog'langan javoblar kanonik talablarni talab qiladi
  imzolangan uchun so'rov sarlavhalari (`X-Iroha-Account`, `X-Iroha-Signature`)
  hisob; etishmayotgan/muddati o'tgan to'plamlar 404 qaytaradi.
- **CLI**: hozir `iroha content publish --bundle <path.tar>` (yoki `--root <dir>`)
  manifestni avtomatik yaratadi, ixtiyoriy `--manifest-out/--bundle-out` chiqaradi va
  qabul qiladi `--auth`, `--cache-max-age-secs`, `--dataspace`, `--lane`, `--immutable`,
  va `--expires-at-height` bekor qiladi. `iroha content pack --root <dir>` quradi
  deterministik tarball + hech narsa topshirmasdan manifest.
- **Konfiguratsiya**: kesh/auth tugmalari `content.*` ostida `iroha_config` da ishlaydi
  (`default_cache_max_age_secs`, `max_cache_max_age_secs`, `immutable_bundles`,
  `default_auth_mode`) va nashr qilish vaqtida amalga oshiriladi.
- **SLO + chegaralar**: `content.max_requests_per_second` / `request_burst` va
  `content.max_egress_bytes_per_second` / `egress_burst_bytes` qopqog'ini o'qish tomoni
  o'tkazish qobiliyati; Torii baytlarga xizmat ko'rsatish va eksport qilishdan oldin ham amal qiladi
  `torii_content_requests_total`, `torii_content_request_duration_seconds` va
  Natija yorliqlari bilan `torii_content_response_bytes_total` ko'rsatkichlari. Kechikish
  maqsadlar `content.target_p50_latency_ms` ostida yashaydi /
  `content.target_p99_latency_ms` / `content.target_availability_bps`.
- **Suiiste'mol boshqaruvlari**: tarif chelaklari UAID/API tokeni/masofaviy IP va
  ixtiyoriy PoW himoyasi (`content.pow_difficulty_bits`, `content.pow_header`) mumkin
  o'qishdan oldin talab qilinadi. DA chiziqli tartibining standart sozlamalari kelib chiqadi
  `content.stripe_layout` va kvitansiyalar/manifest xeshlarida aks ettiriladi.
- **Kvitansiya va DA dalillar**: muvaffaqiyatli javoblar ilova qilinadi
  `sora-content-receipt` (base64 Norito ramkali `ContentDaReceipt` bayt)
  `bundle_id`, `path`, `file_hash`, `served_bytes`, xizmat koʻrsatilgan bayt diapazoni,
  `chunk_root` / `stripe_layout`, ixtiyoriy PDP majburiyati va shunday vaqt tamg'asi
  mijozlar tanani qayta o'qimasdan olib kelingan narsani pin qilishlari mumkin.

Asosiy havolalar:- Ma'lumotlar modeli: `crates/iroha_data_model/src/content.rs`
- Bajarish: `crates/iroha_core/src/smartcontracts/isi/content.rs`
- Torii ishlov beruvchisi: `crates/iroha_torii/src/content.rs`
- CLI yordamchisi: `crates/iroha_cli/src/content.rs`
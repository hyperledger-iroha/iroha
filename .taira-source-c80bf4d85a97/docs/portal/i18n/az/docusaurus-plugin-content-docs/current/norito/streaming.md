---
lang: az
direction: ltr
source: docs/portal/docs/norito/streaming.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito Axın

Norito Streaming tel formatını, idarəetmə çərçivələrini və istinad kodekini müəyyən edir
Torii və SoraNet arasında canlı media axınları üçün istifadə olunur. Kanonik spesifikasiyalar yaşayır
`norito_streaming.md` iş sahəsinin kökündə; bu səhifə parçaları distillə edir
operatorlar və SDK müəllifləri konfiqurasiya əlaqə nöqtələri ilə yanaşı lazımdır.

## Tel formatı və idarəetmə müstəvisi

- **Manifestlər və çərçivələr.** `ManifestV1` və `PrivacyRoute*` seqmenti təsvir edir
  vaxt qrafiki, yığın deskriptorları və marşrut göstərişləri. İdarəetmə çərçivələri (`KeyUpdate`,
  `ContentKeyUpdate` və kadans rəyi) manifestlə yanaşı yaşayır
  izləyicilər deşifrədən əvvəl öhdəlikləri təsdiq edə bilərlər.
- **Baseline codec.** `BaselineEncoder`/`BaselineDecoder` monotonluğu tətbiq edir
  yığın identifikatorları, vaxt damğası arifmetikası və öhdəliklərin yoxlanılması. Ev sahibləri zəng etməlidir
  İzləyicilərə və ya relelərə xidmət etməzdən əvvəl `EncodedSegment::verify_manifest`.
- **Funksiya bitləri.** Bacarıq danışıqları `streaming.feature_bits`-i reklam edir
  (defolt `0b11` = əsas rəy + məxfilik marşrutu təminatçısı) beləliklə rele və
  Müştərilər deterministik olaraq qabiliyyətləri uyğunlaşdırmadan həmyaşıdlarını rədd edə bilərlər.

## Açarlar, suitlər və kadans

- **İdentifikasiya tələbləri.** Streaming nəzarət çərçivələri həmişə ilə imzalanır
  Ed25519. Xüsusi açarlar vasitəsilə təmin edilə bilər
  `streaming.identity_public_key`/`streaming.identity_private_key`; əks halda
  node şəxsiyyəti təkrar istifadə olunur.
- **HPKE dəstləri.** `KeyUpdate` ən aşağı ümumi paketi seçir; №1 dəst
  məcburi (`AuthPsk`, `Kyber768`, `HKDF-SHA3-256`, `ChaCha20-Poly1305`), ilə
  isteğe bağlı `Kyber1024` təkmilləşdirmə yolu. Suite seçimi üzərində saxlanılır
  sessiya və hər yeniləmədə təsdiqlənir.
- **Rotasiya.** Nəşriyyatçılar hər 64MiB və ya 5 dəqiqədən bir imzalanmış `KeyUpdate` yayırlar.
  `key_counter` ciddi şəkildə artırılmalıdır; reqressiya çətin bir səhvdir.
  `ContentKeyUpdate` yuvarlanan Qrup Məzmun Açarını paylayır, altına bükülmüşdür.
  razılaşdırılmış HPKE dəsti və ID + etibarlılıq ilə qapılar seqmentinin şifrəsinin açılması
  pəncərə.
- **Ana görüntülər.** `StreamingSession::snapshot_state` və
  `restore_from_snapshot` davam edir `{session_id, key_counter, suite, sts_root,
  kadans vəziyyəti}` under `streaming.session_store_dir` (defolt
  `./storage/streaming`). Nəqliyyat açarları bərpa edildikdə yenidən əldə edilir, beləliklə qəzalar baş verir
  sessiya sirlərini sızdırmayın.

## İş vaxtı konfiqurasiyası

- **Açar material.** Xüsusi açarları ilə təchiz edin
  `streaming.identity_public_key`/`streaming.identity_private_key` (Ed25519)
  multihash) və isteğe bağlı Kyber materialı vasitəsilə
  `streaming.kyber_public_key`/`streaming.kyber_secret_key`. Dördü də olmalıdır
  defoltları ləğv edərkən mövcuddur; `streaming.kyber_suite` qəbul edir
  `mlkem512|mlkem768|mlkem1024` (ləqəblər `kyber512/768/1024`, defolt
  `mlkem768`).
- **Kodek qoruyucuları.** Quraşdırma imkan vermədiyi halda CABAC qeyri-aktiv qalır;
  paketlənmiş rANS `ENABLE_RANS_BUNDLES=1` tələb edir. vasitəsilə tətbiq edin
  `streaming.codec.{entropy_mode,bundle_width,bundle_accel}` və isteğe bağlıdır
  Xüsusi masalar təmin edərkən `streaming.codec.rans_tables_path`. Paketli
- **SoraNet marşrutları.** `streaming.soranet.*` anonim nəqliyyata nəzarət edir:
  `exit_multiaddr` (defolt `/dns/torii/udp/9443/quic`), `padding_budget_ms`
  (standart 25ms), `access_kind` (`authenticated` vs `read-only`), isteğe bağlı
  `channel_salt`, `provision_spool_dir` (defolt
  `./storage/streaming/soranet_routes`), `provision_spool_max_bytes` (defolt 0,
  limitsiz), `provision_window_segments` (standart 4) və
  `provision_queue_capacity` (defolt 256).
- **Sinxronizasiya qapısı.** `streaming.sync` audiovizual üçün drift tətbiqini dəyişdirir
  axınlar: `enabled`, `observe_only`, `ewma_threshold_ms` və `hard_cap_ms`
  seqmentlərin vaxtın sürüşməsinə görə rədd edildiyini idarə edir.

## Doğrulama və qurğular

- Canonical tip tərifləri və köməkçiləri yaşayır
  `crates/iroha_crypto/src/streaming.rs`.
- İnteqrasiya əhatə dairəsi HPKE əl sıxma, məzmun açarının paylanması,
  və snapshot həyat dövrü (`crates/iroha_crypto/tests/streaming_handshake.rs`).
  Axını yoxlamaq üçün `cargo test -p iroha_crypto streaming_handshake`-i işə salın
  yerli səth.
- Dizayn, səhvlərin idarə edilməsi və gələcək təkmilləşdirmələrə dərindən nəzər salmaq üçün oxuyun
  Repozitor kökündə `norito_streaming.md`.
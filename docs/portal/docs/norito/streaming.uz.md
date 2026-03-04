---
lang: uz
direction: ltr
source: docs/portal/docs/norito/streaming.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9df713c3e078ac2ccbd74eb215b91bb80d08306d0ca455dc122fde535601ce8
source_last_modified: "2026-01-18T10:42:52.828202+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Norito Oqim

Norito Streaming sim formatini, boshqaruv ramkalarini va mos yozuvlar kodekini belgilaydi
Torii va SoraNet bo'ylab jonli media oqimlari uchun ishlatiladi. Kanonik spetsifikatsiya yashaydi
Ish maydoni ildizida `norito_streaming.md`; bu sahifa bo'laklarni distillaydi
operatorlar va SDK mualliflari konfiguratsiya aloqa nuqtalari bilan bir qatorda kerak.

## Sim formati va boshqaruv tekisligi

- **Manifestlar va ramkalar.** `ManifestV1` va `PrivacyRoute*` segmentni tavsiflaydi
  vaqt jadvali, parcha identifikatorlari va marshrut bo'yicha maslahatlar. Boshqarish ramkalari (`KeyUpdate`,
  `ContentKeyUpdate` va kadans teskarisi) manifest bilan birga yashaydi
  tomoshabinlar dekodlashdan oldin majburiyatlarni tasdiqlashlari mumkin.
- **Asosiy kodek.** `BaselineEncoder`/`BaselineDecoder` monotonlikni ta'minlaydi
  parcha identifikatorlari, vaqt tamg'asi arifmetikasi va majburiyatlarni tekshirish. Xostlar qo'ng'iroq qilishlari kerak
  `EncodedSegment::verify_manifest` tomoshabinlarga yoki releylarga xizmat ko'rsatishdan oldin.
- **Funksiya bitlari.** Imkoniyatlarni muhokama qilish `streaming.feature_bits` reklama qiladi
  (standart `0b11` = asosiy fikr-mulohaza + maxfiylik marshruti provayderi) shuning uchun releylar va
  Mijozlar deterministik ravishda qobiliyatlarga mos kelmasdan tengdoshlarini rad etishlari mumkin.

## Kalitlar, to'plamlar va kadans

- **Identifikatsiya talablari.** Streaming boshqaruv ramkalari har doim bilan imzolanadi
  Ed25519. Maxsus kalitlar orqali etkazib berilishi mumkin
  `streaming.identity_public_key`/`streaming.identity_private_key`; aks holda
  tugun identifikatori qayta ishlatiladi.
- **HPKE to'plamlari.** `KeyUpdate` eng past umumiy to'plamni tanlaydi; №1 to'plam
  majburiy (`AuthPsk`, `Kyber768`, `HKDF-SHA3-256`, `ChaCha20-Poly1305`), bilan
  ixtiyoriy `Kyber1024` yangilash yo'li. Suite tanlovi sahifada saqlanadi
  sessiya va har bir yangilanishda tasdiqlangan.
- **Rotatsiya.** Noshirlar har 64MiB yoki 5 daqiqada imzolangan `KeyUpdate` chiqaradi.
  `key_counter` qat'iy ravishda oshishi kerak; regressiya - bu qiyin xato.
  `ContentKeyUpdate` tagiga oʻralgan holda harakatlanuvchi guruh kontent kalitini tarqatadi.
  kelishilgan HPKE to'plami va geyts segmentini ID + amal qilish muddati bo'yicha dekodlash
  oyna.
- **Snapshotlar.** `StreamingSession::snapshot_state` va
  `restore_from_snapshot` doimiy `{session_id, key_counter, suite, sts_root,
  kadans holati}` under `streaming.session_store_dir` (standart
  `./storage/streaming`). Transport kalitlari qayta tiklanganda qaytadan olinadi, shuning uchun ishdan chiqadi
  seans sirlarini oshkor qilmang.

## Ish vaqti konfiguratsiyasi

- **Kalit materiali.** Maxsus kalitlarni taqdim eting
  `streaming.identity_public_key`/`streaming.identity_private_key` (Ed25519)
  multihash) va ixtiyoriy Kyber materiali orqali
  `streaming.kyber_public_key`/`streaming.kyber_secret_key`. To'rttasi bo'lishi kerak
  standart qiymatlarni bekor qilishda mavjud; `streaming.kyber_suite` qabul qiladi
  `mlkem512|mlkem768|mlkem1024` (taxalluslar `kyber512/768/1024`, standart
  `mlkem768`).
- **Kodek to'siqlari.** Qurilish imkon bermaguncha CABAC o'chirilgan bo'lib qoladi;
  paketlangan rANS uchun `ENABLE_RANS_BUNDLES=1` talab qilinadi. orqali amalga oshirish
  `streaming.codec.{entropy_mode,bundle_width,bundle_accel}` va ixtiyoriy
  Maxsus jadvallarni taqdim etishda `streaming.codec.rans_tables_path`. Birlashtirilgan
- **SoraNet marshrutlari.** `streaming.soranet.*` anonim transportni boshqaradi:
  `exit_multiaddr` (standart `/dns/torii/udp/9443/quic`), `padding_budget_ms`
  (standart 25ms), `access_kind` (`authenticated` va `read-only`), ixtiyoriy
  `channel_salt`, `provision_spool_dir` (standart
  `./storage/streaming/soranet_routes`), `provision_spool_max_bytes` (standart 0,
  cheksiz), `provision_window_segments` (standart 4) va
  `provision_queue_capacity` (standart 256).
- **Sinxronlash eshigi.** `streaming.sync` audiovizual uchun driftni qo‘llashni yoqadi
  oqimlar: `enabled`, `observe_only`, `ewma_threshold_ms` va `hard_cap_ms`
  vaqt o'zgarishi uchun segmentlar rad etilganda boshqarish.

## Tasdiqlash va moslamalar

- Kanonik turdagi ta'riflar va yordamchilar yashaydi
  `crates/iroha_crypto/src/streaming.rs`.
- Integratsiya qamrovi HPKE qo'l siqish, kontent-kalitlarni taqsimlash,
  va suratning hayot aylanishi (`crates/iroha_crypto/tests/streaming_handshake.rs`).
  Oqimni tekshirish uchun `cargo test -p iroha_crypto streaming_handshake` ni ishga tushiring
  mahalliy sirt.
- Tartibga chuqur kirib borish, xatolarni boshqarish va kelajakdagi yangilanishlar uchun o'qing
  `norito_streaming.md` ombor ildizida.
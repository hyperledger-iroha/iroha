---
lang: mn
direction: ltr
source: docs/portal/docs/norito/streaming.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Norito Дамжуулж байна

Norito Streaming нь утасны формат, хяналтын хүрээ болон лавлагааны кодлогчийг тодорхойлдог.
Torii болон SoraNet даяар шууд хэвлэл мэдээллийн урсгалд ашигладаг. Каноник үзүүлэлт нь амьдардаг
Ажлын талбарын үндэс дээр `norito_streaming.md`; Энэ хуудас нь тэр хэсгүүдийг нэрдэг
операторууд болон SDK зохиогчид тохиргооны мэдрэгчтэй цэгүүдийн хажууд хэрэгтэй.

## Утасны формат ба хяналтын хавтгай

- **Манифест ба хүрээ.** `ManifestV1` болон `PrivacyRoute*` сегментийг дүрсэлдэг.
  цагийн хуваарь, хэсэгчилсэн тодорхойлогч, чиглүүлэлтийн зөвлөмжүүд. Хяналтын хүрээ (`KeyUpdate`,
  `ContentKeyUpdate`, хэмнэлтийн санал) манифесттэй зэрэгцэн амьдардаг
  Үзэгчид код тайлахын өмнө амлалтаа баталгаажуулах боломжтой.
- **Үндсэн кодлогч.** `BaselineEncoder`/`BaselineDecoder` нь монотоникийг хэрэгжүүлдэг
  бөөгнөрөл ID, цагийн тэмдгийн арифметик, амлалт баталгаажуулалт. Хостууд залгах ёстой
  `EncodedSegment::verify_manifest` үзэгчид эсвэл релейд үйлчлэхээс өмнө.
- **Онцлогын битүүд.** Чадварын хэлэлцээр нь `streaming.feature_bits`-г сурталчилдаг
  (өгөгдмөл `0b11` = үндсэн санал хүсэлт + нууцлалын чиглүүлэлтийн үйлчилгээ үзүүлэгч) тиймээс реле болон
  Үйлчлүүлэгчид чадвараа тодорхойлохгүйгээр үе тэнгийнхнээсээ татгалзаж болно.

## Түлхүүрүүд, иж бүрдэл, хэмнэл

- **Идентификаторын шаардлага.** Урсгалын хяналтын хүрээ нь үргэлж гарын үсэг зурсан байдаг
  Ed25519. Зориулалтын түлхүүрүүдийг дамжуулан нийлүүлж болно
  `streaming.identity_public_key`/`streaming.identity_private_key`; өөрөөр
  зангилааны таних тэмдэг дахин ашиглагдана.
- **HPKE иж бүрдэл.** `KeyUpdate` нь хамгийн бага нийтлэг багцыг сонгоно; Suite №1 нь
  заавал (`AuthPsk`, `Kyber768`, `HKDF-SHA3-256`, `ChaCha20-Poly1305`), хамт
  нэмэлт `Kyber1024` шинэчлэх зам. Suite сонголт дээр хадгалагдана
  сесс болон шинэчлэлт бүрт баталгаажуулдаг.
- **Эргүүлэх.** Хэвлэн нийтлэгчид 64MiB эсвэл 5 минут тутамд `KeyUpdate` гарын үсэг зурдаг.
  `key_counter` хатуу нэмэгдэх ёстой; регресс бол хэцүү алдаа юм.
  `ContentKeyUpdate` нь эргэлдэж буй группын агуулгын түлхүүрийг тарааж, доор нь ороосон.
  тохиролцсон HPKE иж бүрдэл, ID + хүчинтэй байдлын дагуу хаалганы сегментийн шифрийг тайлах
  цонх.
- **Агшин зуурын зураг.** `StreamingSession::snapshot_state` болон
  `restore_from_snapshot` хэвээр `{session_id, key_counter, suite, sts_root,
  хэмнэлтийн төлөв}` under `streaming.session_store_dir` (өгөгдмөл
  `./storage/streaming`). Тээврийн түлхүүрүүдийг сэргээх үед дахин гаргаж авдаг тул гацах болно
  сессийн нууцыг бүү задруул.

## Ажиллах цагийн тохиргоо

- **Түлхүүр материал.** Зориулалтын түлхүүрүүдийг нийлүүлнэ
  `streaming.identity_public_key`/`streaming.identity_private_key` (Ed25519)
  multihash) болон нэмэлт Kyber материалаар дамжуулан
  `streaming.kyber_public_key`/`streaming.kyber_secret_key`. Дөрөв нь байх ёстой
  өгөгдмөлүүдийг хүчингүй болгох үед байх; `streaming.kyber_suite` зөвшөөрнө
  `mlkem512|mlkem768|mlkem1024` (хоол нэр `kyber512/768/1024`, анхдагч
  `mlkem768`).
- **Кодекийн хамгаалалтын хашлага.** Бүтээлт үүнийг идэвхжүүлээгүй тохиолдолд CABAC идэвхгүй хэвээр байна;
  багцалсан rANS-д `ENABLE_RANS_BUNDLES=1` шаардлагатай. дамжуулан хэрэгжүүлэх
  `streaming.codec.{entropy_mode,bundle_width,bundle_accel}` болон нэмэлт
  Захиалгат хүснэгтүүдийг нийлүүлэх үед `streaming.codec.rans_tables_path`. Багцалсан
- **SoraNet чиглүүлэлтүүд.** `streaming.soranet.*` нь нэргүй тээвэрлэлтийг хянадаг:
  `exit_multiaddr` (өгөгдмөл `/dns/torii/udp/9443/quic`), `padding_budget_ms`
  (өгөгдмөл 25мс), `access_kind` (`authenticated` vs `read-only`), нэмэлт
  `channel_salt`, `provision_spool_dir` (өгөгдмөл
  `./storage/streaming/soranet_routes`), `provision_spool_max_bytes` (өгөгдмөл 0,
  хязгааргүй), `provision_window_segments` (анхдагч 4), болон
  `provision_queue_capacity` (өгөгдмөл 256).
- **Sync gate.** `streaming.sync` нь аудиовизуалд зориулсан дрифийн хэрэгжилтийг унтраадаг.
  дамжуулалт: `enabled`, `observe_only`, `ewma_threshold_ms`, болон `hard_cap_ms`
  сегментүүдийг цаг хугацааны зөрүүнээс татгалзсан үед удирдах.

## Баталгаажуулалт ба бэхэлгээ

- Каноник төрлийн тодорхойлолтууд болон туслахууд амьдардаг
  `crates/iroha_crypto/src/streaming.rs`.
- Интеграцийн хамрах хүрээ нь HPKE гар барих, агуулгын түлхүүр хуваарилалт,
  болон хормын хувилбарын амьдралын мөчлөг (`crates/iroha_crypto/tests/streaming_handshake.rs`).
  Дамжуулалтыг шалгахын тулд `cargo test -p iroha_crypto streaming_handshake`-г ажиллуул
  орон нутгийн гадаргуу.
- Зохион байгуулалт, алдаатай харьцах, цаашдын шинэчлэлтийн талаар гүнзгий судлахын тулд уншина уу
  Repository root-д `norito_streaming.md`.
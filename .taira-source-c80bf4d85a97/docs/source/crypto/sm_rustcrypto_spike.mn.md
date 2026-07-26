---
lang: mn
direction: ltr
source: docs/source/crypto/sm_rustcrypto_spike.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f133d9489c4bcfae2212e6c5dc098f39c3dea3e5cd42855ba76e8c9b73b4d03
source_last_modified: "2025-12-29T18:16:35.946614+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! RustCrypto SM интеграцийн огцом өсөлтийн талаархи тэмдэглэл.

# RustCrypto SM баяжуулалтын тэмдэглэл

## Зорилго
RustCrypto-н `sm2`, `sm3`, `sm4` хайрцагуудыг (нэмэх `rfc6979`, `ccm`, ```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
```. `iroha_crypto` хайрцаг ба функцийн тугийг илүү өргөн ажлын талбарт холбохоос өмнө бүтээхэд зөвшөөрөгдөх хугацааг өгдөг.

## Санал болгож буй хараат байдлын газрын зураг

| Хайрцаг | Санал болгож буй хувилбар | Онцлогууд | Тэмдэглэл |
|-------|-------------------|----------|-------|
| `sm2` | `0.13` (RustCrypto/гарын үсэг) | `std` | `elliptic-curve`-ээс хамаарна; MSRV ажлын талбартай таарч байгаа эсэхийг шалгана уу. |
| `sm3` | `0.5.0-rc.1` (RustCrypto/хэшүүд) | анхдагч | API нь `sha2`-тэй зэрэгцээ, одоо байгаа `digest` шинж чанаруудтай нэгддэг. |
| `sm4` | `0.5.1` (RustCrypto/block-ciphers) | анхдагч | Шифрийн шинж чанаруудтай ажилладаг; AEAD боодол нь дараа нь баяжуулалтыг хойшлуулсан. |
| `rfc6979` | `0.4` | анхдагч | Детерминист бус деривацийг дахин ашиглах. |

*Хувилбарууд нь 2024-12 оны одоогийн хувилбаруудыг тусгасан; буухын өмнө `cargo search`-р баталгаажуулна уу.*

## Илэрхий өөрчлөлтүүд (ноорог)

```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
```

Хяналт: `iroha_crypto` (одоогоор `0.13.8`) дээр байгаа хувилбаруудтай тааруулахын тулд `elliptic-curve` зүү.

## Spike шалгах хуудас
- [x] `crates/iroha_crypto/Cargo.toml`-д нэмэлт хамаарал болон функцийг нэмнэ үү.
- [x] утсыг баталгаажуулахын тулд `signature::sm` модулийг `cfg(feature = "sm")`-ийн ард орлуулагчийн бүтэцтэй үүсгэнэ үү.
- [x] эмхэтгэлийг баталгаажуулахын тулд `cargo check -p iroha_crypto --features sm`-г ажиллуул; бүтээх хугацаа болон шинэ хамаарлын тоог бүртгэ (`cargo tree --features sm`).
- [x] `cargo check -p iroha_crypto --features sm --locked`-ээр зөвхөн std-ийн байрлалыг баталгаажуулна уу; `no_std` загваруудыг дэмжихээ больсон.
- [x] `docs/source/crypto/sm_program.md` дээрх файлын үр дүн (цаг хугацаа, хамаарлын модны гурвалжин).

## Баривчлах ажиглалт
- Нэмэлт эмхэтгэх хугацаа, суурь үзүүлэлт.
- `cargo builtinsize`-тэй хоёртын хэмжээний нөлөөлөл (хэрэв хэмжих боломжтой бол).
- Аливаа MSRV эсвэл онцлог зөрчил (жишээ нь, `elliptic-curve` бага хувилбаруудтай).
- Урсгалын өмнөх засваруудыг шаардаж болзошгүй анхааруулга (аюултай код, const-fn гарц).

## Хүлээгдэж буй зүйлс
- Ажлын талбайн хамаарлын графикийг хөөрөгдөхөөс өмнө Crypto WG-ийн зөвшөөрлийг хүлээнэ үү.
- Худалдагчтай хайрцагыг хянуулах эсвэл crates.io-д найдах эсэхээ баталгаажуулна уу (толь шаардлагатай байж болно).
- Хяналтын хуудсыг дууссан гэж тэмдэглэхээс өмнө `Cargo.lock` шинэчлэлтийг `sm_lock_refresh_plan.md` болгон тохируулна уу.
- Түгжигдсэн файл болон хамаарлын модыг сэргээх зөвшөөрөл олгосны дараа `scripts/sm_lock_refresh.sh`-г ашиглана.

## 2025-01-19 Spike Log
- Нэмэлт хамаарал (`sm2 0.13`, `sm3 0.5.0-rc.1`, `sm4 0.5.1`, `rfc6979 0.4`) болон `sm` `iroha_crypto`-д онцлог шинж чанаруудын туг нэмсэн.
- Эмхэтгэх явцад хэш/блок шифр API ашиглахын тулд `signature::sm` модуль.
- `cargo check -p iroha_crypto --features sm --locked` одоо хамаарлын графикийг шийдэж байгаа боловч `Cargo.lock` шинэчлэлтийн шаардлагаар цуцална; Хадгалах сангийн бодлого нь түгжээний файлыг засварлахыг хориглодог тул бид зөвшөөрөгдсөн түгжээний шинэчлэлтийг зохицуулах хүртэл хөрвүүлэх ажил хүлээгдэж байна.## 2026-02-12 Spike Log
- Өмнөх түгжигч файлыг хориглогчийг шийдсэн—хамааралууд нь аль хэдийн баригдсан тул `cargo check -p iroha_crypto --features sm --locked` амжилттай болсон (dev Mac дээр 7.9 секундын хүйтэн хувилбар; 0.23 секундын давтамжтайгаар дахин ажиллуулах).
- `cargo check -p iroha_crypto --no-default-features --features "std sm" --locked` нь 1.0 секундын дотор дамждаг бөгөөд энэ нь зөвхөн `std` тохиргоонд (`no_std` зам үлдэхгүй) нэмэлт функцүүдийн хөрвүүлэлтийг баталгаажуулдаг.
- `sm` функцийг идэвхжүүлсэн хараат дельта нь 11 хайрцгийг танилцуулж байна: `base64ct`, `ghash`, `opaque-debug`, `pem-rfc7468`, `pem-rfc7468`, ```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
``` `primeorder`, `sm2`, `sm3`, `sm4`, `sm4-gcm`. (`rfc6979` аль хэдийн суурь графикийн нэг хэсэг байсан.)
- Ашиглагдаагүй NEON бодлогын туслагчдад зориулж бүтээх анхааруулга хэвээр байна; Хэмжилтийг жигдрүүлэх хугацаа тэдгээр кодын замыг дахин идэвхжүүлэх хүртэл байгаагаар нь үлдээгээрэй.
---
lang: mn
direction: ltr
source: docs/source/crypto/dependency_audits.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 04e4cf26ed0ce9f9782be8aae9d16425a7a87fdbd1986cbcbca68a27ba0a3afe
source_last_modified: "2025-12-29T18:16:35.939138+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Крипто хамаарлын аудит

## Streebog (`streebog` хайрцаг)

- **Мод дахь хувилбар:** `0.11.0-rc.2`-ийг `vendor/streebog`-ийн дагуу үйлдвэрлэсэн (`gost` функц идэвхжсэн үед ашиглагддаг).
- **Хэрэглэгч:** `crates/iroha_crypto::signature::gost` (HMAC-Streebog DRBG + мессеж хэшлэх).
- ** Статус:** Зөвхөн нэр дэвшигчийг чөлөөлнө. Одоогоор ямар ч RC бус хайрцаг шаардлагатай API гадаргууг санал болгодог.
  Тиймээс бид эцсийн хувилбарыг гаргахын тулд дээд урсгалыг хянахын зэрэгцээ аудит хийх боломжтой болгох үүднээс модон доторх хайрцагыг тусгадаг.
- **Хяналтын цэгүүдийг шалгах:**
  - Wycheproof иж бүрдэл болон TC26 төхөөрөмжүүдийн эсрэг баталгаажуулсан хэш гаралт
    `cargo test -p iroha_crypto --features gost` (`crates/iroha_crypto/tests/gost_wycheproof.rs`-г үзнэ үү).
  - `cargo bench -p iroha_crypto --bench gost_sign --features gost`
    Одоогийн хамаарал бүхий TC26 муруй бүрийн хажууд Ed25519/Secp256k1 дасгал хийдэг.
  - `cargo run -p iroha_crypto --bin gost_perf_check --features gost`
    Шинэ хэмжилтийг бүртгэгдсэн медиантай харьцуулна (CI-д `--summary-only`-г ашиглана уу, нэмнэ үү.
    Дахин суурь тогтоох үед `--write-baseline crates/iroha_crypto/benches/gost_perf_baseline.json`).
  - `scripts/gost_bench.sh` вандан сандал ороож + урсгалыг шалгах; JSON-г шинэчлэхийн тулд `--write-baseline`-г дамжуулна уу.
    Төгсгөлийн ажлын урсгалыг `docs/source/crypto/gost_performance.md`-ээс үзнэ үү.
- **Хамууруулах арга хэмжээ:** `streebog` нь зөвхөн түлхүүрүүдийг тэг болгодог детерминист боодолоор дамжуулан дуудагддаг;
  гарын үсэг зурсан хүн RNG сүйрлийн бүтэлгүйтлээс зайлсхийхийн тулд үйлдлийн системийн энтропи бүхий noces-ийг хамгаалдаг.
- **Дараагийн үйлдлүүд:** RustCrypto-ийн `0.11.x` хувилбарыг дагаарай; шошго газардсаны дараа, эмчлэх
  стандарт хамаарлын овойлт болгон шинэчлэх (шалгах нийлбэрийг шалгах, зөрүүг шалгах, гарал үүслийг бүртгэх,
  худалдагчийн толийг унага).
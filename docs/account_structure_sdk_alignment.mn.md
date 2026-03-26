---
lang: mn
direction: ltr
source: docs/account_structure_sdk_alignment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 164bd373091ae3280f9f90fcfd915a90088b0c79b8f3759ffd2548edb64d0a90
source_last_modified: "2026-01-28T17:11:30.632934+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

SDK & Codec эзэмшигчдэд зориулсан # I105 танилцуулах тэмдэглэл

Багууд: Rust SDK, TypeScript/JavaScript SDK, Python SDK, Kotlin SDK, Codec хэрэгсэл

Контекст: `docs/account_structure.md` одоо хүргэлтийн I105 дансны ID-г тусгасан
хэрэгжилт. SDK-ийн үйлдэл болон тестийг каноник үзүүлэлттэй тохируулна уу.

Гол лавлагаа:
- Хаягийн кодлогч + толгой хэсгийн байршил — `docs/account_structure.md` §2
- Муруй бүртгэл — `docs/source/references/address_curve_registry.md`
- Норм v1 домэйн зохицуулалт — `docs/source/references/address_norm_v1.md`
- Бэхэлгээний векторууд — `fixtures/account/address_vectors.json`

Үйлдлийн зүйлс:
1. **Каноник гаралт:** `AccountId::to_string()`/Дэлгэц нь зөвхөн I105-г ялгаруулах ёстой.
   (`@domain` дагавар байхгүй). Каноник зургаан тал нь дибаг хийхэд зориулагдсан (`0x...`).
2. **Accepted inputs:** parsers MUST accept only canonical Katakana i105 account literals. Reject non-canonical Katakana i105 literals, canonical hex (`0x...`), any `@<domain>` suffix, alias literals, legacy `norito:<hex>`, and `uaid:` / `opaque:` parser forms.
3. **Resolvers:** canonical account parsing has no default-domain binding, scoped inference, or fallback resolver path. Use `ScopedAccountId` only on interfaces that explicitly require `<account>@<domain>`.
4. **I105 шалгах нийлбэр:** `I105PRE || prefix || payload` дээр Blake2b-512-г ашиглана уу
   эхний 2 байт. Шахсан цагаан толгойн суурь нь **105**.
5. **Муруй гарц:** SDK-г зөвхөн Ed25519-д тохируулна. Сонголтыг тодорхой зааж өгнө үү
   ML‑DSA/GOST/SM (Swift бүтээх тугууд; JS/Android `configureCurveSupport`). Хий
   Secp256k1-г Rust-аас гадуур анхдагчаар идэвхжүүлсэн гэж бүү бод.
6. **CAIP-10 байхгүй:** илгээсэн CAIP-10 зураглал хараахан байхгүй байна; бүү ил эсвэл
   CAIP‑10 хөрвүүлэлтээс хамаарна.

Кодек/тест шинэчлэгдсэний дараа баталгаажуулна уу; нээлттэй асуултуудыг хянах боломжтой
данс хаяглах RFC хэлхээнд.
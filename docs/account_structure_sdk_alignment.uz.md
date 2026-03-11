---
lang: uz
direction: ltr
source: docs/account_structure_sdk_alignment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 164bd373091ae3280f9f90fcfd915a90088b0c79b8f3759ffd2548edb64d0a90
source_last_modified: "2026-01-28T17:11:30.632934+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

SDK va kodek egalari uchun # I105 tarqatish eslatmasi

Jamoalar: Rust SDK, TypeScript/JavaScript SDK, Python SDK, Kotlin SDK, Codec asboblari

Kontekst: `docs/account_structure.md` endi yuk tashish I105 hisob identifikatorini aks ettiradi
amalga oshirish. Iltimos, SDK xatti-harakatlari va testlarini kanonik spetsifikatsiyaga moslang.

Asosiy havolalar:
- Manzil kodek + sarlavha tartibi - `docs/account_structure.md` §2
- Egri registri — `docs/source/references/address_curve_registry.md`
- Norm v1 domen bilan ishlash — `docs/source/references/address_norm_v1.md`
- Fikstur vektorlari — `fixtures/account/address_vectors.json`

Harakat elementlari:
1. **Kanonik chiqish:** `AccountId::to_string()`/Displey faqat I105 chiqarishi KERAK
   (`@domain` qo'shimchasi yo'q). Kanonik hex disk raskadrovka uchun mo'ljallangan (`0x...`).
2. **Accepted inputs:** parsers MUST accept only canonical I105 account literals. Reject i105-default `sora...`, canonical hex (`0x...`), any `@<domain>` suffix, alias literals, legacy `norito:<hex>`, and `uaid:` / `opaque:` parser forms.
3. **Resolvers:** canonical account parsing has no default-domain binding, scoped inference, or fallback resolver path. Use `ScopedAccountId` only on interfaces that explicitly require `<account>@<domain>`.
4. **I105 nazorat summasi:** `I105PRE || prefix || payload` orqali Blake2b-512 dan foydalaning, oling
   birinchi 2 bayt. Siqilgan alifbo bazasi **105**.
5. **Egri chiziq:** SDK standarti faqat Ed25519 uchun. Aniq ro'yxatdan o'tishni ta'minlang
   ML‑DSA/GOST/SM (Swift yaratish bayroqlari; JS/Android `configureCurveSupport`). Do
   secp256k1 sukut bo'yicha Rust tashqarisida yoqilgan deb hisoblamang.
6. **CAIP-10 yo‘q:** hali jo‘natilgan CAIP‑10 xaritasi mavjud emas; fosh qilmang yoki
   CAIP‑10 konversiyalariga bog'liq.

Iltimos, kodeklar/testlar yangilangandan keyin tasdiqlang; ochiq savollarni kuzatish mumkin
hisob-manzillash RFC oqimida.
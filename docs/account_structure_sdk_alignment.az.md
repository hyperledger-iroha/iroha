---
lang: az
direction: ltr
source: docs/account_structure_sdk_alignment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 164bd373091ae3280f9f90fcfd915a90088b0c79b8f3759ffd2548edb64d0a90
source_last_modified: "2026-01-28T17:11:30.632934+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

SDK və Codec Sahibləri üçün # I105 Yayım Qeydi

Komandalar: Rust SDK, TypeScript/JavaScript SDK, Python SDK, Kotlin SDK, Codec alətləri

Kontekst: `docs/account_structure.md` indi göndərmə I105 hesab ID-sini əks etdirir
həyata keçirilməsi. Lütfən, SDK davranışını və testlərini kanonik spesifikasiya ilə uyğunlaşdırın.

Əsas istinadlar:
- Ünvan kodek + başlıq tərtibatı — `docs/account_structure.md` §2
- Əyri reyestri — `docs/source/references/address_curve_registry.md`
- Norm v1 domen idarəsi — `docs/source/references/address_norm_v1.md`
- Armatur vektorları — `fixtures/account/address_vectors.json`

Fəaliyyət elementləri:
1. **Kanonik çıxış:** `AccountId::to_string()`/Ekran yalnız I105 yaymalıdır
   (`@domain` şəkilçisi yoxdur). Kanonik hex sazlama üçündir (`0x...`).
2. **Accepted inputs:** parsers MUST accept only canonical i105 account literals. Reject i105-default `sora...`, canonical hex (`0x...`), any `@<domain>` suffix, alias literals, legacy `norito:<hex>`, and `uaid:` / `opaque:` parser forms.
3. **Resolvers:** canonical account parsing has no default-domain binding, scoped inference, or fallback resolver path. Use `ScopedAccountId` only on interfaces that explicitly require `<account>@<domain>`.
4. **I105 yoxlama məbləği:** `I105PRE || prefix || payload` üzərində Blake2b-512 istifadə edin, götürün
   ilk 2 bayt. Sıxılmış əlifba bazası **105**-dir.
5. **Əyri keçid:** SDK-lar defolt olaraq yalnız Ed25519-dur. üçün açıq seçim təmin edin
   ML‑DSA/GOST/SM (Swift qurma bayraqları; JS/Android `configureCurveSupport`). Et
   Secp256k1-in Rust xaricində defolt olaraq aktiv olduğunu düşünməyin.
6. **CAIP-10 yoxdur:** göndərilmiş CAIP‑10 xəritəsi hələ yoxdur; ifşa etməyin və ya
   CAIP‑10 çevrilmələrindən asılıdır.

Kodeklər/testlər yeniləndikdən sonra təsdiq edin; açıq suallar izlənilə bilər
hesab ünvanlayan RFC mövzusunda.
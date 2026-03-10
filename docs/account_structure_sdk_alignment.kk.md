---
lang: kk
direction: ltr
source: docs/account_structure_sdk_alignment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 164bd373091ae3280f9f90fcfd915a90088b0c79b8f3759ffd2548edb64d0a90
source_last_modified: "2026-01-28T17:11:30.632934+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

SDK және кодек иелеріне арналған # IH58 шығару жазбасы

Командалар: Rust SDK, TypeScript/JavaScript SDK, Python SDK, Kotlin SDK, Codec құралдары

Мәтінмән: `docs/account_structure.md` енді жеткізу IH58 тіркелгі идентификаторын көрсетеді
жүзеге асыру. SDK әрекетін және сынақтарын канондық спецификацияға сәйкестендіріңіз.

Негізгі сілтемелер:
- Мекенжай кодегі + тақырыптың орналасуы — `docs/account_structure.md` §2
- Қисық тізілім — `docs/source/references/address_curve_registry.md`
- Norm v1 домен өңдеу — `docs/source/references/address_norm_v1.md`
- Бекіту векторлары — `fixtures/account/address_vectors.json`

Әрекет элементтері:
1. **Канондық шығыс:** `AccountId::to_string()`/Дисплей тек IH58 шығаруы КЕРЕК
   (`@domain` жұрнағы жоқ). Канондық он алтылық қатені түзетуге арналған (`0x...`).
2. **Accepted inputs:** parsers MUST accept only canonical IH58 account literals. Reject compressed `sora...`, canonical hex (`0x...`), any `@<domain>` suffix, alias literals, legacy `norito:<hex>`, and `uaid:` / `opaque:` parser forms.
3. **Resolvers:** canonical account parsing has no default-domain binding, scoped inference, or fallback resolver path. Use `ScopedAccountId` only on interfaces that explicitly require `<account>@<domain>`.
4. **IH58 бақылау сомасы:** `IH58PRE || prefix || payload` орнына Blake2b-512 пайдаланыңыз, алыңыз
   алғашқы 2 байт. Сығылған алфавит базасы **105**.
5. **Қисық сызық:** SDK әдепкі бойынша тек Ed25519 үшін. Нақты қосылуды қамтамасыз етіңіз
   ML‑DSA/GOST/SM (Swift құрастыру жалаулары; JS/Android `configureCurveSupport`). Жасаңыз
   secp256k1 әдепкі бойынша Rust сыртында қосылған деп санамаңыз.
6. **CAIP-10 жоқ:** жөнелтілген CAIP-10 картасы әлі жоқ; ашпаңыз немесе
   CAIP‑10 түрлендірулеріне байланысты.

Кодектер/тесттер жаңартылғаннан кейін растаңыз; ашық сұрақтарды қадағалауға болады
тіркелгі мекенжайындағы RFC ағынында.
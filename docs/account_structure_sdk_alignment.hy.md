---
lang: hy
direction: ltr
source: docs/account_structure_sdk_alignment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 164bd373091ae3280f9f90fcfd915a90088b0c79b8f3759ffd2548edb64d0a90
source_last_modified: "2026-01-28T17:11:30.632934+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IH58 Տարածման նշում SDK-ի և կոդեկների սեփականատերերի համար

Թիմեր՝ Rust SDK, TypeScript/JavaScript SDK, Python SDK, Kotlin SDK, Codec tooling

Համատեքստ. `docs/account_structure.md` այժմ արտացոլում է առաքման IH58 հաշվի ID-ն
իրականացումը։ Խնդրում ենք SDK-ի վարքագիծը և թեստերը համապատասխանեցնել կանոնական բնութագրին:

Հիմնական հղումներ.
- Հասցեի կոդեկ + վերնագրի դասավորություն — `docs/account_structure.md` §2
- կորի գրանցամատյան — `docs/source/references/address_curve_registry.md`
- Norm v1 տիրույթի կառավարում — `docs/source/references/address_norm_v1.md`
- Հարմարավետ վեկտորներ — `fixtures/account/address_vectors.json`

Գործողությունների տարրեր.
1. **Կանոնիկ ելք.** `AccountId::to_string()`/Էկրան պետք է թողարկի միայն IH58
   (ոչ `@domain` վերջածանց): Canonical hex-ը նախատեսված է վրիպազերծման համար (`0x...`):
2. **Accepted inputs:** parsers MUST accept only canonical IH58 account literals. Reject compressed `sora...`, canonical hex (`0x...`), any `@<domain>` suffix, alias literals, legacy `norito:<hex>`, and `uaid:` / `opaque:` parser forms.
3. **Resolvers:** canonical account parsing has no default-domain binding, scoped inference, or fallback resolver path. Use `ScopedAccountId` only on interfaces that explicitly require `<account>@<domain>`.
4. **IH58 checksum:** օգտագործեք Blake2b-512 `IH58PRE || prefix || payload`-ի վրա, վերցրեք
   առաջին 2 բայթ. Սեղմված այբուբենի հիմքը **105** է:
5. **Curve gating.** SDK-ների լռելյայն է միայն Ed25519-ը: Տրամադրեք բացահայտ միացում
   ML‑DSA/GOST/SM (Swift build դրոշներ; JS/Android `configureCurveSupport`): Արեք
   մի ենթադրեք, որ secp256k1-ը լռելյայն միացված է Rust-ից դուրս:
6. **Չկա CAIP-10:** առաքված CAIP-10 քարտեզագրում դեռ չկա; մի մերկացնել կամ
   կախված է CAIP-10 փոխակերպումներից:

Խնդրում ենք հաստատել, երբ կոդեկները/թեստերը թարմացվեն; բաց հարցերին կարելի է հետևել
հաշվի հասցեագրող RFC թեմայում:
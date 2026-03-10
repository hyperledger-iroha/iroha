---
lang: ba
direction: ltr
source: docs/account_structure_sdk_alignment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 164bd373091ae3280f9f90fcfd915a90088b0c79b8f3759ffd2548edb64d0a90
source_last_modified: "2026-01-28T17:11:30.632934+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# IH58 SDK өсөн ролл-аут иҫкәрмә & Кодексы хужалары

Командалар: Туст СДК, ТипСкрипт/JavaScript SDK, Python SDK, Котлин СДК, Кодк инструменталь

Контекст: I18NI000000000X хәҙер IH58 иҫәбе ID ташыуҙы сағылдыра.
тормошҡа ашырыу. Зинһар, SDK тәртибе һәм тестар менән канон спецификацияһы тура килтерергә.

Төп һылтанмалар:
- Адрес кодек + баш макеты — `docs/account_structure.md` §2.
- Ҡыйыулыҡ реестры — `docs/source/references/address_curve_registry.md`
- Норма v1 домен менән эш итеү — I18NI000000003X
- Фикстура векторҙары — `fixtures/account/address_vectors.json`

Эш пункттары:
1. **Каноник сығыш:** I18NI000000005X/Display IH58 ғына Эй
   (юҡ I18NI000000006X суффиксы). Канон hex өсөн отладка (`0x...`).
2. **Accepted inputs:** parsers MUST accept only canonical IH58 account literals. Reject compressed `sora...`, canonical hex (`0x...`), any `@<domain>` suffix, alias literals, legacy `norito:<hex>`, and `uaid:` / `opaque:` parser forms.
3. **Resolvers:** canonical account parsing has no default-domain binding, scoped inference, or fallback resolver path. Use `ScopedAccountId` only on interfaces that explicitly require `<account>@<domain>`.
4. **IH58 тикшерелгән сумма:** ҡулланыу Blak2b-512 өҫтөндә I18NI000000015X, алыу
   тәүге 2 байт. Ҡыҫылған алфавит базаһы **105**.
5. **Ҡырмыҫҡа ҡапҡаһы:** SDKs ғәҙәттәгесә Ed25519-тик. Асыҡтан-асыҡ опт-ин тәьмин итеү өсөн .
   ML-DSA/GOST/SM (Свифт төҙөү флагтары; JS/Android I18NI0000016X). Итергә
   secp256k1 тип фаразламай, Rust-тан ситтә ғәҙәттәгесә эшләй.
6. **КАИП-10:** әлегә ебәрелгән CAIP‐10 картаһы юҡ; фашламағыҙ йәки
   CAIP‐10 конверсияһына бәйле.

Зинһар, раҫлау бер тапҡыр кодек/тестар яңыртыла; асыҡ һорауҙарҙы күҙәтеп була
иҫәп-хисап менән идара итеү RFC еп.
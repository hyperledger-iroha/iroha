# Памятка по внедрению I105 для владельцев SDK и кодеков

Команды: SDK Rust, SDK TypeScript/JavaScript, SDK Python, SDK Kotlin, инструменты кодеков

Контекст: `docs/account_structure.md` теперь отражает поставляемую реализацию AccountId I105.
Приведите поведение и тесты SDK в соответствие с канонической спецификацией.

Ключевые ссылки:
- Кодек адреса + разметка заголовка — `docs/account_structure.md` §2
- Реестр кривых — `docs/source/references/address_curve_registry.md`
- Обработка доменов Norm v1 — `docs/source/references/address_norm_v1.md`
- Векторы фикстур — `fixtures/account/address_vectors.json`

Задачи:
1. **Канонический вывод:** `AccountId::to_string()`/Display ДОЛЖЕН выдавать только I105
   (без суффикса `@domain`). Канонический hex — только для отладки (`0x...`).
2. **Accepted inputs:** parsers MUST accept only canonical i105 account literals. Reject i105-default `sora...`, canonical hex (`0x...`), any `@<domain>` suffix, alias literals, legacy `norito:<hex>`, and `uaid:` / `opaque:` parser forms.
3. **Resolvers:** canonical account parsing has no default-domain binding, scoped inference, or fallback resolver path. Use `ScopedAccountId` only on interfaces that explicitly require `<account>@<domain>`.
4. **Контрольная сумма I105:** используйте Blake2b‑512 над `I105PRE || prefix || payload`,
   берите первые 2 байта. Основание сжатого алфавита — **105**.
5. **Ограничение кривых:** по умолчанию SDK — только Ed25519. Предоставьте явный opt‑in
   для ML‑DSA/GOST/SM (флаги сборки Swift; `configureCurveSupport` в JS/Android).
   Не предполагайте включённый secp256k1 по умолчанию вне Rust.
6. **Без CAIP‑10:** готового маппинга CAIP‑10 ещё нет; не раскрывайте и не
   завязывайтесь на преобразования CAIP‑10.

Пожалуйста, подтвердите после обновления кодеков/тестов; вопросы можно вести
в ветке RFC по адресации аккаунтов.

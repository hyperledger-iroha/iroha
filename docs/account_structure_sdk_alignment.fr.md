# Note de déploiement I105 pour responsables SDK & codecs

Équipes : SDK Rust, SDK TypeScript/JavaScript, SDK Python, SDK Kotlin, outillage de codecs

Contexte : `docs/account_structure.md` reflète désormais l’implémentation I105 de l’AccountId
livrée. Alignez le comportement et les tests des SDK sur la spécification canonique.

Références clés :
- Codec d’adresse + disposition de l’en‑tête — `docs/account_structure.md` §2
- Registre des courbes — `docs/source/references/address_curve_registry.md`
- Gestion des domaines Norm v1 — `docs/source/references/address_norm_v1.md`
- Vecteurs de fixtures — `fixtures/account/address_vectors.json`

Actions :
1. **Sortie canonique :** `AccountId::to_string()`/Display DOIT émettre uniquement I105
   (sans suffixe `@domain`). Le hex canonique est réservé au débogage (`0x...`).
2. **Accepted inputs:** parsers MUST accept only canonical i105 account literals. Reject i105-default `sora...`, canonical hex (`0x...`), any `@<domain>` suffix, alias literals, legacy `norito:<hex>`, and `uaid:` / `opaque:` parser forms.
3. **Resolvers:** canonical account parsing has no default-domain binding, scoped inference, or fallback resolver path. Use `ScopedAccountId` only on interfaces that explicitly require `<account>@<domain>`.
4. **Checksum I105 :** utiliser Blake2b‑512 sur `I105PRE || prefix || payload`,
   prendre les 2 premiers octets. La base de l’alphabet compressé est **105**.
5. **Garde des courbes :** les SDK sont Ed25519‑only par défaut. Fournir un opt‑in
   explicite pour ML‑DSA/GOST/SM (flags de build Swift ; `configureCurveSupport`
   en JS/Android). Ne pas supposer secp256k1 activé par défaut hors Rust.
6. **Pas de CAIP‑10 :** aucun mapping CAIP‑10 n’est livré pour l’instant ; ne pas
   exposer ni dépendre de conversions CAIP‑10.

Merci de confirmer une fois les codecs/tests mis à jour ; les questions ouvertes
peuvent être suivies dans le fil RFC sur l’adressage des comptes.

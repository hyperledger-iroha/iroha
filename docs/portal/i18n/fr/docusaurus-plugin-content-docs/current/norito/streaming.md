---
lang: fr
direction: ltr
source: docs/portal/docs/norito/streaming.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Norito Streaming

Norito Streaming definit le format on-wire, les frames de controle et le codec de reference utilises pour les flux media en direct via Torii et SoraNet. La specification canonique se trouve dans `norito_streaming.md` a la racine du workspace ; cette page en distille les elements dont les operateurs et les auteurs de SDK ont besoin, avec les points de configuration.

## Format on-wire et plan de controle

- **Manifests et frames.** `ManifestV1` et `PrivacyRoute*` decrivent la chronologie des segments, les descripteurs de chunks et les indices de route. Les frames de controle (`KeyUpdate`, `ContentKeyUpdate` et le feedback de cadence) vivent a cote du manifest afin que les viewers puissent valider les commitments avant de decoder.
- **Codec de base.** `BaselineEncoder`/`BaselineDecoder` imposent des ids de chunk monotones, l'arithmetique des timestamps et la verification des commitments. Les hotes doivent appeler `EncodedSegment::verify_manifest` avant de servir des viewers ou des relays.
- **Bits de feature.** La negociation de capacites annonce `streaming.feature_bits` (par defaut `0b11` = feedback baseline + privacy route provider) afin que relays et clients rejettent les peers incompatibles de facon deterministe.

## Cles, suites et cadence

- **Exigences d'identite.** Les frames de controle streaming sont toujours signees avec Ed25519. Des cles dediees peuvent etre fournies via `streaming.identity_public_key`/`streaming.identity_private_key` ; sinon l'identite du nud est reutilisee.
- **Suites HPKE.** `KeyUpdate` selectionne la suite commune la plus basse ; la suite #1 est obligatoire (`AuthPsk`, `Kyber768`, `HKDF-SHA3-256`, `ChaCha20-Poly1305`), avec un chemin de mise a niveau optionnel vers `Kyber1024`. La selection de la suite est stockee sur la session et validee a chaque update.
- **Rotation.** Les publishers emettent un `KeyUpdate` signe toutes les 64 MiB ou 5 minutes. `key_counter` doit augmenter strictement ; une regression est une erreur critique. `ContentKeyUpdate` distribue la Group Content Key tournante, enveloppee sous la suite HPKE negociee, et borne le dechiffrement des segments par ID + fenetre de validite.
- **Snapshots.** `StreamingSession::snapshot_state` et `restore_from_snapshot` persistent `{session_id, key_counter, suite, sts_root, cadence state}` sous `streaming.session_store_dir` (par defaut `./storage/streaming`). Les cles de transport sont re-derivees lors de la restauration afin que les crashs ne revelent pas de secrets de session.

## Configuration runtime

- **Materiau de cle.** Fournissez des cles dediees via `streaming.identity_public_key`/`streaming.identity_private_key` (multihash Ed25519) et du materiel Kyber optionnel via `streaming.kyber_public_key`/`streaming.kyber_secret_key`. Les quatre doivent etre presentes lors d'un override ; `streaming.kyber_suite` accepte `mlkem512|mlkem768|mlkem1024` (aliases `kyber512/768/1024`, par defaut `mlkem768`).
- **Routes SoraNet.** `streaming.soranet.*` controle le transport anonyme : `exit_multiaddr` (par defaut `/dns/torii/udp/9443/quic`), `padding_budget_ms` (par defaut 25 ms), `access_kind` (`authenticated` vs `read-only`), `channel_salt` optionnel, et `provision_spool_dir` (par defaut `./storage/streaming/soranet_routes`).
- **Gate de synchronisation.** `streaming.sync` active le controle de derive pour les flux audiovisuels : `enabled`, `observe_only`, `ewma_threshold_ms` et `hard_cap_ms` gouvernent quand des segments sont rejetes pour derive temporelle.

## Validation et fixtures

- Les definitions canoniques de types et les helpers se trouvent dans `crates/iroha_crypto/src/streaming.rs`.
- La couverture d'integration exerce le handshake HPKE, la distribution de content-key et le cycle de vie des snapshots (`crates/iroha_crypto/tests/streaming_handshake.rs`). Executez `cargo test -p iroha_crypto streaming_handshake` pour verifier la surface streaming localement.
- Pour une analyse approfondie du layout, de la gestion d'erreurs et des futures evolutions, lisez `norito_streaming.md` a la racine du depot.

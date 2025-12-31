<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/portal/docs/norito/streaming.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 10ba7b91d73d6723c4b66491951c3257c48557273ab5424d81119e01c8f2c6e3
source_last_modified: "2025-12-09T06:48:00.858874+00:00"
translation_last_reviewed: 2025-12-30
---

# Norito Streaming

Norito Streaming définit le format on-wire, les frames de contrôle et le codec de référence utilisés pour les flux média en direct via Torii et SoraNet. La spécification canonique se trouve dans `norito_streaming.md` à la racine du workspace ; cette page en distille les éléments dont les opérateurs et les auteurs de SDK ont besoin, avec les points de configuration.

## Format on-wire et plan de contrôle

- **Manifests et frames.** `ManifestV1` et `PrivacyRoute*` décrivent la chronologie des segments, les descripteurs de chunks et les indices de route. Les frames de contrôle (`KeyUpdate`, `ContentKeyUpdate` et le feedback de cadence) vivent à côté du manifest afin que les viewers puissent valider les commitments avant de décoder.
- **Codec de base.** `BaselineEncoder`/`BaselineDecoder` imposent des ids de chunk monotones, l'arithmétique des timestamps et la vérification des commitments. Les hôtes doivent appeler `EncodedSegment::verify_manifest` avant de servir des viewers ou des relays.
- **Bits de feature.** La négociation de capacités annonce `streaming.feature_bits` (par défaut `0b11` = feedback baseline + privacy route provider) afin que relays et clients rejettent les peers incompatibles de façon déterministe.

## Clés, suites et cadence

- **Exigences d'identité.** Les frames de contrôle streaming sont toujours signées avec Ed25519. Des clés dédiées peuvent être fournies via `streaming.identity_public_key`/`streaming.identity_private_key` ; sinon l'identité du nœud est réutilisée.
- **Suites HPKE.** `KeyUpdate` sélectionne la suite commune la plus basse ; la suite #1 est obligatoire (`AuthPsk`, `Kyber768`, `HKDF-SHA3-256`, `ChaCha20-Poly1305`), avec un chemin de mise à niveau optionnel vers `Kyber1024`. La sélection de la suite est stockée sur la session et validée à chaque update.
- **Rotation.** Les publishers émettent un `KeyUpdate` signé toutes les 64 MiB ou 5 minutes. `key_counter` doit augmenter strictement ; une régression est une erreur critique. `ContentKeyUpdate` distribue la Group Content Key tournante, enveloppée sous la suite HPKE négociée, et borne le déchiffrement des segments par ID + fenêtre de validité.
- **Snapshots.** `StreamingSession::snapshot_state` et `restore_from_snapshot` persistent `{session_id, key_counter, suite, sts_root, cadence state}` sous `streaming.session_store_dir` (par défaut `./storage/streaming`). Les clés de transport sont re-dérivées lors de la restauration afin que les crashs ne révèlent pas de secrets de session.

## Configuration runtime

- **Matériau de clé.** Fournissez des clés dédiées via `streaming.identity_public_key`/`streaming.identity_private_key` (multihash Ed25519) et du matériel Kyber optionnel via `streaming.kyber_public_key`/`streaming.kyber_secret_key`. Les quatre doivent être présentes lors d'un override ; `streaming.kyber_suite` accepte `mlkem512|mlkem768|mlkem1024` (aliases `kyber512/768/1024`, par défaut `mlkem768`).
- **Garde-fous du codec.** CABAC reste désactivé à moins que le build ne l'active ; rANS packagé nécessite `ENABLE_RANS_BUNDLES=1`. Enforce via `streaming.codec.{entropy_mode,bundle_width,bundle_accel}` et l'option `streaming.codec.rans_tables_path` lorsque vous fournissez des tables personnalisées. Le `bundle_width` packagé doit être entre 2 et 3 (inclus) ; la largeur 1 est legacy-only.
- **Routes SoraNet.** `streaming.soranet.*` contrôle le transport anonyme : `exit_multiaddr` (par défaut `/dns/torii/udp/9443/quic`), `padding_budget_ms` (par défaut 25 ms), `access_kind` (`authenticated` vs `read-only`), `channel_salt` optionnel, et `provision_spool_dir` (par défaut `./storage/streaming/soranet_routes`).
- **Gate de synchronisation.** `streaming.sync` active le contrôle de dérive pour les flux audiovisuels : `enabled`, `observe_only`, `ewma_threshold_ms` et `hard_cap_ms` gouvernent quand des segments sont rejetés pour dérive temporelle.

## Validation et fixtures

- Les définitions canoniques de types et les helpers se trouvent dans `crates/iroha_crypto/src/streaming.rs`.
- La couverture d'intégration exerce le handshake HPKE, la distribution de content-key et le cycle de vie des snapshots (`crates/iroha_crypto/tests/streaming_handshake.rs`). Exécutez `cargo test -p iroha_crypto streaming_handshake` pour vérifier la surface streaming localement.
- Pour une analyse approfondie du layout, de la gestion d'erreurs et des futures évolutions, lisez `norito_streaming.md` à la racine du dépôt.

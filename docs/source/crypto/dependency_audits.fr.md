---
lang: fr
direction: ltr
source: docs/source/crypto/dependency_audits.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 04e4cf26ed0ce9f9782be8aae9d16425a7a87fdbd1986cbcbca68a27ba0a3afe
source_last_modified: "2026-01-03T18:07:57.038859+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Audits de dépendance cryptographique

## Streebog (caisse `streebog`)

- **Version dans l'arborescence :** `0.11.0-rc.2` vendu sous `vendor/streebog` (utilisé lorsque la fonctionnalité `gost` est activée).
- **Consommateur :** `crates/iroha_crypto::signature::gost` (HMAC-Streebog DRBG + hachage de message).
- **Statut :** Candidat à la libération uniquement. Aucune caisse non RC n'offre actuellement la surface API requise,
  nous reflétons donc la caisse dans l'arborescence pour des raisons d'audit pendant que nous suivons en amont une version finale.
- ** Réviser les points de contrôle : **
  - Sortie de hachage vérifiée par rapport à la suite Wycheproof et aux appareils TC26 via
    `cargo test -p iroha_crypto --features gost` (voir `crates/iroha_crypto/tests/gost_wycheproof.rs`).
  -`cargo bench -p iroha_crypto --bench gost_sign --features gost`
    exerce Ed25519/Secp256k1 à côté de chaque courbe TC26 avec la dépendance actuelle.
  -`cargo run -p iroha_crypto --bin gost_perf_check --features gost`
    compare les mesures les plus récentes aux médianes enregistrées (utilisez `--summary-only` dans CI, ajoutez
    `--write-baseline crates/iroha_crypto/benches/gost_perf_baseline.json` lors du rebaselining).
  - `scripts/gost_bench.sh` enveloppe le banc + contrôle du flux ; transmettez `--write-baseline` pour mettre à jour le JSON.
    Voir `docs/source/crypto/gost_performance.md` pour le flux de travail de bout en bout.
- **Atténuations :** `streebog` n'est invoqué que via des wrappers déterministes qui remettent à zéro les clés ;
  le signataire couvre les occasionnels avec l'entropie du système d'exploitation pour éviter une défaillance catastrophique du RNG.
- **Actions suivantes :** Suivez la version streebog `0.11.x` de RustCrypto ; une fois le tag posé, traitez le
  mise à niveau en tant qu'augmentation de dépendance standard (vérifier la somme de contrôle, examiner la différence, enregistrer la provenance et
  déposez le miroir vendu).
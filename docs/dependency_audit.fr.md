---
lang: fr
direction: ltr
source: docs/dependency_audit.md
status: complete
translator: manual
source_hash: 9746f44dbe6c09433ead16647429ad48bba54ecf9c3271e71fad6cb91a212d65
source_last_modified: "2025-11-02T04:40:28.811390+00:00"
translation_last_reviewed: 2025-11-14
---

<!-- Traduction française de docs/dependency_audit.md (Dependency Audit Summary) -->

//! Synthèse d’Audit des Dépendances

Date : 2025‑09‑01

Périmètre : revue à l’échelle du workspace de tous les crates déclarés dans les
Cargo.toml et résolus dans Cargo.lock. Audit réalisé avec `cargo-audit` contre
la base RustSec, complété par une revue manuelle de la légitimité des crates et
des choix de crates “principaux” pour les algorithmes.

Outils/commandes exécutés :
- `cargo tree -d --workspace --locked --offline` – inspection des versions
  dupliquées.
- `cargo audit` – scan de Cargo.lock pour les vulnérabilités connues et les
  crates yanked.

Avis de sécurité trouvés (0 vulnérabilités restantes ; 2 avertissements) :
- crossbeam-channel — RUSTSEC‑2025‑0024  
  - Corrigé : mise à jour en `0.5.15` dans `crates/ivm/Cargo.toml`.

- codec pprof obsolète — RUSTSEC‑2024‑0437  
  - Corrigé : bascule de `pprof` vers `prost-codec` dans
    `crates/iroha_torii/Cargo.toml`.

- ring — RUSTSEC‑2025‑0009  
  - Corrigé : mise à jour de la stack QUIC/TLS (`quinn 0.11`, `rustls 0.23`,
    `tokio-rustls 0.26`) et mise à jour de la stack WS vers
    `tungstenite/tokio-tungstenite 0.24`. Lock forcé de `ring` en `0.17.12`
    avec `cargo update -p ring --precise 0.17.12`.

Avis restants : aucun. Avertissements restants : `backoff` (non maintenu),
`derivative` (non maintenu).

Évaluation de la légitimité et des crates “principaux” (points saillants) :
- Hashing : `sha2` (RustCrypto), `blake2` (RustCrypto), `tiny-keccak`
  (très utilisé) — choix canoniques.
- AEAD/symétrique : `aes-gcm`, `chacha20poly1305`, traits `aead` (RustCrypto)
  — canoniques.
- Signatures/ECC : `ed25519-dalek`, `x25519-dalek` (projet dalek), `k256`
  (RustCrypto), `secp256k1` (bindings libsecp) — tous légitimes ; il est
  recommandé de converger vers une seule stack secp256k1 (`k256` pur Rust ou
  `secp256k1` via libsecp) pour réduire la surface.
- BLS12‑381/ZK : `blstrs`, famille `halo2_*` — largement utilisés dans les
  écosystèmes ZK en production ; légitimes.
- PQ : `pqcrypto-dilithium`, `pqcrypto-traits` — crates de référence
  légitimes.
- TLS : `rustls`, `tokio-rustls`, `hyper-rustls` — stack TLS moderne et
  canonique en Rust.
- Noise : `snow` — implémentation canonique.
- Sérialisation : `parity-scale-codec` est le codec canonique pour SCALE.
  Serde a été retiré des dépendances de production à l’échelle du workspace ;
  les derives et writers Norito couvrent tous les chemins d’exécution. Toute
  référence résiduelle à Serde se trouve dans la documentation historique, les
  scripts de garde ou les allowlists de tests.
- FFI/libs : `libsodium-sys-stable`, `openssl` — légitimes ; pour les chemins
  de production, privilégier Rustls à OpenSSL (ce que fait déjà le code actuel).
- pprof 0.13.0 (crates.io) — correctif upstream fusionné ; utiliser la release
  officielle avec `prost-codec` + frame‑pointer pour éviter le codec obsolète.

Recommandations :
- Traiter les avertissements :
  - Envisager de remplacer `backoff` par `retry`/`futures-retry` ou par un
    helper local de backoff exponentiel.
  - Remplacer les derives `derivative` par des implémentations manuelles ou
    par `derive_more` lorsque c’est pertinent.
- Moyen terme : converger vers `k256` ou `secp256k1` partout où c’est possible
  afin de réduire les implémentations dupliquées (ne garder les deux que si
  c’est réellement nécessaire).
- Moyen terme : revoir la provenance de `poseidon-primitives 0.2.0` pour les
  usages ZK ; si possible, se rapprocher d’une implémentation Poseidon native
  Arkworks/Halo2 pour minimiser les écosystèmes parallèles.

Notes :
- `cargo tree -d` met en évidence des versions majeures dupliquées attendues
  (`bitflags` 1/2, plusieurs `ring`) ; ce n’est pas en soi un risque de
  sécurité, mais cela augmente la surface de build.
- Aucun crate de type typosquat n’a été observé ; tous les noms et sources
  correspondent à des crates bien connus de l’écosystème ou à des membres
  internes du workspace.
- Expérimental : ajout de la feature `iroha_crypto`
  `bls-backend-blstrs` pour commencer à migrer BLS vers un backend uniquement
  `blstrs` (supprime la dépendance à arkworks lorsqu’elle est activée). La
  valeur par défaut reste `w3f-bls` pour éviter tout changement de
  comportement/encodage. Plan d’alignement :
  - Normaliser la sérialisation de clé secrète sur la sortie canonique de
    32 octets en little‑endian, comprise par `w3f-bls` et `blstrs`
    (`Scalar::to_bytes_le`), et supprimer le helper mixte d’endianess
    historique.
  - Exposer un wrapper explicite pour la compression de clé publique en
    réutilisant `blstrs::G1Affine::to_compressed` et en ajoutant un contrôle
    de cohérence avec l’encodage w3f afin de garantir des bytes identiques
    “sur le fil”.
  - Ajouter des fixtures de round‑trip dans
    `crates/iroha_crypto/tests/bls_backend_compat.rs` qui dérivent des clés
    une seule fois et comparent les backends sur `SecretKey`, `PublicKey` et
    l’agrégation de signatures.
  - Protéger le nouveau backend derrière la feature `bls-backend-blstrs` dans
    la CI, tout en gardant les tests de cohérence activés pour le backend
    par défaut, de sorte que les régressions soient détectées avant tout
    changement de backend.

Suivi (travaux proposés) :
- Conserver les garde‑fous Serde dans la CI
  (`scripts/check_no_direct_serde.sh`, `scripts/deny_serde_json.sh`) afin
  d’empêcher l’introduction de nouveaux usages de Serde dans les chemins de
  production.

Tests réalisés pour cet audit :
- 

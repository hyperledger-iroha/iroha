---
lang: fr
direction: ltr
source: docs/source/crypto/sm_rustcrypto_spike.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1f133d9489c4bcfae2212e6c5dc098f39c3dea3e5cd42855ba76e8c9b73b4d03
source_last_modified: "2026-01-03T18:07:57.103009+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Notes pour le pic d'intégration de RustCrypto SM.

# Notes de pointe de RustCrypto SM

## Objectif
Vérifiez que l'introduction des caisses `sm2`, `sm3` et `sm4` de RustCrypto (plus `rfc6979`, `ccm`, `gcm`) en tant que dépendances facultatives se compile proprement dans le `iroha_crypto` et donne des temps de construction acceptables avant de câbler l'indicateur de fonctionnalité dans l'espace de travail plus large.

## Carte de dépendance proposée

| Caisse | Version suggérée | Caractéristiques | Remarques |
|-------|---------|----------|-------|
| `sm2` | `0.13` (RustCrypto/signatures) | `std` | Dépend de `elliptic-curve` ; vérifiez que MSRV correspond à l'espace de travail. |
| `sm3` | `0.5.0-rc.1` (RustCrypto/hachages) | par défaut | L'API est parallèle à `sha2` et s'intègre aux traits `digest` existants. |
| `sm4` | `0.5.1` (RustCrypto/chiffrements par blocs) | par défaut | Fonctionne avec les traits de chiffrement ; Les wrappers AEAD reportés à un pic ultérieur. |
| `rfc6979` | `0.4` | par défaut | Réutilisation pour la dérivation déterministe de cas occasionnel. |

*Les versions reflètent les versions actuelles de 2024 à 2012 ; confirmer avec `cargo search` avant l'atterrissage.*

## Modifications du manifeste (projet)

```toml
[features]
sm = ["dep:sm2", "dep:sm3", "dep:sm4", "dep:rfc6979"]

[dependencies]
sm2 = { version = "0.13", optional = true, default-features = false, features = ["std"] }
sm3 = { version = "0.5.0-rc.1", optional = true }
sm4 = { version = "0.5.1", optional = true }
rfc6979 = { version = "0.4", optional = true, default-features = false }
```

Suivi : épinglez `elliptic-curve` pour correspondre aux versions déjà présentes dans `iroha_crypto` (actuellement `0.13.8`).

## Liste de contrôle des pics
- [x] Ajoutez des dépendances et des fonctionnalités facultatives à `crates/iroha_crypto/Cargo.toml`.
-[x] Créez le module `signature::sm` derrière `cfg(feature = "sm")` avec des structures d'espace réservé pour confirmer le câblage.
- [x] Exécutez `cargo check -p iroha_crypto --features sm` pour confirmer la compilation ; enregistrer le temps de construction et le nombre de nouvelles dépendances (`cargo tree --features sm`).
- [x] Confirmez la posture standard uniquement avec `cargo check -p iroha_crypto --features sm --locked` ; Les versions `no_std` ne sont plus prises en charge.
- [x] Fichier de résultats (timings, delta de l'arborescence des dépendances) dans `docs/source/crypto/sm_program.md`.

## Observations à capturer
- Temps de compilation supplémentaire par rapport à la ligne de base.
- Impact de taille binaire (si mesurable) avec `cargo builtinsize`.
- Tout conflit MSRV ou fonctionnalité (par exemple, avec les versions mineures `elliptic-curve`).
- Avertissements émis (code dangereux, gating const-fn) pouvant nécessiter des correctifs en amont.

## Éléments en attente
- Attendez l'approbation de Crypto WG avant de gonfler le graphique de dépendance de l'espace de travail.
- Confirmez s'il faut vendre des caisses pour examen ou s'appuyer sur crates.io (des miroirs peuvent être nécessaires).
- Coordonner l'actualisation `Cargo.lock` selon `sm_lock_refresh_plan.md` avant de marquer la liste de contrôle comme terminée.
- Utilisez `scripts/sm_lock_refresh.sh` une fois l'approbation accordée pour régénérer le fichier de verrouillage et l'arborescence des dépendances.

## 2025-01-19 Journal des pics
- Ajout de dépendances facultatives (`sm2 0.13`, `sm3 0.5.0-rc.1`, `sm4 0.5.1`, `rfc6979 0.4`) et d'un indicateur de fonctionnalité `sm` dans `iroha_crypto`.
- Module `signature::sm` stubbé pour exercer les API de hachage/chiffrement par bloc pendant la compilation.
- `cargo check -p iroha_crypto --features sm --locked` résout désormais le graphique de dépendance mais abandonne avec l'exigence de mise à jour `Cargo.lock` ; la politique du référentiel interdit les modifications du fichier de verrouillage, de sorte que l'exécution de la compilation reste en attente jusqu'à ce que nous coordonnions une actualisation du verrouillage autorisée.## 2026-02-12 Journal des pics
- Résolution du bloqueur de fichier de verrouillage précédent (les dépendances sont déjà capturées), donc `cargo check -p iroha_crypto --features sm --locked` réussit (construction à froid 7,9 s sur Mac de développement ; réexécution incrémentielle 0,23 s).
- `cargo check -p iroha_crypto --no-default-features --features "std sm" --locked` passe en 1.0s, confirmant que la fonctionnalité facultative est compilée dans les configurations `std` uniquement (il ne reste aucun chemin `no_std`).
- Le delta de dépendance avec la fonctionnalité `sm` activée introduit 11 caisses : `base64ct`, `ghash`, `opaque-debug`, `pem-rfc7468`, `pkcs8`, `polyval`, `primeorder`, `sm2`, `sm3`, `sm4` et `sm4-gcm`. (`rfc6979` faisait déjà partie du graphique de référence.)
- Les avertissements de build persistent pour les assistants de politique NEON inutilisés ; laissez tel quel jusqu'à ce que le runtime de lissage de mesure réactive ces chemins de code.
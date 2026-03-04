---
lang: fr
direction: ltr
source: docs/source/crypto/attachments/sm_openssl_provenance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 95a34657b6064f925995a7e9f20145d14fda681f4af1f182418b9f624047e576
source_last_modified: "2026-01-03T18:07:57.073612+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Instantané de provenance SM OpenSSL/Tongsuo
% généré : 2026-01-30

# Résumé de l'environnement

- `pkg-config --modversion openssl` : `3.6.0`
- `openssl version -a` : rapporte `LibreSSL 3.3.6` (boîte à outils TLS fournie par le système sur macOS).
- `cargo tree -p iroha_crypto --features "sm sm-ffi-openssl"` : voir `sm_iroha_crypto_tree.txt` pour la pile exacte de dépendances Rust (`openssl` crate v0.10.74, `openssl-sys` v0.9.x, sources OpenSSL 3.x vendues disponibles via `openssl-src` crate ; fonctionnalité `vendored` activé dans `crates/iroha_crypto/Cargo.toml` pour les versions préliminaires déterministes).

# Remarques

- Liens d'environnement de développement local vers les en-têtes/bibliothèques LibreSSL ; les versions d'aperçu de production doivent utiliser OpenSSL >= 3.0.0 ou Tongsuo 8.x. Remplacez la boîte à outils système ou définissez `OPENSSL_DIR`/`PKG_CONFIG_PATH` lors de la génération du groupe d'artefacts final.
- Régénérez cet instantané dans l'environnement de construction de la version pour capturer le hachage exact de l'archive tar OpenSSL/Tongsuo (`openssl version -v`, `openssl version -b`, `openssl version -f`) et joignez le script de construction/somme de contrôle reproductible. Pour les versions vendues, enregistrez la version/validation de la caisse `openssl-src` utilisée par Cargo (visible dans `target/debug/build/openssl-sys-*/output`).
- Les hôtes Apple Silicon nécessitent `RUSTFLAGS=-Aunsafe-code` lors de l'exécution du faisceau de fumée OpenSSL afin que les stubs d'accélération AArch64 SM3/SM4 se compilent (les éléments intrinsèques ne sont pas disponibles sur macOS). Le script `scripts/sm_openssl_smoke.sh` exporte cet indicateur avant d'appeler `cargo` pour maintenir la cohérence des exécutions CI et locales.
- Attacher la provenance de la source en amont (par exemple, `openssl-src-<ver>.tar.gz` SHA256) une fois le pipeline d'emballage épinglé ; utilisez le même hachage dans les artefacts CI.
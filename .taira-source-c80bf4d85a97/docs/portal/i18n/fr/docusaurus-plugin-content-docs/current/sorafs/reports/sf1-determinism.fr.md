---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : SoraFS SF1 Déterminisme Essai à sec
résumé : Checklist et digests attendus pour valider le profil chunker canonique `sorafs.sf1@1.0.0`.
---

# SoraFS SF1 Déterminisme Essai à sec

Ce rapport capture le dry-run de base pour le profil chunker canonique
`sorafs.sf1@1.0.0`. Tooling WG doit relancer la liste de contrôle ci-dessous lors de la
validation des rafraîchissements de luminaires ou de nouveaux pipelines de consommateurs.
Consignez le résultat de chaque commande dans le tableau afin de maintenir une
trace vérifiable.

## Liste de contrôle

| Étape | Commande | Résultat attendu | Remarques |
|------|---------|--------|-------|
| 1 | `cargo test -p sorafs_chunker` | Tous les tests passent ; le test de parité `vectors` réussit. | Confirmez que les appareils canoniques sont compilés et correspondant à l'implémentation Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | Le script sort en 0 ; rapporte les digests de manifeste ci-dessous. | Vérifiez que les luminaires se régénèrent proprement et que les signatures restent attachées. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | L'entrée pour `sorafs.sf1@1.0.0` correspond au descripteur du registre (`profile_id=1`). | S'assurer que les métadonnées du registre restent synchronisées. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | La régénération réussie sans `--allow-unsigned` ; les fichiers de manifeste et de signature restent inchangés. | Fournit une preuve de déterminisme pour les limites de chunk et les manifestes. |
| 5 | `node scripts/check_sf1_vectors.mjs` | Ne rapporte aucune différence entre les appareils TypeScript et le JSON Rust. | Aide en option ; garantir la parité cross-runtime (script maintenu par Tooling WG). |

## Digests attendus

- Résumé de fragments (SHA3-256) : `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json` : `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json` : `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts` : `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go` : `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs` : `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## Journal de signature

| Dates | Ingénieur | Résultat du checklist | Remarques |
|------|----------|-------------|-------|
| 2026-02-12 | Outillage (LLM) | ✅Réussi | Luminaires régénérées via `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f`, produisant la liste canonique + alias et un manifeste digest frais `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. Vérifié avec `cargo test -p sorafs_chunker` et un `ci/check_sorafs_fixtures.sh` propre (luminaires étapes pour la vérification). Étape 5 en attente jusqu'à l'arrivée du helper de parité Node. |
| 2026-02-20 | Outillage de stockage CI | ✅Réussi | Enveloppe du Parlement (`fixtures/sorafs_chunker/manifest_signatures.json`) récupérée via `ci/check_sorafs_fixtures.sh` ; le script a régénéré les luminaires, confirmé le manifest digest `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`, et relancé le harnais Rust (les étapes Go/Node s'exécutent quand disponibles) sans diff. |

Tooling WG doit ajouter une ligne datée après l'exécution de la checklist. Si une
étape échec, ouvrir un problème lié ici et inclure les détails de remédiation
avant d'approuver de nouveaux luminaires ou profils.
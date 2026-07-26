---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : SoraFS SF1 Déterminisme Essai à sec
résumé : Liste de contrôle et résumés attendus pour valider le profil chunker canonique `sorafs.sf1@1.0.0`.
---

# SoraFS SF1 Déterminisme Essai à sec

Ce rapport capture la base de fonctionnement à sec pour le profil chunker canonique
`sorafs.sf1@1.0.0`. Le groupe de travail sur l'outillage doit ré-exécuter la liste de contrôle à ce moment-là
valider les mises à jour de luminaires ou de nouveaux pipelines de consommateurs. Registre
résultat de chaque commande sur la table pour maintenir une piste vérifiable.

## Liste de contrôle

| Paso | Commandant | Résultats attendus | Notes |
|------|---------|--------|-------|
| 1 | `cargo test -p sorafs_chunker` | Tous les tests ont été passés ; Le test de parité `vectors` est terminé. | Confirmez que les appareils canoniques sont compilés et coïncident avec l'implémentation de Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | La vente de script avec 0 ; reporta los digestes de manifest de abajo. | Vérifiez que les luminaires sont régénérés limpiamente et que les entreprises peuvent être ajoutées de manière permanente. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | L'entrée de `sorafs.sf1@1.0.0` coïncide avec le descripteur du registre (`profile_id=1`). | Assurez-vous que les métadonnées du registre sont synchronisées. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | La régénération a lieu sans `--allow-unsigned` ; les archives du manifeste et de l'entreprise ne changent pas. | Prouvez un test de déterminisme pour les limites de fragments et de manifestes. |
| 5 | `node scripts/check_sf1_vectors.mjs` | Il n'y a pas de différence entre les appareils TypeScript et Rust JSON. | Aide facultative ; assurer la parité entre les environnements d'exécution (script géré par Tooling WG). |

## Digests espérados

- Résumé de fragments (SHA3-256) : `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json` : `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json` : `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts` : `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go` : `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs` : `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## Journal de signature

| Fécha | Ingénieur | Résultat de la liste de contrôle | Notes |
|------|----------|---------------|-------|
| 2026-02-12 | Outillage (LLM) | D'accord | Luminaires régénérés via `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102...1f`, production de poignées canonico + listes d'alias et une fresque de résumé manifeste `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. Vérifié avec `cargo test -p sorafs_chunker` et une corrida propre de `ci/check_sorafs_fixtures.sh` (luminaires préparés pour la vérification). Paso 5 pendiente hasta que llegue el helper de paridad Node. |
| 2026-02-20 | Outillage de stockage CI | D'accord | Enveloppe du Parlement (`fixtures/sorafs_chunker/manifest_signatures.json`) obtenue via `ci/check_sorafs_fixtures.sh` ; le script régénére les appareils, confirme le résumé du manifeste `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21` et réexécute le harnais Rust (pasos Go/Node se exécute quand il est disponible) sans différences. |

Tooling WG doit ajouter une fila fechada après avoir exécuté la liste de contrôle. Si
Quelqu'un n'a pas réussi, ouvrez un problème enlazado ici et incluez des détails de remédiation
avant de tester de nouveaux luminaires ou profils.
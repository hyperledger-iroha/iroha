---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : SoraFS SF1 Déterminisme Essai à sec
résumé : profil de chunker canonique `sorafs.sf1@1.0.0` et validation de la liste de contrôle et des résumés attendus.
---

# SoraFS SF1 Déterminisme Essai à sec

Profil canonique `sorafs.sf1@1.0.0` chunker et essai à sec de base
capturer کرتی ہے۔ Tooling WG et les luminaires actualisent les pipelines grand public
validation et liste de contrôle pour les tâches ہر کمانڈ کا نتیجہ
ٹیبل میں ریکارڈ کریں تاکہ piste vérifiable برقرار رہے۔

## Liste de contrôle

| Étape | Commande | Résultat attendu | Remarques |
|------|---------|--------|-------|
| 1 | `cargo test -p sorafs_chunker` | تمام tests پاس ہوں؛ Test de parité `vectors` ici | Il y a une compilation de luminaires canoniques et une implémentation de Rust et une correspondance entre les deux. |
| 2 | `ci/check_sorafs_fixtures.sh` | اسکرپٹ 0 کے ساتھ exit کرے؛ نیچے والے manifeste digests رپورٹ کرے۔ | vérifier les appareils et régénérer les signatures attachées |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | `sorafs.sf1@1.0.0` est un descripteur de registre d'entrée (`profile_id=1`) et correspond à un descripteur de registre d'entrée (`profile_id=1`). | یقینی بناتا ہے کہ synchronisation des métadonnées du registre رہے۔ |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | la régénération `--allow-unsigned` est réussie manifeste اور signature فائلیں inchangé رہیں۔ | les limites des morceaux se manifestent comme une preuve de déterminisme |
| 5 | `node scripts/check_sf1_vectors.mjs` | Appareils TypeScript et Rust JSON et rapport diff ici | assistant facultatif؛ les temps d'exécution sont parités de parité (le script Tooling WG maintient la parité) |

## Résumés attendus

- Résumé de fragments (SHA3-256) : `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json` : `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json` : `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts` : `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go` : `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs` : `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## Journal de signature

| Dates | Ingénieur | Résultat de la liste de contrôle | Remarques |
|------|----------|--------|-------|
| 2026-02-12 | Outillage (LLM) | ✅ Réussi | `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f` les appareils régénèrent le handle canonique + les listes d'alias et le résumé du manifeste `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55` `cargo test -p sorafs_chunker` اور صاف `ci/check_sorafs_fixtures.sh` exécute et vérifie la fonction (vérification des appareils par étape). Étape 5 : En attente et Assistant de parité de nœud actuellement |
| 2026-02-20 | Outillage de stockage CI | ✅ Réussi | Enveloppe du Parlement (`fixtures/sorafs_chunker/manifest_signatures.json`) `ci/check_sorafs_fixtures.sh` کے ذریعے حاصل ہوا؛ Les appareils de ce type régénèrent le résumé du manifeste `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21` confirment le harnais Rust et les étapes Go/Node (étapes Go/Node pour chaque étape) بغیر diffs کے۔ |

Liste de contrôle du Tooling WG pour les tâches à accomplir اگر
Il s'agit d'un échec et d'un problème lié, ainsi que d'une remédiation.
Les calendriers des matchs et les profils approuvés sont approuvés.
<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# SoraFS SF1 Determinism Dry-Run

Ce rapport capture le dry-run de base pour le profil chunker canonique
`sorafs.sf1@1.0.0`. Tooling WG doit relancer le checklist ci-dessous lors de la
validation des refreshes de fixtures ou de nouveaux pipelines de consommateurs.
Consignez le résultat de chaque commande dans le tableau afin de maintenir une
trace auditable.

## Checklist

| Étape | Commande | Résultat attendu | Notes |
|------|---------|------------------|-------|
| 1 | `cargo test -p sorafs_chunker` | Tous les tests passent ; le test de parité `vectors` réussit. | Confirme que les fixtures canoniques compilent et correspondent à l'implémentation Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | Le script sort en 0 ; rapporte les digests de manifest ci-dessous. | Vérifie que les fixtures se régénèrent proprement et que les signatures restent attachées. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | L'entrée pour `sorafs.sf1@1.0.0` correspond au descriptor du registry (`profile_id=1`). | S'assure que la metadata du registry reste synchronisée. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | La régénération réussit sans `--allow-unsigned` ; les fichiers de manifest et de signature restent inchangés. | Fournit une preuve de déterminisme pour les limites de chunk et les manifests. |
| 5 | `node scripts/check_sf1_vectors.mjs` | Ne rapporte aucune diff entre les fixtures TypeScript et le JSON Rust. | Helper optionnel ; garantir la parité cross-runtime (script maintenu par Tooling WG). |

## Digests attendus

- Chunk digest (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## Journal de sign-off

| Date | Engineer | Résultat du checklist | Notes |
|------|----------|-----------------------|-------|
| 2026-02-12 | Tooling (LLM) | ✅ Réussi | Fixtures régénérées via `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f`, produisant la liste canonique + aliases et un manifest digest frais `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. Vérifié avec `cargo test -p sorafs_chunker` et un `ci/check_sorafs_fixtures.sh` propre (fixtures stagées pour la vérification). Étape 5 en attente jusqu'à l'arrivée du helper de parité Node. |
| 2026-02-20 | Storage Tooling CI | ✅ Réussi | Parliament envelope (`fixtures/sorafs_chunker/manifest_signatures.json`) récupéré via `ci/check_sorafs_fixtures.sh` ; le script a régénéré les fixtures, confirmé le manifest digest `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`, et relancé le harness Rust (les étapes Go/Node s'exécutent quand disponibles) sans diff. |

Tooling WG doit ajouter une ligne datée après l'exécution du checklist. Si une
étape échoue, ouvrir un issue lié ici et inclure les détails de remédiation
avant d'approuver de nouveaux fixtures ou profils.

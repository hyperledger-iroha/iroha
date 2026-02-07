---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : SoraFS SF1 Déterminisme Essai à sec
résumé : Liste de contrôle et résumés attendus pour valider le chunker de profil canonique `sorafs.sf1@1.0.0`.
---

# SoraFS SF1 Déterminisme Essai à sec

Ce rapport capture la base de fonctionnement à sec pour le chunker de profil canonique
`sorafs.sf1@1.0.0`. Le groupe de travail sur l'outillage doit réexécuter la liste de contrôle et la valider
rafraîchit les luminaires ou les nouveaux pipelines de consommateurs. Registre des résultats de chaque année
comando na tabela para manter un trail auditavel.

## Liste de contrôle

| Passer | Commandant | Résultats attendus | Notes |
|------|---------|--------|-------|
| 1 | `cargo test -p sorafs_chunker` | Tous les tests ont été réussis ; Le test de parité `vectors` vous a réussi. | Confirmez que les appareils canoniques sont compilés et correspondent à l'implémentation de Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | O script sai com 0; reporta os digests de manifest abaixo. | Vérifiez que les luminaires sont régénérés et que les assinaturas permanecem anexadas. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | L'entrée `sorafs.sf1@1.0.0` correspond au descripteur de registre (`profile_id=1`). | Garantir que les métadonnées du registre soient synchronisées en permanence. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | Une régénération ocorre sem `--allow-unsigned` ; arquivos de manifest e assinatura nao mudam. | Il faut prouver le déterminisme pour les limites des blocs et des manifestes. |
| 5 | `node scripts/check_sf1_vectors.mjs` | Rapport nenhum diff entre les appareils TypeScript et Rust JSON. | Aide facultative ; garantir la parité entre les runtimes (script exécuté par Tooling WG). |

## Digests espérados

- Résumé de fragments (SHA3-256) : `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json` : `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json` : `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts` : `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go` : `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs` : `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## Journal de signature

| Données | Ingénieur | Résultat de la liste de contrôle | Notes |
|------|----------|--------------|-------|
| 2026-02-12 | Outillage (LLM) | D'accord | Les appareils sont régénérés via `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102...1f`, je produis une poignée canonico + des listes d'alias et un résumé de manifeste nouveau `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. Vérifié avec `cargo test -p sorafs_chunker` et un `ci/check_sorafs_fixtures.sh` limpo (luminaires préparés pour la vérification). Passo 5 pendente ate o helper de paridade Node Chegar. |
| 2026-02-20 | Outillage de stockage CI | D'accord | Enveloppe du Parlement (`fixtures/sorafs_chunker/manifest_signatures.json`) obtenue via `ci/check_sorafs_fixtures.sh` ; o script régénérant les appareils, confirmant le résumé du manifeste `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`, et réexécutant le harnais Rust (passer Go/Node exécuté quand il est disponible) avec les différences. |

Tooling WG a développé une ligne de données pour exécuter la liste de contrôle. Voir l'algum
passo falhar, abra um issue ligado aqui e inclua detalhes de remédiation antes
de aprovar novos luminaires ou perfis.
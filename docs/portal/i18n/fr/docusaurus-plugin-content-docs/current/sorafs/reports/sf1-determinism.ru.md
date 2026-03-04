---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/reports/sf1-determinism.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : SoraFS SF1 Déterminisme Essai à sec
résumé : Résumés et résumés pour la vérification du profil de chunker canonique `sorafs.sf1@1.0.0`.
---

# SoraFS SF1 Déterminisme Essai à sec

Ceci permet de fixer le fonctionnement à sec de base pour le chunker de profil canonique
`sorafs.sf1@1.0.0`. Tooling WG doit mettre en place une liste de clients pour la fourniture
обновлений luminaires или новых pipelines de consommation. Записывайте результат каждой
команды в таблицу, чтобы сохранить piste auditable.

## Checklist

| Шаг | Commande | Résultat final | Première |
|------|---------|--------|-------|
| 1 | `cargo test -p sorafs_chunker` | Все тесты проходят; Test de parité `vectors` effectué. | Il s'avère que les luminaires canoniques sont compilés et intégrés à la réalisation de Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | Le script est 0 ; сообщает digère les manifestations ниже. | Vérifiez que les luminaires se régénèrent et peuvent être installés. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | Téléchargez pour `sorafs.sf1@1.0.0` le descripteur de registre (`profile_id=1`). | Garantissez que le registre de métadonnées est synchronisé. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | La régénération est effectuée sans `--allow-unsigned` ; файлы manifeste и signature не меняются. | Il y a une documentation relative à la détermination des limites des morceaux et des manifestes. |
| 5 | `node scripts/check_sf1_vectors.mjs` | Les différences peuvent être trouvées entre les appareils TypeScript et Rust JSON. | Aide spécialisée ; Utilisez le runtime de chaque partie (le script fournit Tooling WG). |

## Résumés détaillés

- Résumé de fragments (SHA3-256) : `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json` : `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json` : `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts` : `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go` : `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs` : `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## Journal de signature

| Données | Ingénieur | Résultat de la recherche | Première |
|------|----------|----------|-------|
| 2026-02-12 | Outillage (LLM) | ✅ Réussi | Les luminaires sont régénérés à partir de `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f`, avec une poignée canonique + des alias spécifiques et un nouveau résumé de manifeste `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. Testé `cargo test -p sorafs_chunker` et boîtier `ci/check_sorafs_fixtures.sh` (luminaires adaptés aux tests). Partie 5 sur la mise à disposition de l'assistant de parité de nœud. |
| 2026-02-20 | Outillage de stockage CI | ✅ Réussi | Enveloppe du Parlement (`fixtures/sorafs_chunker/manifest_signatures.json`) получен через `ci/check_sorafs_fixtures.sh` ; Le script a régénéré les luminaires, a mis à jour le résumé du manifeste `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21` et a publié le harnais Rust (le Go/Node est disponible pour les différences) sans différences. |

Tooling WG doit effectuer les travaux de planification après la vérification. Esli
какой-LIBO шаг падает, заведите issue со ссылкой здесь и добавьте детали
l'assainissement des nouveaux luminaires ou profils.
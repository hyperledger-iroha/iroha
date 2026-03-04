---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/reports/sf1-determinism.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
כותרת: SoraFS SF1 Determinism Dry-Run
תקציר: Checklist et digests attendus pour valider le profil chunker canonique `sorafs.sf1@1.0.0`.
---

# SoraFS SF1 דטרמיניזם יבש הפעלה

Ce rapport capture le dry run de base pour le profil chunker canonique
`sorafs.sf1@1.0.0`. Tooling WG doit relancer le checklist ci-dessous lors de la
validation des refreshes de fixtures ou de nouveaux pipelines de consommateurs.
Consignez le résultat de chaque commande dans le tableau afin de maintenir une
מעקב ניתן לביקורת.

## רשימת תיוג

| Étape | קומנדה | Résultat attendu | הערות |
|------|--------|----------------|-------|
| 1 | `cargo test -p sorafs_chunker` | Tous les tests נוסעים; le test de parité `vectors` réussit. | אשר que les fixtures canoniques compilent and correspondent à l'implementation Rust. |
| 2 | `ci/check_sorafs_fixtures.sh` | Le script sort en 0; rapporte les digests de manifest ci-dessous. | Verifie que les fixtures se régénèrent proprement et que les signatures restent attachées. |
| 3 | `cargo run -p sorafs_manifest --bin sorafs_manifest_chunk_store -- --list-profiles` | L'entrée pour `sorafs.sf1@1.0.0` מתאים לתיאור הרישום (`profile_id=1`). | S'assure que la metadata du registry reste Syncée. |
| 4 | `cargo run --locked -p sorafs_chunker --bin export_vectors` | La régénération réussit sans `--allow-unsigned` ; les fichiers de manifest et de signature restent inchangés. | Fournit une preuve de déterminisme pour les limites de chunk et les manifests. |
| 5 | `node scripts/check_sf1_vectors.mjs` | אין קשר בין אבזור מסוג TypeScript ו-JSON Rust. | אופציונלי עוזר; משך זמן ריצה צולב (תחזוקת סקריפט לפי Tooling WG). |

## מעכל attendus

- תקציר נתחים (SHA3-256): `13fa919c67e55a2e95a13ff8b0c6b40b2e51d6ef505568990f3bc7754e6cc482`
- `manifest_blake3.json`: `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`
- `sf1_profile_v1.json`: `23a14fe4bf06a44bc2cc84ad0f287659f62a3ff99e4147e9e7730988d9eb01be`
- `sf1_profile_v1.ts`: `2bc35d45a9a1e539c4b0e3571817dc57d5a938e954882537379d7abba7b751a1`
- `sf1_profile_v1.go`: `dcca46978768cca5fdbc5174a35036d5e168cc5e584bba33056b76f316590666`
- `sf1_profile_v1.rs`: `181f0595284dcbb862db997d1c18564832c157f9e1eaf804f0bf88c846f73d65`

## יומן חתימה

| תאריך | מהנדס | רשימת התוצאות | הערות |
|------|--------|-----------------------|-------|
| 2026-02-12 | כלי עבודה (LLM) | ✅ Réussi | מתקנים régénérées דרך `cargo run --locked -p sorafs_chunker --bin export_vectors -- --signing-key=000102…1f`, produisant la list canonique + aliases et un manifest digest frais `2084f98010fd59b630fede19fa85d448e066694f77fa41a03c62b867eb5a9e55`. Vérifié avec `cargo test -p sorafs_chunker` et un `ci/check_sorafs_fixtures.sh` propre (משחקי הופעות pour la vérification). Étape 5 en attente jusqu'à l'arrivée du helper de parité Node. |
| 2026-02-20 | Storage Tooling CI | ✅ Réussi | מעטפת הפרלמנט (`fixtures/sorafs_chunker/manifest_signatures.json`) récupéré via `ci/check_sorafs_fixtures.sh` ; התסריט והתקשורת הרגילה, אישור ה-manife digest `101ec2aa55346e0ec57b2da6c7b9a9adde85ef13cbbf56c349bceafad7917c21`, ו-relance the harness Rust (les étapes Go/Node s'exécutent quand disponibles) sans diff. |

Tooling WG doit ajouter une ligne datée après l'exécution du checklist. סי אונה
étape échoue, ouvrir un issue lié ici et inclure les details de remediation
אבנט ד'אפרובר דה נובו או פרופילים.
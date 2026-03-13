---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/node-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
מזהה: תוכנית צמתים
כותרת: Plan d'implémentation du nœud SoraFS
sidebar_label: Plan d'implémentation du nœud
תיאור: רובואי SF-3 בעבודת יד הניתנת לפעולה עם ג'אלונים, טפסים ובדיקות כיסוי.
---

:::הערה מקור קנוניק
Cette page reflète `docs/source/sorafs/sorafs_node_plan.md`. Gardez les deux copies Syncées jusqu'au retrait de la documentation Sphinx historique.
:::

SF-3 חיה לביצוע ארגז ראשי `sorafs-node` כדי להפוך את תהליך Iroha/Torii לארבעה מלאי SoraFS. Utilisez ce plan avec le [guide de stockage du nœud](node-storage.md), la [politique d'admission des providers](provider-admission-policy.md) et la [feuille de route du marketplace de capacité de stockage](Norito) les liverqueser.

## Portée cible (Jalon M1)

1. **Intégration du chunk store.** Envelopper `sorafs_car::ChunkStore` מלווה ב-backend persistant qui stocke les bytes de chunk, les manifests and les arbres PoR dans le répertoire de données configuré.
2. **שער נקודות קצה.** Exposer des endpoints HTTP Norito pour la soumission de pin, le fetch de chunks, l'échantillonnage PoR et la télémétrie de stockage dans le processus Torii.
3. **Plomberie de configuration.** Ajouter une structure de config `SoraFsStorage` (flag d'activation, capacité, répertoires, limites de concurrence) reliée à `iroha_config`, `iroha_core`.
4. **מכסה/תקשורת.** אפליקציית מגבלות דיסק/מקבילית מוגדרת לאופרה ושמירה על רצונות של לחץ אחורי.
5. **Télémétrie.** Émettre des métriques/logs pour les succès de pin, la latence de fetch de chunks, l'utilisation de capacité et les résultats d'échantillonnage PoR.

## Decomposition du travail

### א. מבנה ארגז ומודולים

| טאצ'ה | אחראי(ים) | הערות |
|------|----------------|------|
| Créer `crates/sorafs_node` עם מודולים: `config`, `store`, `gateway`, `scheduler`, `telemetry`. | מחסן Équipe | Re-exporter les types réutilisables pour l'intégration Torii. |
| Implémenter `StorageConfig` mappé depuis `SoraFsStorage` (משתמש → בפועל → ברירות מחדל). | Équipe Storage / Config WG | Garantir que les couchs Norito/`iroha_config` restent déterministes. |
| Fournir une חזית `NodeHandle` que Torii לנצל סיכות/שליפות של soumettre. | מחסן Équipe | Encapsuler les internes de stockage et la plomberie async. |

### ב. חנות נתחים מתמשכת| טאצ'ה | אחראי(ים) | הערות |
|------|----------------|------|
| בנה את ה-backend disk enveloppant `sorafs_car::ChunkStore` avec un index de manifest sur disque (`sled`/`sqlite`). | מחסן Équipe | פריסה דטרמיניסטית: `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| Maintenir les métadonnées PoR (arbres 64 KiB/4 KiB) דרך `ChunkStore::sample_leaves`. | מחסן Équipe | תומך בשידור חוזר לאחר הנישואין; להיכשל מהר באשר לשחיתות. |
| Implémenter le replay d'intégrité au démarrage (rehash des manifests, purge des pins incomplets). | מחסן Équipe | Bloque le démarrage de Torii jusqu'à la fin du replay. |

### ג. שער נקודות קצה

| נקודת קצה | דוח | טאצ'ס |
|--------|--------------|-------|
| `POST /sorafs/pin` | קבל את `PinProposalV1`, תקף למניפסטים, עם קובץ זה, השב עם ה-CID du Manifest. | Valider le profil de chunker, appliquer les quotas, streamer les données דרך le chunk store. |
| `GET /sorafs/chunks/{cid}` + בקשת טווח | Sert les bytes de chunk avec les headers `Content-Chunker`; respecte la specification de capacité de range. | ניצול מתזמן + תקציבי זרימה (אפשרות לטווח SF-2d). |
| `POST /sorafs/por/sample` | Lance un échantillonnage PoR pour un manifest et renvoie un bundle de preuve. | Reutiliser l'échantillonnage du chunk store, répondre avec des payloads Norito JSON. |
| `GET /sorafs/telemetry` | קורות חיים: capacité, succès PoR, compteurs d'erreurs de fetch. | Fournir les données יוצקים לוחות מחוונים/מפעילים. |

La plomberie runtime relie les interactions PoR via `sorafs_node::por` : le tracker enregistre chaque `PorChallengeV1`, `PorProofV1` et `AuditVerdictV1` pour que les métriques I10è610X verdict reflève sans logique Torii spécifique.【crates/sorafs_node/src/scheduler.rs#L147】

הערות ליישום:

- Utiliser le stack Axum de Torii avec des payloads `norito::json`.
- Ajouter des schémas Norito pour les réponses (`PinResultV1`, `FetchErrorV1`, structures de télémétrie).

- ✅ `/v2/sorafs/por/ingestion/{manifest_digest_hex}` לחשוף את התחזוקה של ה-profondeur du backlog ainsi que l'époque/échéance la plus ancienne et les timestamps de succès/échec les plus récents par ספק, דרך `sorafs_node::NodeHandle::por_ingestion_status`, et `sorafs_node::NodeHandle::por_ingestion_status`, et I00000180X gauges `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` pour les לוחות מחוונים.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:18 83】【crates/iroha_torii/src/routing.rs:7244】【ארגזים/iroha_telemetry/src/metrics.rs:5390】

### ד. מתזמן ויישום מכסות| טאצ'ה | פרטים |
|------|--------|
| דיסק מכסה | Suivre les bytes sur disque; rejeter les nouveaux pins au-delà de `max_capacity_bytes`. Fournir des hooks d'éviction pour des politiques futures. |
| Concurrence de fetch | Sémaphore global (`max_parallel_fetches`) בתוספת תקציבים לפי ספק SF-2d. |
| קובץ דה פינים | Limiter les jobs d'ingestion en attente; חושף נקודות הקצה של חוק Norito pour la profondeur de file. |
| Cadence PoR | Worker de fond piloté par `por_sample_interval_secs`. |

### E. Télémétrie et loging

מטריקס (Prometheus):

- `sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (תוויות היסטוגרמה avec `result`)
- `torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
- `torii_sorafs_storage_fetch_bytes_per_sec`
- `torii_sorafs_storage_por_inflight`
- `torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

יומנים / מאפיינים :

- Télémétrie Norito structureee pour l'ingestion governance (`StorageTelemetryV1`).
- התראות quand l'utilisation > 90% או que la serie d'échecs PoR dépasse le seuil.

### F. אסטרטגיית בדיקות

1. **מבחן יחידות.** Persistance du chunk store, calculs de quata, invariants du scheduler (voir `crates/sorafs_node/src/scheduler.rs`).
2. **בדיקות אינטגרציה** (`crates/sorafs_node/tests`). סיכה ← אחזר אלר-retour, reprise after redédémarrage, rejet de quota, vérification de preuve d'échantillonnage PoR.
3. **בדיקות אינטגרציה Torii.** Exécuter Torii עם מלאי פעיל, מפעילים נקודות קצה HTTP דרך `assert_cmd`.
4. **כאוס במפת הדרכים.** Des drills futurs simulent l'épuisement du disque, l'IO lente, la suppression de providers.

## תלות

- Politique d'admission SF-2b — s'assurer que les nœuds vérifient les envelopes d'admission avant publication d'adverts.
- Marketplace de capacité SF-2c — rattacher la télémétrie aux déclarations de capacité.
- Extensions d'advert SF-2d - consommer la capacité de range + budgets de stream dès disponibilité.

## קריטריונים למיון

- `cargo run -p sorafs_node --example pin_fetch` fonctionne sur des fixtures locales.
- Torii לבנות עם `--features sorafs-storage` ועבור בדיקות אינטגרציה.
- תיעוד ([guide de stockage du nœud](node-storage.md)) mise à jour avec הגדרות ברירת המחדל + דוגמאות CLI ; לרונבוק מפעיל זמין.
- Télémétrie visible sur les dashboards de staging; התראות תצורות לשפוך רוויה de capacité et échecs PoR.

## תיעוד Livrables ופעולות

- Mettre à jour la [référence de stockage du nœud](node-storage.md) עם ברירות מחדל של תצורה, שימוש ב-CLI ופתרון בעיות.
- Garder le [runbook d'opérations du nœud](node-operations.md) aligné avec l'implémentation au fur et à mesure de l'évolution SF-3.
- Publier les références API pour les endpoints `/sorafs/*` dans le portail développeur et les relier au manifeste OpenAPI une fois les handlers Torii en place.
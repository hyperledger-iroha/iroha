---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/pin-registry-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: pin-registry-plan
כותרת: Plan d'implémentation du Pin Registry de SoraFS
sidebar_label: Plan du Pin Registry
תיאור: Plan d'implementation SF-4 couvrant la machine d'états du registry, la חזית Torii, le tooling et l'observabilité.
---

:::הערה מקור קנוניק
Cette page reflète `docs/source/sorafs/pin_registry_plan.md`. Gardez les deux copies Syncées tant que la documentation héritée reste active.
:::

# Plan d'implémentation du Pin Registry de SoraFS (SF-4)

SF-4 חיוור לקונטרה Pin Registry et les services d'appui qui stockent les
engagements de manifest, appliquent les politiques de pinning et exposent des
API à Torii, aux gateways et aux orchestratorens. Ce document étend le plan de
validation avec des tâches d'implémentation concrètes, couvrant la logique
על רשת, les services côté hôte, les fixtures et les exigences opérationnelles.

## Portée

1. **Machine d'états du registry** : רישומים Norito יוצקים מניפסטים, כינויים,
   chaînes de succession, époques de rétention et métadonnées de governance.
2. **יישום דו קונטרה**: פעולות CRUD déterministes pour le cycle de vie
   des pins (`ReplicationOrder`, `Precommit`, `Completion`, פינוי).
3. **Façade de service**: נקודות קצה gRPC/REST adossés au registry consommés par Torii
   ו-SDKs, עם עימוד ואישור.
4. **כלים ואביזרים**: עוזרי CLI, וקטורי בדיקה ותיעוד לשפוך
   מניפסטים, כינויים ומעטפות של סינכרון שלטון.
5. **Télémétrie et ops** : מדריכים, התראות ו-runbooks pour la santé du registry.

## דגם דגמי

### רישום עיקרי (Norito)

| מבנה | תיאור | Champs |
|--------|-------------|--------|
| `PinRecordV1` | Entrée canonique de manifest. | I18NIS `governance_envelope_hash`. |
| `AliasBindingV1` | מפה כינוי -> CID de manifest. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instruction pour que les providers pinent le manifest. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Accusé de réception du ספק. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | תמונת מצב של פוליטיקה דה שלטון. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Référence d'implementation : voir `crates/sorafs_manifest/src/pin_registry.rs` pour les
schémas Norito en Rust et les helpers de validation qui sotiennent ces registrements.
La validation reflète le tooling Manifest (בדיקת רישום chunker, שער מדיניות סיכות)
pour que le contrat, les facades Torii et la CLI partagent des invariants identiques.תכונה:
- Finaliser les schémas Norito ב-`crates/sorafs_manifest/src/pin_registry.rs`.
- קוד כללי (Rust + autres SDK) באמצעות פקודות מאקרו Norito.
- Mettre à jour la documentation (`sorafs_architecture_rfc.md`) une fois les schémas en place.

## Implementation du contrat

| טאצ'ה | בעלים | הערות |
|-------|----------------|-------|
| יישום ה-stockage du registry (מזחלת/sqlite/off-chain) או מודול חוזה חכם. | Core Infra / צוות חוזה חכם | Fournir un hashing déterministe, éviter le flottant. |
| נקודות כניסה: `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | אינפרא ליבה | S'appuyer sur `ManifestValidator` du plan de validation. Le binding d'alias passe Maintenant par `RegisterPinManifest` (DTO Torii exposé) tandis que `bind_alias` dédié reste prévu pour des mises à jour successives. |
| Transitions d'état: imposer la succession (מניפסט A -> B), les époques de rétention, l'unicité des aliases. | מועצת ממשל / Infra Core | L'unicité des aliases, les limites de rétention et les checks d'approbation/retrait des prédécesseurs vivent dans `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ; la détection de succession multi-hop et le bookkeeping de réplication restent ouverts. |
| Paramètres gouvernés : מטען `ManifestPolicyV1` depuis la config/l'état de governance ; permettre les mises à jour via événements de gouvernance. | מועצת ממשל | Fournir une CLI pour les mises à jour de politique. |
| Émission d'événements : émettre des événements Norito pour la télémétrie (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | צפייה | Définir le schéma d'événements + רישום. |

מבחנים:
- בדיקות unitaires pour chaque נקודת הכניסה (positif + rejet).
- Tests de propriétés pour la chaîne de succession (pas de cycles, époques monotones).
- Fuzz de validation en générant des manifests aléatoires (bornés).

## Façade de service (Intégration Torii/SDK)

| קומפוזיטור | טאצ'ה | בעלים |
|---------|------|--------|
| שירות Torii | Exposer `/v2/sorafs/pin` (שלח), `/v2/sorafs/pin/{cid}` (חיפוש), `/v2/sorafs/aliases` (רשימה/כריכה), `/v2/sorafs/replication` (הזמנות/קבלות). עימוד פורניר + סינון. | Networking TL / Core Infra |
| אישור | כלול la hauteur/hash du registry dans les réponses; ajouter une structure d'attestation Norito consommée par les SDKs. | אינפרא ליבה |
| CLI | Étendre `sorafs_manifest_stub` או une nouvelle CLI `sorafs_pin` avec `pin submit`, `alias bind`, `order issue`, `registry export`. | Tooling WG |
| SDK | Générer des bindings client (Rust/Go/TS) depuis le schéma Norito ; ajouter des tests d'integration. | צוותי SDK |

פעולות:
- Ajouter une couche de cache/ETag pour les נקודות הקצה GET.
- הגבלת תעריף / אישור קוהרנטים של פוליטיקה Torii.

## מתקנים & CI- מסמך מתקנים : `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` stocke des snapshots signnés de manifest/alias/order régénérés דרך `cargo run -p iroha_core --example gen_pin_snapshot`.
- Étape CI : `ci/check_sorafs_fixtures.sh` régénère le snapshot et échoue in cas de diff, gardant les fixtures CI alignés.
- Tests d'intégration (`crates/iroha_core/tests/pin_registry.rs`) couvrent le happy path plus le rejet d'alias dupliqué, les guards d'approbation/retention, les handles de chunker non concordants, la validation du compte de replicas et les échecs de garde de succession (pointeurs inconnus/pré-approuvés/retirés/auto-référencés); voir les cas `register_manifest_rejects_*` pour les details de couverture.
- Les tests unitaires couvrent Maintenant la validation d'alias, les guards de rétention et les checks de successeur dans `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ; la détection de succession multi-hop להשתתף ב-la machine d'états.
- JSON golden pour les événements utilisés par les pipelines d'observabilité.

## Télémétrie & observabilité

מטריקס (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- ספקי הטלפונים הקיימים (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) נשארו בטווח רחב של לוחות מחוונים מקצה לקצה.

יומנים:
- Flux d'événements Norito structurés pour les audits de governance (signés ?).

התראות:
- Ordres de réplication en attente dépassant le SLA.
- Expiration d'alias < seuil.
- Violations de rétention (התפוגה של אוונט לא רנובלי).

לוחות מחוונים:
- Le JSON Grafana `docs/source/grafana_sorafs_pin_registry.json` suit les totaux du cycle de vie des manifests, la couverture d'alias, la saturation du backlog, le ratio SLA, les overlays latence vs slack et les taux d'ordres manqués on- pour la revue.

## ספרי ריצה ותיעוד

- Mettre à jour `docs/source/sorafs/migration_ledger.md` pour inclure les mises à jour de statut du registry.
- מדריך מפעיל: `docs/source/sorafs/runbooks/pin_registry_ops.md` (déjà publié) מדדים, התרעה, ביצוע, זרימה ושטף חוזר.
- Guide de governance: décrire les paramètres de politique, le workflow d'approbation, la gestion des litiges.
- Pages de référence API pour chaque endpoint (מסמכים Docusaurus).

## תלות וסידור

1. Termins les tâches du plan de validation (Integration ManifestValidator).
2. Finaliser le schéma Norito + ברירות מחדל של פוליטיקה.
3. Implémenter le contrat + service, brancher la télémétrie.
4. מתקנים של Régénerer les, exécuter les suites d'integration.
5. Mettre à jour docs/runbooks et marquer les items du מפת הדרכים comme complets.

צ'ק פריט רשימת SF-4 דויט référencer ce plan lorsque le progrès est registré.
La facade REST livre désormais des endpoints של רישום עדויות:

- `GET /v2/sorafs/pin` et `GET /v2/sorafs/pin/{digest}` renvoient des manifests avec
  כריכות ד'alias, orders de réplication et un objet d'attestation dérivé du hash
  גוש du dernier.
- קטלוג `GET /v2/sorafs/aliases` ו-`GET /v2/sorafs/replication` חשוף
  d'alias actif et le backlog des ordres de réplication avec une pagetion cohérente
  et des filtres de statut.La CLI encapsule ces appels (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) pour permettre aux opérateurs d'automatiser les audits du
registry sans toucher aux APIs bas niveau.
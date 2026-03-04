---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan-registre-pin
titre : SoraFS Pin Registry نفاذی منصوبہ
sidebar_label : Registre des broches منصوبہ
description : SF-4 est un outil de registre et une machine à états, une façade Torii, un outillage et une observabilité.
---

:::note مستند ماخذ
یہ صفحہ `docs/source/sorafs/pin_registry_plan.md` کی عکاسی کرتا ہے۔ جب تک پرانی دستاویزات فعال ہیں دونوں نقول ہم آہنگ رکھیں۔
:::

# SoraFS Pin Registry نفاذی منصوبہ (SF-4)

Registre SF-4 Pin pour les engagements manifestes et les engagements manifestes
les politiques de broches sont des exemples de passerelles et d'orchestrateurs Torii pour les API et les API.
Il s'agit d'un plan de validation et de tâches de mise en œuvre ainsi que d'une logique en chaîne.
services côté hôte, installations, اور عملیاتی تقاضے شامل ہیں۔

## دائرہ کار

1. **Machine à états de registre** : enregistrements définis par Norito, manifestes, alias, chaînes de successeurs,
   époques de rétention et métadonnées de gouvernance.
2. ** کنٹریکٹ نفاذ** : cycle de vie des broches pour les opérations CRUD déterministes (`ReplicationOrder`,
   `Precommit`, `Completion`, expulsion).
3. **Façade extérieure** : points de terminaison gRPC/REST et registre pour le Torii et les kits SDK.
   جن میں pagination اور attestation شامل ہے۔
4. **outils et accessoires** : assistants CLI, vecteurs de test, et documentation, manifestes, alias et
   enveloppes de gouvernance ہم آہنگ رہیں۔
5. **Opérations de télémétrie** : le registre contient des métriques, des alertes et des runbooks.

## ڈیٹا ماڈل

### بنیادی ریکارڈز (Norito)

| Structure | وضاحت | فیلڈز |
|--------|-------|-------|
| `PinRecordV1` | entrée manifeste canonique. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | alias -> mappage CID manifeste. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | fournisseurs et broches manifestes. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | accusé de réception du fournisseur. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | instantané de la politique de gouvernance. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Référence d'implémentation : `crates/sorafs_manifest/src/pin_registry.rs` pour les schémas Rust Norito
اور aides à la validation موجود ہیں۔ Outils de manifeste de validation (recherche de registre de fragments et contrôle des politiques de broches)
Les façades Torii et les invariants CLI sont également disponibles.

Tâches :
- `crates/sorafs_manifest/src/pin_registry.rs` et schémas Norito pour les schémas de base
- Les macros Norito et le code génèrent des fichiers (Rust + SDK)
- les schémas sont disponibles dans les documents (`sorafs_architecture_rfc.md`) pour les documents

## کنٹریکٹ نفاذ| کام | مالک/مالکان | نوٹس |
|-----|-------------|------|
| stockage de registre (sled/sqlite/off-chain) avec module de contrat intelligent | Équipe Core Infra / Smart Contract | hachage déterministe à virgule flottante à virgule flottante |
| Points d'entrée : `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Infrastructure de base | plan de validation `ManifestValidator` استعمال کریں۔ Liaison d'alias `RegisterPinManifest` (Torii DTO) pour créer un lien vers `bind_alias`. لیے منصوبہ بند ہے۔ |
| Transitions d'état : succession (manifeste A -> B), époques de rétention et unicité d'alias. | Conseil de gouvernance / Core Infra | Unicité de l'alias, limites de conservation, et contrôles d'approbation/retraite du prédécesseur `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` میں ہیں؛ détection de succession multi-sauts et comptabilité de réplication |
| Paramètres régis : `ManifestPolicyV1` pour l'état de configuration/gouvernance et l'état de gouvernance. événements de gouvernance | Conseil de gouvernance | mises à jour des politiques en ligne avec la CLI |
| Émission d'événements : télémétrie et événements Norito (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`) جاری کریں۔ | Observabilité | schéma d'événement + journalisation |

Test :
- ہر point d'entrée کے لیے tests unitaires (positif + rejet).
- chaîne de succession et tests de propriétés (pas de cycles, époques monotones).
- manifestes aléatoires (limités) et validation fuzz.

## façade façade (Torii/SDK انضمام)

| جزو | کام | مالک/مالکان |
|------|-----|-------------|
| Torii Service | `/v1/sorafs/pin` (soumettre)، `/v1/sorafs/pin/{cid}` (recherche)، `/v1/sorafs/aliases` (liste/liaison)، `/v1/sorafs/replication` (commandes/reçus) pagination + filtrage | Mise en réseau TL / Core Infra |
| Attestation | réponses en hauteur/hachage du registre La structure d'attestation Norito est utilisée pour les SDK qui consomment des fichiers | Infrastructure de base |
| CLI | `sorafs_manifest_stub` est compatible avec `sorafs_pin` CLI pour `pin submit`, `alias bind`, `order issue`, `registry export` ہو۔ | GT Outillage |
| SDK | Le schéma Norito et les liaisons client (Rust/Go/TS) génèrent des erreurs tests d'intégration | Équipes SDK |

Opérations :
- OBTENIR les points de terminaison et la couche cache/ETag
- Politiques Torii pour limitation de débit / autorisation d'authentification

## Calendrier par CI- Répertoire des luminaires : `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` avec manifeste/alias/instantanés de commande signés et `cargo run -p iroha_core --example gen_pin_snapshot` pour régénérer le système.
- Étape CI : régénération de l'instantané `ci/check_sorafs_fixtures.sh` et diff et échec en cas d'échec des appareils CI alignés.
- Tests d'intégration (`crates/iroha_core/tests/pin_registry.rs`) chemin heureux comme rejet d'alias en double, gardes d'approbation/rétention d'alias, poignées de chunker incompatibles, validation du nombre de répliques, et échecs de garde de succession (inconnus/pré-approuvés/retraités/auto-pointeurs) et تفصیل کے لیے `register_manifest_rejects_*` cases دیکھیں۔
- Tests unitaires comme `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` pour la validation d'alias, les gardes de rétention et les contrôles de successeur pour les tests détection de succession multi-sauts comme machine à états
- Pipelines d'observabilité pour les événements JSON dorés

## Télémétrie et observabilité

Métriques (Prometheus) :
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Tableaux de bord de bout en bout du fournisseur de télémétrie (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) et portée de l'application

Journaux :
- audits de gouvernance کے لیے flux d'événements structurés Norito (signé ?).

Alertes :
- SLA a des commandes de réplication en attente.
- seuil d'expiration du pseudonyme سے کم.
- violations de conservation (renouvellement manifeste وقت سے پہلے نہ ہو).

Tableaux de bord :
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` Totaux du cycle de vie des manifestes, couverture des alias, saturation du backlog, ratio SLA, latence par rapport aux superpositions Slack, taux de commandes manquées et examen sur appel pour les clients.

## Runbooks et documentation

- `docs/source/sorafs/migration_ledger.md` pour les mises à jour de l'état du registre
- Guide de l'opérateur : métriques `docs/source/sorafs/runbooks/pin_registry_ops.md` (اب شائع شدہ), alertes, déploiement, sauvegarde et flux de récupération.
- Guide de gouvernance : paramètres politiques, workflow d'approbation, gestion des litiges.
- Les pages de référence de l'API du point de terminaison et de l'API (documents Docusaurus).

## Dépendances et séquençage

1. tâches du plan de validation مکمل کریں (intégration ManifestValidator).
2. Schéma Norito + paramètres par défaut de la politique
3. contrat + service نافذ کریں اور fil de télémétrie کریں۔
4. les luminaires régénèrent les suites d'intégration et les suites d'intégration
5. docs/runbooks pour les éléments de la feuille de route et les éléments de la feuille de route

Liste de contrôle SF-4
Façade REST et points de terminaison de liste attestés par exemple :

- `GET /v1/sorafs/pin` et `GET /v1/sorafs/pin/{digest}` manifestent des manifestations de ce genre.
  liaisons d'alias, ordres de réplication, ou hachage de bloc, ou objet d'attestation, etc.
- `GET /v1/sorafs/aliases` vers `GET /v1/sorafs/replication` vers le catalogue alias vers
  arriéré des ordres de réplication et pagination cohérente et filtres d'état

CLI appelle et enveloppe کرتی ہے (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) Opérateurs de gestion d'API et d'audits de registre pour les audits de registre
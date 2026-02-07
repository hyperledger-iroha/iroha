---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan-registre-pin
titre : Plan d'implémentation du Pin Registry de SoraFS
sidebar_label : Registre du Plan du Pin
description : Plan d'implémentation SF-4 comprenant la machine d'états du registre, la façade Torii, l'outillage et l'observabilité.
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/pin_registry_plan.md`. Gardez les deux copies synchronisées tant que la documentation héritée reste active.
:::

# Plan d'implémentation du Pin Registry de SoraFS (SF-4)

SF-4 livre le contrat Pin Registry et les services d'appui qui stockent les
engagements de manifester, appliquer les politiques d'épinglage et exposer des
API à Torii, aux passerelles et aux orchestrateurs. Ce document étend le plan de
validation avec des tâches d'implémentation concrètes, incluant la logique
en chaîne, les services côté hôte, les luminaires et les exigences opérationnelles.

## Portée

1. **Machine d'états du registre** : enregistrements Norito pour manifests, alias,
   chaînes de succession, époques de rétention et métadonnées de gouvernance.
2. **Implémentation du contrat** : opérations CRUD déterministes pour le cycle de vie
   des pins (`ReplicationOrder`, `Precommit`, `Completion`, expulsion).
3. **Façade de service** : endpoints gRPC/REST adossés au registre consommé par Torii
   et les SDK, avec pagination et attestation.
4. **Tooling et montages** : helpers CLI, vecteurs de test et documentation pour
   garder les manifestes, alias et enveloppes de gouvernance synchronisées.
5. **Télémétrie et ops** : métriques, alertes et runbooks pour la santé du registre.

## Modèle de données

### Enregistrements principaux (Norito)

| Structure | Descriptif | Champions |
|--------|-------------|--------|
| `PinRecordV1` | Entrée canonique du manifeste. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | Mappe alias -> CID du manifeste. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instruction pour que les fournisseurs pinent le manifeste. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Accusé de réception du fournisseur. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Aperçu de la politique de gouvernance. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Référence d'implémentation : voir `crates/sorafs_manifest/src/pin_registry.rs` pour les
Norito en Rust et les helpers de validation qui soutiennent ces schémas enregistrements.
La validation reflète le manifeste d'outillage (recherche du registre chunker, pin Policy Gating)
pour que le contrat, les façades Torii et la CLI partagent des invariants identiques.Tâches :
- Finaliser les schémas Norito dans `crates/sorafs_manifest/src/pin_registry.rs`.
- Générer le code (Rust + autres SDKs) via les macros Norito.
- Mettre à jour la documentation (`sorafs_architecture_rfc.md`) une fois les schémas en place.

## Implémentation du contrat

| Tâche | Propriétaire(s) | Remarques |
|-------|----------|-------|
| Implémenter le stockage du registre (sled/sqlite/off-chain) ou un module de contrat intelligent. | Équipe Core Infra / Smart Contract | Fournir un hachage déterministe, éviter le flottant. |
| Points d'entrée : `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Infrastructure de base | S'appuyer sur `ManifestValidator` du plan de validation. Le contraignant d'alias passe maintenant par `RegisterPinManifest` (DTO Torii exposé) tandis que `bind_alias` dédié reste prévu pour des mises à jour successives. |
| Transitions d'état : imposer la succession (manifeste A -> B), les époques de rétention, l'unicité des alias. | Conseil de gouvernance / Core Infra | L'unicité des alias, les limites de rétention et les contrôles d'approbation/retrait des précédents sont présents dans `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ; la détection de succession multi-sauts et la comptabilité de réplication restent ouvertes. |
| Paramètres gouvernés : charger `ManifestPolicyV1` depuis la config/l'état de gouvernance ; permettre les mises à jour via des événements de gouvernance. | Conseil de gouvernance | Fournir une CLI pour les mises à jour de politique. |
| Émission d'événements : émet des événements Norito pour la télémétrie (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Observabilité | définir le schéma d'événements + logging. |

Essais :
- Tests unitaires pour chaque point d'entrée (positif + rejet).
- Tests de propriétés pour la chaîne de succession (pas de cycles, époques monotones).
- Fuzz de validation en générant des manifestes aléatoires (bornés).

## Façade de service (Intégration Torii/SDK)

| Composant | Tâche | Propriétaire(s) |
|---------------|------|--------------|
| Service Torii | Exposer `/v1/sorafs/pin` (soumettre), `/v1/sorafs/pin/{cid}` (recherche), `/v1/sorafs/aliases` (liste/liaison), `/v1/sorafs/replication` (commandes/reçus). Fournir pagination + filtrage. | Mise en réseau TL / Core Infra |
| Attestation | Inclure la hauteur/hash du registre dans les réponses ; ajouter une structure d'attestation Norito consommée par les SDK. | Infrastructure de base |
| CLI | Étendre `sorafs_manifest_stub` ou une nouvelle CLI `sorafs_pin` avec `pin submit`, `alias bind`, `order issue`, `registry export`. | GT Outillage |
| SDK | Générer des clients de liaisons (Rust/Go/TS) depuis le schéma Norito ; ajouter des tests d'intégration. | Équipes SDK |

Opérations :
- Ajouter une couche de cache/ETag pour les endpoints GET.
- Fournir rate limitation / authentification cohérente avec les politiques Torii.

## Calendriers et CI- Dossier de luminaires : `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` stocke des snapshots signés de manifest/alias/order régénérés via `cargo run -p iroha_core --example gen_pin_snapshot`.
- Étape CI : `ci/check_sorafs_fixtures.sh` régénère le snapshot et échoue en cas de diff, en gardant les luminaires CI alignés.
- Les tests d'intégration (`crates/iroha_core/tests/pin_registry.rs`) couvrent le happy path plus le rejet d'alias dupliqué, les gardes d'approbation/rétention, les handles de chunker non concordants, la validation du compte de répliques et les échecs de garde de succession (pointeurs inconnus/pré-approuvés/retirés/auto-référencés) ; voir les cas `register_manifest_rejects_*` pour les détails de couverture.
- Les tests unitaires couvrent désormais la validation d'alias, les gardes de rétention et les contrôles de successeur dans `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ; la détection de succession multi-sauts assiste à la machine d'états.
- JSON golden pour les événements utilisés par les pipelines d'observabilité.

## Télémétrie & observabilité

Métriques (Prometheus) :
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Le fournisseur de télémétrie existant (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) reste dans le périmètre pour les tableaux de bord de bout en bout.

Journaux :
- Flux d'événements Norito structurés pour les audits de gouvernance (signés ?).

Alertes :
- Ordres de réplication en attente dépassant le SLA.
- Expiration d'alias < seuil.
- Violations de rétention (manifeste non renouvelé avant expiration).

Tableaux de bord :
- Le JSON Grafana `docs/source/grafana_sorafs_pin_registry.json` suit les totaux du cycle de vie des manifestes, la couverture d'alias, la saturation du backlog, le ratio SLA, les overlays latence vs slack et les taux d'ordres manqués pour la revue on-call.

## Runbooks et documentation

- Mettre à jour `docs/source/sorafs/migration_ledger.md` pour inclure les mises à jour de statut du registre.
- Guide opérateur : `docs/source/sorafs/runbooks/pin_registry_ops.md` (déjà publié) comprenant métriques, alerting, déploiement, sauvegarde et flux de reprise.
- Guide de gouvernance : décrire les paramètres de politique, le workflow d'approbation, la gestion des litiges.
- Pages de référence API pour chaque endpoint (docs Docusaurus).

## Dépendances & séquencement

1. Terminer les tâches du plan de validation (intégration ManifestValidator).
2. Finaliser le schéma Norito + valeurs par défaut de politique.
3. Implémenter le contrat + service, brancher la télémétrie.
4. Régénérer les luminaires, exécuter les suites d'intégration.
5. Mettre à jour les docs/runbooks et marquer les éléments de la feuille de route comme complets.

Chaque élément de la liste de contrôle SF-4 doit référencer ce plan lorsque la progression est enregistrée.
La façade REST livre désormais des points de terminaison de listing attestés :

- `GET /v1/sorafs/pin` et `GET /v1/sorafs/pin/{digest}` renvoient des manifestes avec
  liaisons d'alias, ordres de réplication et un objet d'attestation dérivé du hash
  du dernier bloc.
- `GET /v1/sorafs/aliases` et `GET /v1/sorafs/replication` exposent le catalogue
  d'alias actif et le backlog des ordres de réplication avec une pagination cohérente
  et des filtres de statut.La CLI encapsule ces appels (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) pour permettre aux opérateurs d'automatiser les audits du
registre sans toucher aux APIs bas niveau.
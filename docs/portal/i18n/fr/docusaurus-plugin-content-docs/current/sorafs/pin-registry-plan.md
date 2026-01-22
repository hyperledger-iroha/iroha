<!-- Auto-generated stub for French (fr) translation. Replace this content with the full translation. -->

---
id: pin-registry-plan
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note Source canonique
Cette page reflète `docs/source/sorafs/pin_registry_plan.md`. Gardez les deux copies synchronisées tant que la documentation héritée reste active.
:::

# Plan d'implémentation du Pin Registry de SoraFS (SF-4)

SF-4 livre le contrat Pin Registry et les services d'appui qui stockent les
engagements de manifest, appliquent les politiques de pinning et exposent des
API à Torii, aux gateways et aux orchestrateurs. Ce document étend le plan de
validation avec des tâches d'implémentation concrètes, couvrant la logique
on-chain, les services côté hôte, les fixtures et les exigences opérationnelles.

## Portée

1. **Machine d'états du registry** : enregistrements Norito pour manifests, aliases,
   chaînes de succession, époques de rétention et métadonnées de gouvernance.
2. **Implémentation du contrat** : opérations CRUD déterministes pour le cycle de vie
   des pins (`ReplicationOrder`, `Precommit`, `Completion`, eviction).
3. **Façade de service** : endpoints gRPC/REST adossés au registry consommés par Torii
   et les SDKs, avec pagination et attestation.
4. **Tooling et fixtures** : helpers CLI, vecteurs de test et documentation pour
   garder manifests, aliases et envelopes de gouvernance synchronisés.
5. **Télémétrie et ops** : métriques, alertes et runbooks pour la santé du registry.

## Modèle de données

### Enregistrements principaux (Norito)

| Struct | Description | Champs |
|--------|-------------|--------|
| `PinRecordV1` | Entrée canonique de manifest. | `manifest_cid`, `chunk_plan_digest`, `por_root`, `profile_handle`, `approved_at`, `retention_epoch`, `pin_policy`, `successor_of`, `governance_envelope_hash`. |
| `AliasBindingV1` | Mappe alias -> CID de manifest. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instruction pour que les providers pinent le manifest. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Accusé de réception du provider. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Snapshot de politique de gouvernance. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Référence d'implémentation : voir `crates/sorafs_manifest/src/pin_registry.rs` pour les
schémas Norito en Rust et les helpers de validation qui soutiennent ces enregistrements.
La validation reflète le tooling manifest (lookup du chunker registry, pin policy gating)
pour que le contrat, les façades Torii et la CLI partagent des invariants identiques.

Tâches :
- Finaliser les schémas Norito dans `crates/sorafs_manifest/src/pin_registry.rs`.
- Générer le code (Rust + autres SDKs) via les macros Norito.
- Mettre à jour la documentation (`sorafs_architecture_rfc.md`) une fois les schémas en place.

## Implémentation du contrat

| Tâche | Owner(s) | Notes |
|-------|----------|-------|
| Implémenter le stockage du registry (sled/sqlite/off-chain) ou un module de smart contract. | Core Infra / Smart Contract Team | Fournir un hashing déterministe, éviter le flottant. |
| Entry points : `submit_manifest`, `approve_manifest`, `bind_alias`, `issue_replication_order`, `complete_replication`, `evict_manifest`. | Core Infra | S'appuyer sur `ManifestValidator` du plan de validation. Le binding d'alias passe maintenant par `RegisterPinManifest` (DTO Torii exposé) tandis que `bind_alias` dédié reste prévu pour des mises à jour successives. |
| Transitions d'état : imposer la succession (manifest A -> B), les époques de rétention, l'unicité des aliases. | Governance Council / Core Infra | L'unicité des aliases, les limites de rétention et les checks d'approbation/retrait des prédécesseurs vivent dans `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ; la détection de succession multi-hop et le bookkeeping de réplication restent ouverts. |
| Paramètres gouvernés : charger `ManifestPolicyV1` depuis la config/l'état de gouvernance ; permettre les mises à jour via événements de gouvernance. | Governance Council | Fournir une CLI pour les mises à jour de politique. |
| Émission d'événements : émettre des événements Norito pour la télémétrie (`ManifestApproved`, `ReplicationOrderIssued`, `AliasBound`). | Observability | Définir le schéma d'événements + logging. |

Tests :
- Tests unitaires pour chaque entry point (positif + rejet).
- Tests de propriétés pour la chaîne de succession (pas de cycles, époques monotones).
- Fuzz de validation en générant des manifests aléatoires (bornés).

## Façade de service (Intégration Torii/SDK)

| Composant | Tâche | Owner(s) |
|-----------|------|----------|
| Service Torii | Exposer `/v1/sorafs/pin` (submit), `/v1/sorafs/pin/{cid}` (lookup), `/v1/sorafs/aliases` (list/bind), `/v1/sorafs/replication` (orders/receipts). Fournir pagination + filtrage. | Networking TL / Core Infra |
| Attestation | Inclure la hauteur/hash du registry dans les réponses ; ajouter une structure d'attestation Norito consommée par les SDKs. | Core Infra |
| CLI | Étendre `sorafs_manifest_stub` ou une nouvelle CLI `sorafs_pin` avec `pin submit`, `alias bind`, `order issue`, `registry export`. | Tooling WG |
| SDK | Générer des bindings client (Rust/Go/TS) depuis le schéma Norito ; ajouter des tests d'intégration. | SDK Teams |

Opérations :
- Ajouter une couche de cache/ETag pour les endpoints GET.
- Fournir rate limiting / auth cohérents avec les politiques Torii.

## Fixtures & CI

- Dossier de fixtures : `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` stocke des snapshots signés de manifest/alias/order régénérés via `cargo run -p iroha_core --example gen_pin_snapshot`.
- Étape CI : `ci/check_sorafs_fixtures.sh` régénère le snapshot et échoue en cas de diff, gardant les fixtures CI alignés.
- Tests d'intégration (`crates/iroha_core/tests/pin_registry.rs`) couvrent le happy path plus le rejet d'alias dupliqué, les guards d'approbation/rétention, les handles de chunker non concordants, la validation du compte de replicas et les échecs de garde de succession (pointeurs inconnus/pré-approuvés/retirés/auto-référencés) ; voir les cas `register_manifest_rejects_*` pour les détails de couverture.
- Les tests unitaires couvrent maintenant la validation d'alias, les guards de rétention et les checks de successeur dans `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ; la détection de succession multi-hop attend la machine d'états.
- JSON golden pour les événements utilisés par les pipelines d'observabilité.

## Télémétrie & observabilité

Métriques (Prometheus) :
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- La télémétrie provider existante (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) reste dans le scope pour les dashboards end-to-end.

Logs :
- Flux d'événements Norito structurés pour les audits de gouvernance (signés ?).

Alertes :
- Ordres de réplication en attente dépassant le SLA.
- Expiration d'alias < seuil.
- Violations de rétention (manifest non renouvelé avant expiration).

Dashboards :
- Le JSON Grafana `docs/source/grafana_sorafs_pin_registry.json` suit les totaux du cycle de vie des manifests, la couverture d'alias, la saturation du backlog, le ratio SLA, les overlays latence vs slack et les taux d'ordres manqués pour la revue on-call.

## Runbooks & documentation

- Mettre à jour `docs/source/sorafs/migration_ledger.md` pour inclure les mises à jour de statut du registry.
- Guide opérateur : `docs/source/sorafs/runbooks/pin_registry_ops.md` (déjà publié) couvrant métriques, alerting, déploiement, sauvegarde et flux de reprise.
- Guide de gouvernance : décrire les paramètres de politique, le workflow d'approbation, la gestion des litiges.
- Pages de référence API pour chaque endpoint (docs Docusaurus).

## Dépendances & séquencement

1. Terminer les tâches du plan de validation (integration ManifestValidator).
2. Finaliser le schéma Norito + defaults de politique.
3. Implémenter le contrat + service, brancher la télémétrie.
4. Régénérer les fixtures, exécuter les suites d'intégration.
5. Mettre à jour docs/runbooks et marquer les items du roadmap comme complets.

Chaque item de checklist SF-4 doit référencer ce plan lorsque le progrès est enregistré.
La façade REST livre désormais des endpoints de listing attestés :

- `GET /v1/sorafs/pin` et `GET /v1/sorafs/pin/{digest}` renvoient des manifests avec
  bindings d'alias, ordres de réplication et un objet d'attestation dérivé du hash
  du dernier bloc.
- `GET /v1/sorafs/aliases` et `GET /v1/sorafs/replication` exposent le catalogue
  d'alias actif et le backlog des ordres de réplication avec une pagination cohérente
  et des filtres de statut.

La CLI encapsule ces appels (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) pour permettre aux opérateurs d'automatiser les audits du
registry sans toucher aux APIs bas niveau.

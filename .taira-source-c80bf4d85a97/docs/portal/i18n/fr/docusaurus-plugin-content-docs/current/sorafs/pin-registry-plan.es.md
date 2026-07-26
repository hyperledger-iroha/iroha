---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan-registre-pin
titre : Plan de mise en œuvre du registre Pin de SoraFS
sidebar_label : Registre du Plan del Pin
description : Plan de mise en œuvre SF-4 qui comprend la machine des états du registre, la façade Torii, l'outillage et l'observabilité.
---

:::note Fuente canonica
Cette page reflète `docs/source/sorafs/pin_registry_plan.md`. Manten ambas copias synchronisés mientras la documentacion heredada siga activa.
:::

# Plan de mise en œuvre du registre Pin de SoraFS (SF-4)

SF-4 entre le contrat du Pin Registry et les services de support qui stockent
compromis sur le manifeste, qui complètent la politique des broches et exposent les API à Torii, les passerelles
et les orchestres. Ce document étend le plan de validation avec les tâches de
mise en œuvre concrète, intégration de la logique en chaîne, des services de l'hôte, des
les installations et les éléments requis pour l'exploitation.

## Alcance

1. **Maquina de estados del Registry** : registros définis par Norito para manifests,
   pseudonymes, chaînes sucesoras, époques de rétention et métadonnées de gouvernance.
2. **Mise en œuvre du contrat** : opérations CRUD déterministes pour le cycle de vie
   de pins (`ReplicationOrder`, `Precommit`, `Completion`, expulsion).
3. **Fachada de servicio** : les points de terminaison gRPC/REST sont répondus au registre consommé
   Torii et les SDK, y compris la pagination et l'attestation.
4. **Outils et accessoires** : aides de CLI, vecteurs de test et documentation pour la maintenance
   manifestes, alias et enveloppes de gobernanza sincronizados.
5. **Télémétrie et opérations** : mesures, alertes et runbooks pour la santé du registre.

## Modèle de données

### Registres centraux (Norito)

| Structure | Description | Campos |
|------------|-------------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Alias ​​Mapea -> CID du manifeste. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instruction pour que les fournisseurs épinglent le manifeste. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Accusé de réception du fournisseur. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Aperçu de la politique gouvernementale. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

Implementation reference: the authoritative manifest lifecycle and finalized
read schemas live in `crates/iroha_data_model/src/sorafs/pin_registry.rs`.
Supporting alias, replication, and policy envelopes live in
`crates/sorafs_manifest/src/pin_registry.rs`. Consensus admission derives and
validates the stored commitments; Torii and operator tooling consume the exact
native finalized record rather than maintaining a second pin-record format.

Status:
- The native `PinManifestRecord` and `PinManifestFinalizedRecordV1` are the V1
  manifest-registry surface used by core, Torii, fixtures, and reference
  validators.
- Rust code generation uses Norito derives; SDK parity follows the normal guard
  lanes whenever the native schema changes.
- Architecture, manifest-pipeline, CLI, OpenAPI, status, and roadmap documents
  describe the shared validation path and endpoint behavior.

## Contract Implementation

| Task | Owner(s) | Notes |
|------|----------|-------|
| Registry storage and smart-contract state. | Core Infra / Smart Contract Team | Implemented in Iroha world state (`pin_manifests`, `manifest_aliases`, `replication_orders`) with deterministic Norito payload hashing and integer-only policy arithmetic. |
| Entry points: `RegisterPinManifest`, `ApprovePinManifest`, `RetirePinManifest`, `BindManifestAlias`, `IssueReplicationOrder`, `CompleteReplicationOrder`, `ExpireReplicationOrder`. | Core Infra | Registration carries the complete canonical manifest, resource-bounds and validates it in consensus, and derives all stored commitments. Core execution also validates aliases, council envelopes, governance permissions, canonical replication payloads, completion, and deadline-bound expiration. |
| State transitions: enforce succession (manifest A -> B), retention epochs, alias uniqueness, and replication status changes. | Governance Council / Core Infra | `ensure_successor_chain` enforces approved, non-retired, acyclic multi-hop lineage; alias uniqueness, retention, and replication issue/complete bookkeeping are covered by unit tests. |
| Governed parameters: load `ManifestPolicyV1` from config/governance state. | Governance Council | Runtime config maps pin-policy constraints into the shared validator. Live policy-change ceremonies are rollout governance evidence, not missing local contract code. |
| Registry telemetry and audit surface. | Observability | Torii exports registry metrics and attested REST snapshots. Additional signed event archives can be layered over those snapshots if governance requires them. |

Coverage:
- Unit tests cover registration, approval, retirement, alias binding, replication
  order issue/complete, permissions, duplicate rejection, and side-effect-free
  failure paths.
- Successor tests cover self references, unknown/pending/retired predecessors,
  cycle closure, and malformed existing predecessor cycles.
- `ci/check_sorafs_fixtures.sh` regenerates chunker, provider-admission, and pin
  registry fixtures and runs the parity checks that keep the canonical schema
  surface stable.

## Service Facade (Torii/SDK Integration)

| Component | Task | Owner(s) |
|-----------|------|----------|
| Torii Service | Ships `/v1/sorafs/pin`, `/v1/sorafs/pin/{digest_hex}`, `/v1/sorafs/aliases`, and `/v1/sorafs/replication`. The manifest-detail route returns exact native `PinManifestFinalizedRecordV1` JSON and accepts only the optional paired expected finalized height/hash precondition; pagination and filters remain on list routes. | Networking TL / Core Infra |
| Finality binding | Listing responses retain their listing attestation. A manifest-detail response carries the native `finalized_cursor` beside the authoritative `PinManifestRecord`; a stale requested cursor fails with HTTP 409. | Core Infra |
| CLI | `iroha app sorafs pin register`, `pin list`, `pin show`, `alias list`, and `replication list` wrap the REST and ISI surfaces for operator audits. | Tooling WG |
| SDK | Rust request builders and the JavaScript, Python, Swift, and C# guard lanes mirror the manifest payload and pin-register validation surface. | SDK Teams |

Operations:
- List endpoints use attested snapshots, deterministic pagination, and the cache
  behavior documented in the alias policy where alias proofs are involved.
- `GET /v1/sorafs/pin/{digest_hex}` returns only `finalized_cursor` and the
  native `manifest`. The retired `limit`, attestation, embedded alias/order
  arrays, counts, and truncation fields are absent; callers use
  `/v1/sorafs/aliases` and `/v1/sorafs/replication` for bounded list queries.
- Mutating operations go through ISI/governance permissions; REST handling keeps
  the same Torii auth and resource-guard model as the surrounding SoraFS APIs.

## Calendriers et CI- Directeur des appareils : `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` garde les instantanés fermes du manifeste/alias/ordre régénérés par `cargo run -p iroha_core --example gen_pin_snapshot`.
- Paso de CI : `ci/check_sorafs_fixtures.sh` régénérera l'instantané et supprimera les différences, en maintenant les appareils de CI alignés.
- Tests d'intégration (`crates/iroha_core/tests/pin_registry.rs`) ejercitan el flujo feliz mas el rechazo de alias duplicado, gardes d'approbation/rétention d'alias, poignées de chunker desalineados, validation de conteo de répliques et chutes de gardes de succession (points desconocidos/preaprobados/retirados/autorreferencias); voir les cas `register_manifest_rejects_*` pour les détails de la couverture.
- Tests unitarios ahora cubren validacion de alias, guards de retencion y checks de successeur en `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ; la détection de succession multi-sauts lorsque la maquina de estados est aterrice.
- JSON Golden pour les événements utilisés par les pipelines d'observabilité.

## Télémétrie et observabilité

Métriques (Prometheus) :
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- La télémétrie existante des fournisseurs (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) est disponible pour les tableaux de bord de bout en bout.

Journaux :
- Stream de eventos Norito structuré pour les auditoriums de gouvernement (firmados ?).

Alertes :
- Ordenes de réplication pendantes dépassant le SLA.
- Expiration de l'alias par debajo del umbral.
- Violaciones de retencion (manifeste non rénové avant l'expiration).

Tableaux de bord :
- Le JSON de Grafana `docs/source/grafana_sorafs_pin_registry.json` rastrea total du cycle de vie des manifestes, couverture d'alias, saturation du backlog, ratio de SLA, superpositions de latence vs slack et charges de commandes perdues pour la révision d'astreinte.

## Runbooks et documentation

- Actualiser `docs/source/sorafs/migration_ledger.md` pour inclure les actualisations de l'état du registre.
- Guide des opérateurs : `docs/source/sorafs/runbooks/pin_registry_ops.md` (vous avez publié) cubriendo metricas, alertas, despliegue, backup and flujos de recuperacion.
- Guide d'administration : décrit les paramètres politiques, le flux de travail d'approbation, la gestion des litiges.
- Pages de référence de l'API pour chaque point de terminaison (documents Docusaurus).

## Dépendances et sécurité

1. Compléter les zones du plan de validation (intégration de ManifestValidator).
2. Finaliser le schéma Norito + valeurs politiques par défaut.
3. Mettre en œuvre un contrat + service, connecter la télémétrie.
4. Régénérer les luminaires, corriger les suites d'intégration.
5. Actualiser les documents/runbooks et marquer les éléments de la feuille de route comme complets.

Chaque liste de contrôle du SF-4 doit être référencée à ce plan lorsqu'elle est marquée en cours.
La page REST contient maintenant les points finaux de la liste avec attestation :

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` et `GET /v1/sorafs/replication` exposent le catalogue de
  alias actif et le backlog des commandes de réplication avec une page cohérente et
  filtres d'état.La CLI envoie ces appels (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) pour que les opérateurs puissent automatiser les salles du
le registre ne couvre pas les API de bas niveau.
---
lang: ja
direction: ltr
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/sorafs/pin-registry-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7e951db0df1ab4b19107103176af19e5665afbba1d181a6bb6bbbc8b777b3a0c
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---


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
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Mappe alias -> CID de manifest. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instruction pour que les providers pinent le manifest. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Accusé de réception du provider. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Snapshot de politique de gouvernance. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

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

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` et `GET /v1/sorafs/replication` exposent le catalogue
  d'alias actif et le backlog des ordres de réplication avec une pagination cohérente
  et des filtres de statut.

La CLI encapsule ces appels (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) pour permettre aux opérateurs d'automatiser les audits du
registry sans toucher aux APIs bas niveau.

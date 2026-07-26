---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan-registre-pin
titre : Plan de mise en œuvre du registre des broches par SoraFS
sidebar_label : Registre Plano do Pin
description : Plan de mise en œuvre SF-4 cobrindo a maquina de estados do Registry, a fachada Torii, outillage et observabilité.
---

:::note Fonte canonica
Cette page reflète `docs/source/sorafs/pin_registry_plan.md`. Mantenha ambas as copias sincronizadas quanto a documentacao herdada permanecer ativa.
:::

# Plan de mise en œuvre du registre Pin SoraFS (SF-4)

Le SF-4 entre le contrat du registre Pin et les services d'activation
compromissos de manifeste, impoem politicas de pin et expoem APIs para Torii,
passerelles et orchestres. Ce document est étendu ou plan de validation avec
tâches de mise en œuvre concrètes, cobrindo a logica on-chain, les services du
hôte, les installations et les exigences opérationnelles.

## Escopo

1. **Maquina de estados do Registry** : registros définisdos por Norito para manifests,
   pseudonymes, cadeias successeurs, époques de rétention et métadonnées de gouvernance.
2. **Mise en œuvre du contrat** : opérations déterministes CRUD pour le cycle de vie
   dos pins (`ReplicationOrder`, `Precommit`, `Completion`, expulsion).
3. **Fachada de servico**: endpoints gRPC/REST soutenus par le registre Torii
   Il existe un ensemble de SDK, y compris la page et le certificat.
4. **Outils et accessoires** : aides de CLI, anciens tests et documentation pour le reste
   manifestes, alias et enveloppes de gouvernance synchronisées.
5. **Télémétrie et opérations** : mesures, alertes et runbooks pour le registre légitime.

## Modèle de données

### Registres centraux (Norito)

| Structure | Description | Campos |
|--------|-----------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Alias ​​Mapeia -> CID du manifeste. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Instructions pour les fournisseurs de réparation du manifeste. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Confirmation du fournisseur. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Aperçu de la politique de gouvernance. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

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

## Calendriers et CI- Répertoire des appareils : `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` armazena snapshots assassinés de manifeste/alias/ordre régénérés par `cargo run -p iroha_core --example gen_pin_snapshot`.
- Étape de CI : `ci/check_sorafs_fixtures.sh` régénérera l'instantané et falha se maintiendra en diffs, en gardant les appareils de CI alignés.
- Testes d'intégration (`crates/iroha_core/tests/pin_registry.rs`) exercent le flux agréable mais avec le rejet d'alias dupliqué, gardes d'approbation/retenue d'alias, poignées de chunker incompatibles, validation de contagem de répliques et falhas de guardas de successao (ponteiros desconhecidos/preaprovados/retirados/autorreferencias); Voir les cas `register_manifest_rejects_*` pour les détails de la couverture.
- Testes unitarios agora cobrem validacao de alias, guards de retencao e checks of successeur em `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ; une détection de succès multi-saut quando a maquina de estados chegar.
- JSON Golden pour les événements utilisés par les pipelines d'observation.

## Télémétrie et observation

Métriques (Prometheus) :
-`torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
-`torii_sorafs_registry_aliases_total`
-`torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
-`torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
-`torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
-`torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Une télémétrie existante des fournisseurs (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) est permanente sous surveillance pour les tableaux de bord de bout en bout.

Journaux :
- Stream de eventos Norito structuré pour les auditoires de gouvernance (assinados ?).

Alertes :
- Ordres de réplication pendants dépassant le SLA.
- Expiracao de alias abaixo do limiar.
- Violacoes de retencao (manifeste nao renovado antes de expirar).

Tableaux de bord :
- Le JSON du Grafana `docs/source/grafana_sorafs_pin_registry.json` rastreia totalise le cycle de vie des manifestes, la couverture des alias, la saturation du backlog, le calcul du SLA, les superpositions de latence par rapport au slack et les taxes de commandes perdues pour la révision sur appel.

## Runbooks et documentation

- Actualiser `docs/source/sorafs/migration_ledger.md` pour inclure l'actualisation de l'état du registre.
- Guide de l'opérateur : `docs/source/sorafs/runbooks/pin_registry_ops.md` (ja publié) cobrindo métriques, alertes, déploiement, sauvegarde et flux de récupération.
- Guide de gouvernance : découvrir les paramètres politiques, le flux de travail d'approbation, le traitement des litiges.
- Pages de référence de l'API pour chaque point de terminaison (docs Docusaurus).

## Dépendances et séquencement

1. Compléter les tarifications du plan de validation (intégration du ManifestValidator).
2. Finaliser le schéma Norito + valeurs politiques par défaut.
3. Mettre en œuvre un contrat + service, connecter la télémétrie.
4. Régénérer les luminaires, les suites rodar de integracao.
5. Actualiser les documents/runbooks et marquer les éléments de la feuille de route comme des éléments complets.

Chaque liste de contrôle du SF-4 doit être référencée à ce plan lorsque vous continuez.
La page REST vient d'entrer les points de terminaison de la liste avec attestation :

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` et `GET /v1/sorafs/replication` exposition ou catalogue de
  Alias ativo et le backlog des commandes de réplication avec une page cohérente et
  filtres de statut.Une CLI encapsule ces chamadas (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) pour que les opérateurs puissent automatiser les auditoires
registre avec les API de bas niveau.
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
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | alias -> mappage CID manifeste. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | fournisseurs et broches manifestes. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | accusé de réception du fournisseur. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | instantané de la politique de gouvernance. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

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

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` vers `GET /v1/sorafs/replication` vers le catalogue alias vers
  arriéré des ordres de réplication et pagination cohérente et filtres d'état

CLI appelle et enveloppe کرتی ہے (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) Opérateurs de gestion d'API et d'audits de registre pour les audits de registre
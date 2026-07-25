---
lang: dz
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7cc63e7549adebfe3ab539eca608e2fc88830361b3fe53b165491e36ecb83177
source_last_modified: "2026-01-22T14:35:36.748626+00:00"
translation_last_reviewed: 2026-02-07
id: pin-registry-plan
title: SoraFS Pin Registry Implementation Plan
sidebar_label: Pin Registry Plan
description: SF-4 implementation plan covering registry state machine, Torii facade, tooling, and observability.
translator: machine-google-reviewed
---

:::དྲན་ཐོའི་འབྱུང་ཁུངས།
:::

# SoraFS པིན་ཐོ་བཀོད་ལག་བསྟར་འཆར་གཞི་ (SF-4)

SF-4 གིས་ པིན་ཐོ་བཀོད་ཀྱི་གན་རྒྱ་དང་ རྒྱབ་སྐྱོར་གྱི་ཞབས་ཏོག་ཚུ་ གསོག་འཇོག་འབདཝ་ཨིན།
གསལ་སྟོན་ཁས་བླངས་, པིན་སྲིད་བྱུས་བསྟར་སྤྱོད་འབད་ཞིནམ་ལས་ I18NT0000014X, འཛུལ་སྒོ་ཚུ་ APIs ཚུ་ ཕྱིར་བཏོན་འབདཝ་ཨིན།
དང་ སྙན་ཆའི་སྡེ་ཚན་ཚུ། ཡིག་ཆ་འདི་གིས་ བདེན་དཔྱད་འཆར་གཞི་འདི་ བརྟན་པོ་སྦེ་རྒྱ་སྐྱེད་འབདཝ་ཨིན།
ལག་ལེན་འཐབ་ཐངས་ཚུ་ རིམ་སྒྲིག་ཚད་མ་དང་ གཙོ་བོར་གྱི་ཞབས་ཏོག་ དེ་ལས་ སྒྲིག་ཆས་ཚུ་ ཁྱབ་སྟེ་ཡོདཔ་ཨིན།
དང་ ལག་ལེན་གྱི་ དགོས་མཁོ།

## གོ་སྐབས

1. **ཐོ་བཀོད་གནས་སྟངས་འཕྲུལ་ཆས་**: Norito-ངེས་འཛིན་འབད་ཡོད་པའི་དྲན་ཐོ་ཚུ་ གསལ་སྟོན་ཚུ་གི་དོན་ལུ་ ཚིག་བརྗོད་ཚུ།
   ཤུལ་འཛིན་པའི་རིམ་པ་དང་ བཀག་འཛིན་གྱི་དུས་སྐབས་ དེ་ལས་ གཞུང་སྐྱོང་མེ་ཊ་ཌེ་ཊ་ཚུ་ཨིན།
2. **གན་ཡིག་ལག་ལེན་འཐབ་ཐངས་**: པིན་མི་ཚེ་འཁོར་རིམ་གྱི་དོན་ལུ་ གཏན་འབེབས་བཟོ་ནིའི་ CRUD བཀོལ་སྤྱོད་ཚུ།
   (I 18NI00000021X, `Precommit`, `Completion`, ཕྱིར་འདོན་པ།)
3. **ཞབས་ཏོག་གདོང་ཕྱོགས་**: ཇི་ཨར་པི་སི་/ཨར་ཨི་ཨེསི་ཊི་མཐའ་ཐིག་ཚུ་ ཐོ་བཀོད་ཀྱིས་རྒྱབ་སྐྱོར་འབད་མི་ I18NT0000015X ཨིན།
   དང་ SDKs ཚུ་ ཤོག་ལེབ་དང་ བདེན་ཁུངས་ཚུ་རྩིས་ཏེ་ ཟ་སྤྱོད་འབདཝ་ཨིན།
4. **Tooling དང་ Teons**: CLI རོགས་རམ་འབད་མི་ བརྟག་དཔྱད་བེག་ཊར་ དེ་ལས་ ཡིག་ཆ་ཚུ་ བདག་འཛིན་འཐབ་ནིའི་དོན་ལུ་ཨིན།
   མཉམ་མཐུན་ནང་ གསལ་སྟོན་དང་ མིང་གཞན་ དེ་ལས་ གཞུང་སྐྱོང་ཡིག་ཆ་ཚུ་ གསལ་སྟོན་འབདཝ་ཨིན།
༥. *Telemetry & Ops**: ཐོ་བཀོད་གསོ་བའི་དོན་ལུ་ མེ་ཊིག་དང་ ཉེན་བརྡ་ དེ་ལས་ རྒྱུག་དེབ་ཚུ།

## གནད་སྡུད་དཔེ་གཞི།

### ཀོར་དྲན་ཐོ་ (I18NT0000004X)

| སྒྲིག་བཀོད་ | འགྲེལ་བཤད་ | ཕིལཌ་ |
|---------------------------|-------------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| I18NI0000034X | སབ་ཁྲ་ཚུ་ -> གསལ་སྟོན་སི་ཨའི་ཌི་། | `alias`, `manifest_cid`, I18NI000000003X, I18NI0000000038X. |
| I18NI0000039X | བྱིན་མི་ཚུ་གིས་ གསལ་སྟོན་འབད་ནི་ལུ་ བཀོད་རྒྱ། | `manifest_cid`, I18NI0000000042X, I18NI000000043X, `deadline`, `deadline`, `policy_hash`. |
| I18NI0000046X | མཁོ་སྤྲོད་འབད་མི་ངོས་ལེན་འབད་ནི། | I18NI000000047X, `provider_id`, I18NI000000049X, I18NI000000000500X, I18NI00000000500, I18NI000000051X. |
| I18NI0000002X | གཞུང་སྐྱོང་སྲིད་བྱུས་པར་བཀོད། | I18NI000000053X, `max_retention_epochs`, Norito, I18NI000000066X. |

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

## བཅོས་མ་དང་སི་ཨའི།

- བདེ་སྒྲིག་སྣོད་ཐོ་: I18NI0000084X ཚོང་ཁང་ཚུ་གིས་ `cargo run -p iroha_core --example gen_pin_snapshot` གིས་ བསྐྱར་བཟོ་འབད་མི་ གསལ་སྟོན་/མིང་ཚིག་/གོ་རིམ་གྱི་པར་ཚུ་ མཚན་རྟགས་བཀོད་ཡོདཔ་ཨིན།
- CI གོ་རིམ་: `ci/check_sorafs_fixtures.sh` གིས་ པར་རིས་འདི་ བསྐྱར་བཟོ་འབདཝ་ཨིནམ་དང་ གལ་སྲིད་ ཌིཕ་ཚུ་ཐོན་པ་ཅིན་ འཐུས་ཤོར་བྱུངམ་ཨིན།
- མཉམ་བསྡོམས་བརྟག་དཔྱད་ (I18NI0000087X) གིས་ དགའ་སྤྲོའི་ལམ་དང་ འདྲ་བཤུས་ཆ་འཇོག་/བཀག་ཆ་འབད་མི་ མཐུན་སྒྲིག་ཅན་གྱི་ ཆུ་བོ་འཛིན་ཆས་ འདྲ་གྲངས་བདེན་དཔྱད་ དེ་ལས་ ཤུལ་འཛིན་ལམ་སྟོན་གྱི་ འཐུས་ཤོར་ (མ་ཤེས་/སྔོན་སྒྲིག་འབད་ཡོདཔ་/བཀག་ཆ་འབད་མི་/སེཕ་པོའིནཊི་ཚུ་) ཚུ་ ལག་ལེན་འཐབ་ཨིན། ཁྱབ་ཁོངས་ཁ་གསལ་གྱི་དོན་ལུ་ Norito གནད་དོན་ཚུ་བལྟ།
- ད་ལྟོ་ ཡུ་ནིཊ་བརྟག་དཔྱད་ཚུ་གིས་ བདེན་དཔྱད་དང་ བཀག་འཛིན་འབད་མི་ དེ་ལས་ ཤུལ་འཛིན་གྱི་བརྟག་དཔྱད་ཚུ་ `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ནང་ལུ་ ཁྱབ་ཚུགསཔ་ཨིན། མང་ཚོགས་ཀྱི་འཕྲུལ་ཆས་ས་ཆ་ཚུ་ ཚར་གཅིག་ སྣ་མང་-ཧོབ་ཀྱི་ རིམ་སྒྲིག་བརྟག་དཔྱད་ འབད་ནི།
- བལྟ་བརྟོག་འབད་བཏུབ་པའི་ པའིཔ་མདོང་གིས་ལག་ལེན་འཐབ་མི་ བྱུང་ལས་ཚུ་གི་དོན་ལུ་ གསེར་གྱི་ཇེ་ཨེསི་ཨོ་ཨེན་།

## བརྒྱུད་འཕྲིན་དང་བལྟ་ཚུགས།

མེ་ཊིག་ (I18NT000000000X):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- ད་ལྟོ་ཡོད་པའི་ བྱིན་མི་ བརྒྱུད་འཕྲིན་ (I18NI0000096X, `torii_sorafs_fee_projection_nanos`) འདི་ མཇུག་ལས་མཇུག་ཚུན་ཚོད་ ཌེཤ་བོརཌི་ཚུ་གི་དོན་ལུ་ གོ་སྐབས་ནང་ ལུས་ཡོདཔ་ཨིན།

དྲན་ཐོ་།
- གཞུང་སྐྱོང་རྩིས་ཞིབ་ཚུ་གི་དོན་ལུ་ བཀོད་སྒྲིག་ I18NT0000011X བྱུང་ལས་རྒྱུན་ལམ་ (མིང་རྟགས་བཀོད་ཡོདཔ་?)།

ཉེན་བརྡ་:
- ཨེསི་ཨེལ་ཨེ་ལས་བརྒལ་ཏེ་ འདྲ་དཔེ་བཟོ་ནིའི་བཀའ་རྒྱ་ཚུ་ བསྒུག་ནི།
- མིང་གཞན་ < ཚད་གཞི།
- བཀག་འཛིན་གྱི་འགལ་འཛོལ་ཚུ་ (དུས་ཡུན་མ་རྫོགས་པའི་ཧེ་མར་ གཡོ་སྒྱུ་འདི་ བསྐྱར་གསོ་མ་འབད་བས།)

ཌེཤ་བོརཌ་ཚུ།
- I18NT000000002X JSON I18NI0000098X གིས་ མི་ཚེ་གི་བསྡོམས་རྩིས་དང་ མིང་གཞན་ རྒྱབ་ལོག་ཚད་གཞི་ SLA ཆ་ཚད་ བསྐྱར་ལོག་དང་ བཀབ་སྟེ་ དེ་ལས་ བཀའ་རྒྱ་བསྐྱར་ཞིབ་ཀྱི་ གོ་རིམ་ཚུ་ བཏོནམ་ཨིན།

## གཡོག་བཀོལ་དེབ་དང་ཡིག་ཆ།

- ཐོ་བཀོད་གནས་རིམ་དུས་མཐུན་ཚུ་བཙུགས་ནིའི་དོན་ལུ་ `docs/source/sorafs/migration_ledger.md` དུས་མཐུན་བཟོ།
- བཀོལ་སྤྱོད་ལམ་སྟོན་པ་: `docs/source/sorafs/runbooks/pin_registry_ops.md` (ད་ལྟོ་དཔར་བསྐྲུན་འབད་ཡོད་པའི་) མེ་ཊིག་དང་ ཉེན་བརྡ་ བཀྲམ་སྤེལ་ རྒྱབ་ཐག་ དེ་ལས་ སླར་གསོ་རྒྱུན་འབབ་ཚུ་ བཀབ་ནི།
- གཞུང་སྐྱོང་ལམ་སྟོན་: སྲིད་བྱུས་ཚད་གཞི་དང་ ཆ་འཇོག་ལཱ་གི་རྒྱུན་རིམ་ རྩོད་གཞི་འཛིན་སྐྱོང་།
- མཐའ་མཚམས་རེ་རེ་གི་དོན་ལུ་ ཨེ་པི་ཨའི་ གཞི་བསྟུན་ཤོག་ལེབ་ (I18NT0000001X docs).

## བརྟེན་པ་དང་གོ་རིམ།

༡ བདེན་དཔྱད་འཆར་གཞི་ལས་འགན་ཚུ་མཇུག་བསྡུ་ (འགན་འཁུར་མཉམ་བསྡོམས)།
2. Norito ལས་འཆར་ + སྲིད་བྱུས་སྔོན་སྒྲིག་ཚུ་ མཇུག་བསྡུ།
༣ གན་རྒྱ་ + ཞབས་ཏོག་ གློག་ཐག་བརྡ་འཕྲིན་ལག་ལེན་འཐབ་དགོ།
༤ བསྐྱར་བཟོ་འབད་ནི། མཉམ་བསྡོམས་ཁང་མིག་ཚུ་གཡོག་བཀོལ།
༥ ཡིག་ཆ་ཚུ་དུས་མཐུན་བཟོ་ཞིནམ་ལས་ ལམ་སྟོན་གྱི་ཅ་ཆས་ཚུ་ མཇུག་བསྡུ་དགོ།

ཡར་རྒྱས་འགྱོ་བའི་སྐབས་ལུ་ SF-4 གི་འོག་ལུ་ཡོད་པའི་ ལམ་སབ་ཁྲ་གི་ཞིབ་དཔྱད་ཐོ་ཡིག་རེ་རེ་གིས་ འཆར་གཞི་འདི་ གཞི་བསྟུན་འབད་དགོ།
ད་ལྟོ་ REST གི་གདོང་ཕྱོགས་འདི་གིས་ ཐོ་བཀོད་ཀྱི་མཇུག་བསྡུའི་བདེན་ཁུངས་ཚུ་ བདེན་ཁུངས་བཀལ་ཡོདཔ་ཨིན།

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` དང་ `GET /v1/sorafs/replication` ཤུགས་ལྡན་ཕྱིར་སྟོན་འབདཝ་ཨིན།
  alias ཐོ་གཞུང་དང་ འདྲ་དཔེ་བཀོད་པའི་གོ་རིམ་འདི་ རྟག་བརྟན་གྱི་ ཤོག་ལེབ་དང་ དང་།
  གནས་ཚད་ཚགས་མ་ཚུ།

CLI གིས་ འབོད་བརྡ་འདི་ཚུ་ (`iroha app sorafs pin list`, `pin show`, `alias list`, འདི་ བཀབ་བཞགཔ་ཨིན།
`replication list`) དེ་འབདཝ་ལས་ བཀོལ་སྤྱོད་པ་ཚུ་གིས་ ཡིག་ཚུགས་ཐོ་བཀོད་རྩིས་ཞིབ་ཚུ་ ལགཔ་མ་རྐྱབ་པར་ འབད་ཚུགས།
དམའ་རིམ་ཨེ་པི་ཨའི་ཚུ།
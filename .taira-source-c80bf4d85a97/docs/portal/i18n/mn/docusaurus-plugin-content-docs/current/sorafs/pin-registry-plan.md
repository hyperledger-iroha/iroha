---
id: pin-registry-plan
lang: mn
direction: ltr
source: docs/portal/docs/sorafs/pin-registry-plan.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS Pin Registry Implementation Plan
sidebar_label: Pin Registry Plan
description: SF-4 implementation plan covering registry state machine, Torii facade, tooling, and observability.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

::: Каноник эх сурвалжийг анхаарна уу
:::

# SoraFS Pin бүртгэлийн хэрэгжилтийн төлөвлөгөө (SF-4)

SF-4 нь Pin бүртгэлийн гэрээ болон хадгалдаг туслах үйлчилгээг хүргэдэг
амлалт өгөх, пин бодлогыг хэрэгжүүлэх, API-г Torii, гарцууд,
болон найрал хөгжимчид. Энэхүү баримт бичиг нь баталгаажуулалтын төлөвлөгөөг бетоноор өргөжүүлдэг
гинжин логик, хост талын үйлчилгээ, бэхэлгээ зэргийг хамарсан хэрэгжүүлэх ажлууд,
болон үйл ажиллагааны шаардлага.

## Хамрах хүрээ

1. **Бүртгэлийн төлөвийн машин**: Norito-тодорхойлогдсон манифест, нэр,
   залгамжлагч хэлхээ, хадгалах эрин үе, засаглалын мета өгөгдөл.
2. **Гэрээний хэрэгжилт**: зүү амьдралын мөчлөгийн тодорхойлогч CRUD үйлдлүүд
   (`ReplicationOrder`, `Precommit`, `Completion`, нүүлгэн шилжүүлэх).
3. **Үйлчилгээний фасад**: Torii бүртгэлээр баталгаажсан gRPC/REST төгсгөлийн цэгүүд
   болон SDK-ууд, үүнд хуудаслалт, баталгаажуулалт орно.
4. **Багаж хэрэгсэл, бэхэлгээ**: CLI-ийн туслахууд, туршилтын векторууд, хадгалах баримт бичиг
   манифест, нэр, засаглалын дугтуйг синхрончилно.
5. **Телеметри ба үйл ажиллагаа**: бүртгэлийн эрүүл мэндэд зориулсан хэмжигдэхүүн, сэрэмжлүүлэг, runbooks.

## Өгөгдлийн загвар

### Үндсэн бүртгэл (Norito)

| Бүтэц | Тодорхойлолт | Талбарууд |
|--------|-------------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Газрын зургийн бусад нэр -> манифест CID. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Үйлчилгээ үзүүлэгчдийн манифестийг тогтоох заавар. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Үйлчилгээ үзүүлэгчийн хүлээн зөвшөөрөлт. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Засаглалын бодлогын агшин зураг. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

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

## Бэхэлгээ & CI

- Барилгын лавлах: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` нь `cargo run -p iroha_core --example gen_pin_snapshot`-ээр сэргээсэн гарын үсэгтэй манифест/алиа/захиалгын агшин агшинг хадгалдаг.
- CI алхам: `ci/check_sorafs_fixtures.sh` нь агшин зуурын зургийг сэргээж, хэрэв ялгаа гарч ирвэл амжилтгүй болж, CI бэхэлгээг зэрэгцүүлэн хадгална.
- Интеграцийн тестүүд (`crates/iroha_core/tests/pin_registry.rs`) аз жаргалтай зам дээр давхардсан нэрсийг үгүйсгэх, өөр нэр батлах/хадгалах хамгаалалт, таарахгүй chunker бариул, хуулбар тоолох баталгаажуулалт, залгамжлагчийн хамгаалалтын алдаа (үл мэдэгдэх/урьдчилан батлагдсан/тэтгэвэрт гарсан/өөрийгөө заагч); Хамрах хүрээний дэлгэрэнгүйг `register_manifest_rejects_*` тохиолдлоос харна уу.
- Нэгжийн туршилтууд нь одоо `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` дахь нэрийн баталгаажуулалт, хадгалах хамгаалалт, залгамжлагчийн шалгалтыг хамардаг; төрийн машин газардсаны дараа олон хоп залгамжлал илрүүлэх.
- Ажиглалтын шугамын ашигладаг үйл явдлуудад зориулсан Алтан JSON.

## Телеметр ба ажиглалт

Хэмжилт (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Одоо байгаа үйлчилгээ үзүүлэгчийн телеметр (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) нь төгсгөлийн хяналтын самбарт хамаарах хэвээр байна.

Бүртгэлүүд:
- Засаглалын аудитын зохион байгуулалттай Norito үйл явдлын урсгал (гарын үсэг зурсан?).

Анхааруулга:
- Хүлээгдэж буй хуулбарлах захиалга нь SLA-аас хэтэрсэн.
- Алиарын хугацаа дуусах < босго.
- Хадгалалтын зөрчил (хугацаа дуусахаас өмнө манифест шинэчлэгдээгүй).

Хяналтын самбар:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` нь манифестийн амьдралын мөчлөгийн нийлбэр, нэрийн хамрах хүрээ, хоцрогдсон бүртгэлийн ханасан байдал, SLA харьцаа, хоцролт ба сул давхцал, дуудлага дээр хянуулахын тулд орхигдсон захиалгын хэмжээг хянадаг.

## Runbooks & Documentation

- Бүртгэлийн статусын шинэчлэлтүүдийг оруулахын тулд `docs/source/sorafs/migration_ledger.md`-г шинэчил.
- Операторын гарын авлага: `docs/source/sorafs/runbooks/pin_registry_ops.md` (одоо нийтлэгдсэн) хэмжигдэхүүн, сэрэмжлүүлэг, байршуулалт, нөөцлөлт, сэргээх урсгалыг хамарсан.
- Засаглалын гарын авлага: бодлогын параметрүүд, батлах ажлын урсгал, маргааныг шийдвэрлэх талаар тайлбарлана.
- Төгсгөлийн цэг бүрийн API лавлагааны хуудас (Docusaurus docs).

## Хамаарал ба дараалал

1. Баталгаажуулалтын төлөвлөгөөний даалгавруудыг гүйцээнэ үү (ManifestValidator нэгтгэх).
2. Norito схем + бодлогын өгөгдмөлүүдийг эцэслэнэ үү.
3. Гэрээ + үйлчилгээ, утсан телеметрийг хэрэгжүүлэх.
4. Бэхэлгээг сэргээж, нэгтгэх багцуудыг ажиллуул.
5. Docs/runbooks-г шинэчилж, замын газрын зураг дууссан гэж тэмдэглээрэй.

SF-4-ийн дагуух замын зургийн хяналтын хуудас бүр ахиц дэвшил гарсан үед энэ төлөвлөгөөг иш татна.
REST фасадыг одоо баталгаажуулсан жагсаалтын төгсгөлийн цэгүүдээр нийлүүлдэг.

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` ба `GET /v1/sorafs/replication` идэвхтэй бодисыг илрүүлдэг.
  нэрийн каталог болон хуулбарлах захиалгын дарааллыг тууштай хуудаслах ба
  статус шүүлтүүрүүд.

CLI нь эдгээр дуудлагыг (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) тул операторууд бүртгэлийн аудитыг гар хүрэхгүйгээр бичих боломжтой.
доод түвшний API.
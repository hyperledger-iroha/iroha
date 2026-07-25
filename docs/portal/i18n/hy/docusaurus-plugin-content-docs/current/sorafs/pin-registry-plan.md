---
id: pin-registry-plan
lang: hy
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

:::note Կանոնական աղբյուր
:::

# SoraFS Pin ռեեստրի իրականացման պլան (SF-4)

SF-4-ը տրամադրում է Pin Registry պայմանագիրը և աջակցող ծառայությունները, որոնք պահում են
դրսևորել պարտավորությունները, կիրառել կապի քաղաքականություն և բացահայտել API-ները Torii-ին, դարպասներին,
և նվագախմբեր։ Այս փաստաթուղթը ընդլայնում է վավերացման պլանը կոնկրետով
իրականացման առաջադրանքներ, որոնք ներառում են շղթայական տրամաբանությունը, հյուրընկալող կողմի ծառայությունները, հարմարանքները,
և գործառնական պահանջները:

## Շրջանակ

1. **Ռեեստրի պետական մեքենա**.
   հաջորդող շղթաներ, պահպանման դարաշրջաններ և կառավարման մետատվյալներ:
2. **Պայմանագրի իրականացում**. դետերմինիստական CRUD գործողություններ փին կյանքի ցիկլի համար
   (`ReplicationOrder`, `Precommit`, `Completion`, վտարում):
3. **Ծառայության ճակատը**. gRPC/REST վերջնակետեր ապահովված ռեեստրի կողմից, որը Torii
   և SDK-ները սպառում են, ներառյալ էջադրումը և ատեստավորումը:
4. **Գործիքներ և հարմարանքներ**. CLI օգնականներ, փորձարկման վեկտորներ և փաստաթղթեր, որոնք պետք է պահպանվեն
   դրսևորումները, կեղծանունները և կառավարման ծրարները համաժամանակյա:
5. **Հեռաչափություն և օպերացիա**. չափումներ, ծանուցումներ և ռեեստրի առողջության համար նախատեսված գրքույկներ:

## Տվյալների մոդել

### Հիմնական գրառումներ (Norito)

| Կառուցվածք | Նկարագրություն | Դաշտեր |
|--------|-------------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Քարտեզներ alias -> manifest CID: | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`: |
| `ReplicationOrderV1` | Հրահանգ մատակարարների համար՝ ամրացնել մանիֆեստը: | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`: |
| `ReplicationReceiptV1` | Մատակարարի հաստատում: | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`: |
| `ManifestPolicyV1` | Կառավարման քաղաքականության ակնարկ. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`: |

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

## Հարմարանքներ և CI

- Սարքավորումների գրացուցակ. `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` խանութներում ստորագրված մանիֆեստներ/փոխանուն/պատվերի նկարներ, որոնք վերականգնվել են `cargo run -p iroha_core --example gen_pin_snapshot`-ի կողմից:
- CI քայլ. `ci/check_sorafs_fixtures.sh`-ը վերականգնում է լուսանկարը և ձախողվում է, եթե տարբերություններ հայտնվեն՝ պահելով CI սարքերը հավասարեցված:
- Ինտեգրման թեստերը (`crates/iroha_core/tests/pin_registry.rs`) իրականացնում են երջանիկ ուղին, գումարած կրկնօրինակների մերժումը, կեղծանունների հաստատման/պահպանման պահակները, չհամապատասխանող բլոկների բռնակները, կրկնօրինակների քանակի վավերացումը և իրավահաջորդների պահակային ձախողումները (անհայտ/նախապես հաստատված/թոշակի անցած/ինքնացուցիչներ); Ծածկույթի մանրամասների համար տես `register_manifest_rejects_*` պատյանները:
- Միավորի թեստերն այժմ ներառում են կեղծանունների վավերացումը, պահպանման պահակները և իրավահաջորդների ստուգումները `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`-ում; բազմակի հոպ հաջորդականության հայտնաբերում, երբ պետական ​​մեքենան վայրէջք կատարի:
- Ոսկե JSON իրադարձությունների համար, որոնք օգտագործվում են դիտելիության խողովակաշարերի կողմից:

## Հեռաչափություն և դիտելիություն

Չափումներ (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Գոյություն ունեցող մատակարարի հեռաչափությունը (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) մնում է ծայրից ծայր վահանակների շրջանակում:

Տեղեկամատյաններ:
- Կառուցվածքային Norito իրադարձությունների հոսք կառավարման աուդիտի համար (ստորագրված է?):

Զգուշացումներ.
- Սպասվող կրկնօրինակման պատվերները գերազանցում են SLA-ը:
- Alias ​​expiry < շեմ.
- Պահպանման խախտումներ (դրսևորվում է, որ ժամկետը լրանալուց առաջ չի երկարաձգվել):

Վահանակներ.
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json`-ը հետագծում է կյանքի ցիկլի բացահայտումների ընդհանուր գումարները, ծածկանունների ծածկույթը, կուտակվածության հագեցվածությունը, SLA հարաբերակցությունը, հետաձգումը ընդդեմ անփույթ ծածկույթների և բաց թողնված պատվերի դրույքաչափերը՝ ժամանակի ընթացքում վերանայման համար:

## Runbooks & Documentation

- Թարմացրեք `docs/source/sorafs/migration_ledger.md`՝ ներառելու ռեեստրի կարգավիճակի թարմացումները:
- Օպերատորի ուղեցույց. `docs/source/sorafs/runbooks/pin_registry_ops.md` (այժմ հրապարակված) ընդգրկում է չափումները, ահազանգերը, տեղակայումը, պահուստավորումը և վերականգնման հոսքերը:
- Կառավարման ուղեցույց. նկարագրեք քաղաքականության պարամետրերը, հաստատման աշխատանքների ընթացքը, վեճերի լուծումը:
- API տեղեկատու էջեր յուրաքանչյուր վերջնակետի համար (Docusaurus փաստաթղթեր):

## Կախվածություններ և հաջորդականություն

1. Լրացրեք վավերացման պլանի առաջադրանքները (ManifestValidator ինտեգրում):
2. Վերջնականացրեք Norito սխեման + քաղաքականության լռելյայն:
3. Իրականացնել պայմանագիր + սպասարկում, լարային հեռաչափություն։
4. Վերականգնել հարմարանքները, գործարկել ինտեգրացիոն սյուիտները:
5. Թարմացրեք փաստաթղթերը/վազքագրքերը և նշեք ճանապարհային քարտեզի տարրերը ավարտված:

Ճանապարհային քարտեզի ստուգաթերթի յուրաքանչյուր կետ SF-4-ում պետք է հղում կատարի այս պլանին, երբ առաջընթաց լինի:
REST ճակատն այժմ առաքվում է վավերացված ցուցակման վերջնակետերով.

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` և `GET /v1/sorafs/replication` ցուցադրում են ակտիվը
  alias catalog և replication order backlog՝ հետևողական էջադրմամբ և
  կարգավիճակի զտիչներ:

CLI-ն ավարտում է այս զանգերը (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`), որպեսզի օպերատորները կարողանան սկրիպտի ռեեստրի աուդիտներ առանց դիպչելու
ցածր մակարդակի API-ներ:
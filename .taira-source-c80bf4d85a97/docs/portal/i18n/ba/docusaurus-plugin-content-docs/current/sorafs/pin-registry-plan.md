---
id: pin-registry-plan
lang: ba
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

:::иҫкәртергә канонлы сығанаҡ
::: 1990 й.

# I18NT000000013X булавка теркәүен тормошҡа ашырыу планы (SF-4)

SF-4 тапшыра Pin Registry килешеп һәм ярҙам хеҙмәттәре, улар һаҡлай .
асыҡ йөкләмәләр, штекер сәйәсәтен үтәү, һәм API-ларҙы I18NT000000014X, шлюздар,
һәм оркестристар. Был документ идентификация планын бетон менән киңәйтә
тормошҡа ашырыу бурыстары, сылбырҙа логика, хужа яғынан хеҙмәттәр, ҡоролмалар ҡаплау,
һәм оператив талаптар.

## Масштаб

1. **Берләшкән дәүләт машинаһы**: I18NT00000000003X-билдәләнгән яҙмалар өсөн манифест, псевдоним, псевдоним,
   вариҫы сылбырҙары, һаҡлау эпохалары һәм идара итеү метамағлүмәттәре.
2. **Контракт тормошҡа ашырыу**: детерминистик CRUD операциялары өсөн булавка йәшәү циклы
   (`ReplicationOrder`, `Precommit`, `Completion`, күсерелгән).
3. **Хеҙмәт фасады**: gRPC/REST ос нөктәләре ярҙамында реестр, тип I18NT000000015X
   һәм SDKs ҡуллана, шул иҫәптән pagination һәм аттестация.
4. **Тулы һәм ҡоролмалары**: CLI ярҙамсылары, һынау векторҙары, һәм документация һаҡлау өсөн
   төҫлө, псевдоним һәм идара итеү синхронлаштырыу уратып ала.
5. **Телеметрия & опс**: метрика, иҫкәртмәләр, һәм runbooks өсөн реестр һаулығы.

## Мәғлүмәттәр моделе

### Ядро яҙмалары (I18NT000000004X)

| Струк | Тасуирлама | Яландар |
|-------|-------------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Карталар псевдоним -> асыҡ CID. | I18NI000000035X, I18NI000000036X, `bound_at`, I18NI000000038X. |
| `ReplicationOrderV1` | Инструкция өсөн провайдерҙар өсөн пенсорный манифест. | I18NI000000040X, `manifest_cid`, I18NI000000042X, `redundancy`, `deadline`, I18NI000000000045X. |
| `ReplicationReceiptV1` | Провайдер таныу. | I18NI000000047X, I18NI000000048X, I18NI000000049X X, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Идара итеү сәйәсәте снимок. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

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

## Фикстуралар & CI

- Fixtures каталогы: I18NI000000084X магазиндарында ҡул ҡуйылған манифест/сәйәхәт/заказ снимоктары тергеҙелә I18NI000000085X.
- CI аҙым: `ci/check_sorafs_fixtures.sh` тергеҙелә снимок һәм уңышһыҙлыҡҡа осрай, әгәр диффтар барлыҡҡа килә, CI ҡорамалдар тура килтереп тота.
- Интеграция һынауҙары (`crates/iroha_core/tests/pin_registry.rs`) бәхетле юлды ғәмәлгә ашырыу плюс-псевдонимдарҙы кире ҡағыу, псевдонимдарҙы раҫлау/һаҡлау һаҡсылары, тап килмәгән chunker ручкалары, реплика-һайлау раҫлау, һәм вариҫ-һаҡсылар (билдәһеҙ/алдан раҫланған/пенсия/үҙенсәлекле күрһәткестәр); ҡарағыҙ I18NI000000088X осраҡтар өсөн ҡаплау реквизиттары.
- Блок һынауҙары хәҙер псевдонимдарҙы раҫлау, һаҡлау һаҡсылары һәм вариҫы тикшерелгән I18NI000000089X; күп-хоп эҙмә-эҙлекле асыҡлау бер тапҡыр дәүләт машинаһы ерҙәре.
- Күҙәтеүсәнлек торбалары ҡулланған ваҡиғалар өсөн алтын JSON.

## Телеметрия & Күҙәтеүсәнлек

Метрика (I18NT000000000X):
- I18NI000000090X
- I18NI000000091X
- I18NI000000092X
- I18NI000000093X
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- I18NI0000000955Х.
- Ғәмәлдәге провайдер телеметрияһы (`torii_sorafs_capacity_*`X, I18NI000000097X) ос-остан-осоу таҡталары өсөн даирәлә ҡала.

Журнал:
- Структуралы I18NT000000011X ваҡиғалар ағымы өсөн идара итеү аудиты (ҡулға алынған?).

Иҫкәртмәләр:
- репликация бойороҡтарын көтөп SLA-нан артып китә.
- Псевдоним < сиге.
- Һаҡлау боҙоуҙар (ваҡытын бөткәнсе яңыртылмаған төйөн).

Приборҙар таҡталары:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` тректар тормош циклы дөйөм, псевдоним яҡтыртыу, артта ҡалған туйындырыу, SLA нисбәте, латентлыҡ ҡаршы ялҡаулыҡ, һәм шылтыратыуҙарҙы тикшерергә заказ биргән ставкалар.

## Ранбуктар & Документация

- Яңыртыу I18NI000000099X X реестр статусын яңыртыуҙы индереү өсөн.
- Оператор етәксеһе: `docs/source/sorafs/runbooks/pin_registry_ops.md` (хәҙер баҫылған) ҡаплау метрикаһы, иҫкәртеү, таратыу, резерв һәм тергеҙеү ағымдары.
- Идара итеү етәксеһе: сәйәсәт параметрҙарын һүрәтләү, раҫлау эш ағымы, бәхәстәр менән эш итеү.
- API һылтанма биттәре өсөн һәр ос нөктәһе (I18NT000000001X docs).

## Зависимость & Секвенирование

1. Тулы раҫлау планы бурыстары (ManifestValidator интеграцияһы).
2. Финаллаштырыу I18NT000000012X схемаһы + сәйәсәт ғәҙәттәгесә.
3. Ҡабул итеү килешүе + хеҙмәте, сым телеметрияһы.
4. Регенерация ҡоролмалары, интеграция люкстарын эшләтеү.
5. Яңыртыу docs/ranbooks һәм юл картаһы әйберҙәрен билдәләү.

Һәр юл картаһы тикшерелгән исемлек элементы буйынса SF-4 был планға һылтанма яһарға тейеш, ҡасан прогресс яһала.
REST фасады хәҙер аттеслы исемлектең ос нөктәләре менән йөк ташый:

- I18NI000000101X һәм I18NI000000102X кире ҡайтарыу маскировкалары менән
  псевдонимдар бәйләүҙәр, репликация тәртибе һәм аттестация объекты алынған
  һуңғы блок хеш.
- I18NI000000103X һәм I18NI000000104X әүҙем фашлау
  псевдоним каталог һәм репликация тәртибе артта ҡалыу менән эҙмә-эҙлекле страница һәм
  статус фильтрҙары.

CLI был шылтыратыуҙарҙы урап ала (`iroha app sorafs pin list`, `pin show`, `alias list`,
I18NI000000108X) шулай итеп, операторҙар сценарий реестр аудиттары теймәйенсә ала
түбән кимәлдәге API-лар.
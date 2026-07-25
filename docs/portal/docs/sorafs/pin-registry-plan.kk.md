---
lang: kk
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

:::ескерту Канондық дереккөз
:::

№ SoraFS Pin тізілімін енгізу жоспары (SF-4)

SF-4 Pin Registry келісім-шартын және сақтайтын қосалқы қызметтерді жеткізеді
манифест міндеттемелері, PIN саясаттарын орындау және API интерфейстерін Torii, шлюздер,
және оркестрлер. Бұл құжат валидация жоспарын бетонмен кеңейтеді
тізбектегі логиканы, хост тарапындағы қызметтерді, құрылғыларды қамтитын іске асыру тапсырмалары,
және операциялық талаптар.

## Ауқым

1. **Тізілімнің күй машинасы**: Norito-анифесттер, бүркеншік аттар,
   жалғастырушы тізбектер, сақтау дәуірлері және басқару метадеректері.
2. **Келісімшартты орындау**: түйреуіштердің өмірлік циклі үшін детерминирленген CRUD операциялары
   (`ReplicationOrder`, `Precommit`, `Completion`, шығару).
3. **Қызмет фасады**: Torii тізілімімен қамтамасыз етілген gRPC/REST соңғы нүктелері
   және SDK пайдаланады, соның ішінде беттеу және аттестация.
4. **Құралдар мен құрылғылар**: CLI көмекшілері, сынақ векторлары және сақталатын құжаттама
   манифесттер, бүркеншік аттар және басқару конверттері синхрондалады.
5. **Телеметрия және амалдар**: тізбе денсаулығына арналған көрсеткіштер, ескертулер және жұмыс кітаптары.

## Деректер үлгісі

### Негізгі жазбалар (Norito)

| Құрылым | Сипаттама | Өрістер |
|--------|-------------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Карталардың бүркеншік аты -> манифест CID. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | Провайдерлерге манифестті бекіту туралы нұсқаулық. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | Провайдердің растауы. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | Басқару саясатының суреті. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

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

## Арматуралар және CI

- Арматуралар каталогы: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` `cargo run -p iroha_core --example gen_pin_snapshot` арқылы қалпына келтірілген қол қойылған манифест/бүркеншік ат/тапсырыс суреттерін сақтайды.
- CI қадамы: `ci/check_sorafs_fixtures.sh` суретті қалпына келтіреді және CI арматураларын туралап сақтай отырып, айырмашылықтар пайда болса, сәтсіз аяқталады.
- Интеграциялық сынақтар (`crates/iroha_core/tests/pin_registry.rs`) бақытты жолды және қайталанатын бүркеншік атты қабылдамауды, бүркеншік атты бекітуді/сақтауды қорғауды, сәйкес келмейтін түйіндерді өңдеуді, көшірмелерді санауды тексеруді және мұрагерді қорғаудың сәтсіздіктерін (белгісіз/алдын ала мақұлданған/өткізілген/өзіндік көрсеткіштер); қамту мәліметтері үшін `register_manifest_rejects_*` жағдайларын қараңыз.
- Бірлік сынақтары енді `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ішіндегі бүркеншік аттың тексеруін, сақтау қорғаушыларын және мұрагерді тексеруді қамтиды; күй машинасы қонғаннан кейін көп септік сабақтастығын анықтау.
- Бақылау құбырлары пайдаланатын оқиғаларға арналған алтын JSON.

## Телеметрия және бақылау мүмкіндігі

Көрсеткіштер (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- Бар провайдердің телеметриясы (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) түпкілікті бақылау тақталары үшін ауқымда қалады.

Журналдар:
- Басқару аудиттеріне арналған құрылымдық Norito оқиғалар ағыны (қол қойылған?).

Ескертулер:
- SLA асатын күтудегі репликация тапсырыстары.
- Бүркеншік аттың мерзімі < шек.
- сақтауды бұзу (манифест мерзімі біткенге дейін жаңартылмаған).

Бақылау тақталары:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` манифесттің өмірлік циклінің қорытындыларын, бүркеншік аттың қамтылуын, артта қалудың қанықтылығын, SLA қатынасын, кідіріс пен баяу қабаттасуларды және қоңырау кезінде қарау үшін өткізіп алған тапсырыс мөлшерлемелерін қадағалайды.

## Runbooks & Documentation

- Тіркеу күйінің жаңартуларын қосу үшін `docs/source/sorafs/migration_ledger.md` жаңартыңыз.
- Оператор нұсқаулығы: `docs/source/sorafs/runbooks/pin_registry_ops.md` (қазір жарияланған) метрика, ескерту, орналастыру, сақтық көшірме жасау және қалпына келтіру ағындарын қамтиды.
- Басқару нұсқаулығы: саясат параметрлерін, бекіту жұмыс процесін, дауларды өңдеуді сипаттаңыз.
- Әрбір соңғы нүктеге арналған API анықтамалық беттері (Docusaurus құжаттары).

## Тәуелділіктер және реттілік

1. Валидация жоспарының тапсырмаларын аяқтаңыз (ManifestValidator интеграциясы).
2. Norito схемасын + саясаттың әдепкі параметрлерін аяқтаңыз.
3. Келісімшарт + қызмет көрсету, сымды телеметрияны жүзеге асыру.
4. Арматураларды қалпына келтіріңіз, интеграциялық жинақтарды іске қосыңыз.
5. Docs/runbooks жаңартыңыз және жол картасы элементтерін аяқталды деп белгілеңіз.

SF-4 астындағы әрбір жол картасының бақылау тізімі тармағында прогреске қол жеткізілген кезде осы жоспарға сілтеме жасау керек.
REST қасбеті қазір расталған листингтің соңғы нүктелерімен жеткізіледі:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` және `GET /v1/sorafs/replication` белсенділікті көрсетеді
  бүркеншік ат каталогы және дәйекті беттеу және репликация тапсырысының артта қалуы
  күй сүзгілері.

CLI бұл қоңырауларды (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`) сондықтан операторлар тізілім аудитін қол тигізбестен сценарий жасай алады.
төменгі деңгейдегі API интерфейстері.
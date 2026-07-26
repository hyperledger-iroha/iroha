---
id: pin-registry-plan
lang: ka
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

:::შენიშვნა კანონიკური წყარო
:::

# SoraFS პინის რეესტრის განხორციელების გეგმა (SF-4)

SF-4 აწვდის Pin Registry კონტრაქტს და დამხმარე სერვისებს, რომლებიც ინახება
გამოავლინეთ ვალდებულებები, განახორციელეთ პინის წესები და გაამჟღავნეთ API-ები Torii, კარიბჭეებისთვის,
და ორკესტრატორები. ეს დოკუმენტი აფართოებს ვალიდაციის გეგმას ბეტონით
განხორციელების ამოცანები, რომელიც მოიცავს ჯაჭვის ლოგიკას, მასპინძლის სერვისებს, მოწყობილობებს,
და ოპერატიული მოთხოვნები.

## სფერო

1. **რეგისტრის სახელმწიფო მანქანა**: Norito-განსაზღვრული ჩანაწერები მანიფესტებისთვის, მეტსახელებისთვის,
   მემკვიდრე ჯაჭვები, შეკავების ეპოქები და მმართველობის მეტამონაცემები.
2. **კონტრაქტის განხორციელება**: დეტერმინისტული CRUD ოპერაციები პინის სასიცოცხლო ციკლისთვის
   (`ReplicationOrder`, `Precommit`, `Completion`, გამოსახლება).
3. **მომსახურების ფასადი**: gRPC/REST ბოლო წერტილები, რომლებიც მხარდაჭერილია რეესტრით, რომელიც Torii
   და SDK-ები მოიხმარენ, მათ შორის პაგინაცია და ატესტაცია.
4. **ინსტრუმენტები და მოწყობილობები **: CLI დამხმარეები, ტესტის ვექტორები და დოკუმენტაცია შესანახად
   მანიფესტები, მეტსახელები და მმართველობის კონვერტები სინქრონულად.
5. **ტელემეტრია და ოპერაციები**: მეტრიკა, გაფრთხილებები და წიგნები რეესტრის ჯანმრთელობისთვის.

## მონაცემთა მოდელი

### ძირითადი ჩანაწერები (Norito)

| სტრუქტურა | აღწერა | ველები |
|--------|------------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Maps alias -> manifest CID. | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`. |
| `ReplicationOrderV1` | ინსტრუქცია პროვაიდერებისთვის, რომ დაამაგრონ მანიფესტი. | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`. |
| `ReplicationReceiptV1` | პროვაიდერის აღიარება. | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`. |
| `ManifestPolicyV1` | მმართველობის პოლიტიკის სურათი. | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`. |

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

## მოწყობილობები და CI

- მოწყობილობების დირექტორია: `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` ინახავს ხელმოწერილი მანიფესტის/ალიასი/შეკვეთის სნეპშოტებს, რომლებიც რეგენერირებულია `cargo run -p iroha_core --example gen_pin_snapshot`-ის მიერ.
- CI ნაბიჯი: `ci/check_sorafs_fixtures.sh` აღადგენს სნეპშოტს და წარუმატებელია, თუ განსხვავებები გამოჩნდება, რაც CI მოწყობილობების გასწორებას ინარჩუნებს.
- ინტეგრაციის ტესტები (`crates/iroha_core/tests/pin_registry.rs`) ახორციელებს ბედნიერ გზას, პლუს დუბლიკატის ალიასის უარყოფა, მეტსახელის დამტკიცების/შეკავების მცველები, შეუსაბამო ცუნკერის სახელურები, რეპლიკა-რიცხვის ვალიდაცია და მემკვიდრე-მცველი წარუმატებლობა (უცნობი/წინასწარ დამტკიცებული/გადასული/თვითმაჩვენებლები); დაფარვის დეტალებისთვის იხილეთ `register_manifest_rejects_*` ქეისები.
- ერთეულის ტესტები ახლა მოიცავს მეტსახელის ვალიდაციას, შეკავების მცველებს და მემკვიდრეობის შემოწმებას `crates/iroha_core/src/smartcontracts/isi/sorafs.rs`-ში; მრავალ ჰოპ თანმიმდევრობის გამოვლენა მას შემდეგ, რაც სახელმწიფო მანქანა დაეშვება.
- ოქროს JSON მოვლენებისთვის, რომლებიც გამოიყენება დაკვირვებადობის მილსადენებით.

## ტელემეტრია და დაკვირვება

მეტრიკა (Prometheus):
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- არსებული პროვაიდერის ტელემეტრია (`torii_sorafs_capacity_*`, `torii_sorafs_fee_projection_nanos`) რჩება საზღვრებში ბოლოდან ბოლომდე დაფებისთვის.

ჟურნალები:
- სტრუქტურირებული Norito ღონისძიებების ნაკადი მმართველობითი აუდიტისთვის (ხელმოწერილია?).

გაფრთხილებები:
- მოლოდინში რეპლიკაციის ბრძანებები, რომლებიც აღემატება SLA-ს.
- მეტსახელი გასვლის < ბარიერი.
- შეკავების დარღვევები (მანიფესტი არ არის განახლებული ვადის გასვლამდე).

დაფები:
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` ასახავს მანიფესტის სასიცოცხლო ციკლის ჯამებს, მეტსახელის დაფარვას, ნარჩენების გაჯერებას, SLA თანაფარდობას, შეყოვნებას და სუსტ გადაფარვებს და გამოტოვებული შეკვეთის ტარიფებს გამოძახების განხილვისთვის.

## Runbooks & Documentation

- განაახლეთ `docs/source/sorafs/migration_ledger.md` რეესტრის სტატუსის განახლებისთვის.
- ოპერატორის სახელმძღვანელო: `docs/source/sorafs/runbooks/pin_registry_ops.md` (ახლა გამოქვეყნებულია), რომელიც მოიცავს მეტრიკას, გაფრთხილებას, განლაგებას, სარეზერვო და აღდგენის ნაკადებს.
- მმართველობის სახელმძღვანელო: აღწერეთ პოლიტიკის პარამეტრები, დამტკიცების სამუშაო პროცესი, დავების განხილვა.
- API საცნობარო გვერდები თითოეული საბოლოო წერტილისთვის (Docusaurus დოკუმენტები).

## დამოკიდებულებები და თანმიმდევრობა

1. დაასრულეთ ვალიდაციის გეგმის ამოცანები (ManifestValidator ინტეგრაცია).
2. დაასრულეთ Norito სქემა + პოლიტიკის ნაგულისხმევი პარამეტრები.
3. განახორციელოს ხელშეკრულება + მომსახურება, მავთულის ტელემეტრია.
4. განაახლეთ მოწყობილობები, გაუშვით ინტეგრაციის კომპლექტები.
5. განაახლეთ Docs/Runbooks და მონიშნეთ საგზაო რუქის ელემენტები დასრულებულად.

ყოველი საგზაო რუქის საკონტროლო სიის პუნქტი SF-4-ში უნდა მიუთითებდეს ამ გეგმაზე, როდესაც მიიღწევა პროგრესი.
REST ფასადი ახლა იგზავნება დამოწმებული ჩამონათვალის ბოლო წერტილებით:

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` და `GET /v1/sorafs/replication` ამჟღავნებს აქტიურს
  ფსევდონიმების კატალოგი და რეპლიკაციის შეკვეთის ჩანაწერი თანმიმდევრული პაგინაცია და
  სტატუსის ფილტრები.

CLI აფუჭებს ამ ზარებს (`iroha app sorafs pin list`, `pin show`, `alias list`,
`replication list`), რათა ოპერატორებმა შეძლონ რეესტრის აუდიტის დაწერა შეხების გარეშე
ქვედა დონის API-ები.
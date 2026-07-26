---
lang: am
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

::: ማስታወሻ ቀኖናዊ ምንጭ
::

# SoraFS ፒን መዝገብ ቤት ትግበራ እቅድ (SF-4)

SF-4 የፒን መዝገብ ቤት ውል እና የሚያከማቹ ደጋፊ አገልግሎቶችን ያቀርባል
ቃል ኪዳኖችን ማሳየት፣ የፒን ፖሊሲዎችን ማስፈጸም እና ኤፒአይዎችን ለTorii፣ መግቢያ መንገዶች፣
እና ኦርኬስትራዎች. ይህ ሰነድ የማረጋገጫ እቅድን በኮንክሪት ያሰፋዋል
የመተግበር ተግባራት፣ በሰንሰለት ላይ ያለውን አመክንዮ የሚሸፍን ፣ የአስተናጋጅ-ጎን አገልግሎቶች ፣ የቤት ዕቃዎች ፣
እና የአሠራር መስፈርቶች.

## ወሰን

1. ** የመመዝገቢያ ግዛት ማሽን ***: Norito-የተገለጹ መዝገቦች ለገለጻዎች, ተለዋጭ ስሞች,
   ተተኪ ሰንሰለቶች፣ የማቆየት ዘመን እና የአስተዳደር ዲበ ውሂብ።
2. **የኮንትራት ትግበራ**፡ ለፒን የህይወት ኡደት የሚወስኑ የCRUD ስራዎች
   (`ReplicationOrder`፣ `Precommit`፣ `Completion`፣ ማስወጣት)።
3. **የአገልግሎት ፊት ለፊት**፡ gRPC/REST በ Torii በመዝገቡ የተደገፈ የመጨረሻ ነጥቦች
   እና ኤስዲኬዎች በገጽ መግለጫ እና ማረጋገጫን ጨምሮ ይበላሉ።
4. ** መሳሪያዎች እና እቃዎች ***: CLI አጋዥዎች፣ ቬክተሮችን ይፈትሹ እና የሚቀመጡ ሰነዶች
   ይገለጻል፣ ተለዋጭ ስሞች እና የአስተዳደር ኤንቨሎፖች በማመሳሰል።
5. **ቴሌሜትሪ እና ኦፕስ**፡ መለኪያዎች፣ ማንቂያዎች እና የሩጫ መጽሐፍት ለመመዝገቢያ ጤና።

## የውሂብ ሞዴል

### ኮር መዛግብት (Norito)

| መዋቅር | መግለጫ | መስኮች |
|--------|-------------|----|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | ካርታዎች ተለዋጭ ስም -> አንጸባራቂ CID። | `alias`፣ `manifest_cid`፣ `bound_at`፣ `expiry_epoch`። |
| `ReplicationOrderV1` | አንጸባራቂን ለመሰካት አቅራቢዎች መመሪያ። | `order_id`፣ `manifest_cid`፣ `providers`፣ `redundancy`፣ `deadline`፣ `policy_hash`። |
| `ReplicationReceiptV1` | የአቅራቢ እውቅና. | `order_id`፣ `provider_id`፣ `status`፣ `timestamp`፣ `por_sample_digest`። |
| `ManifestPolicyV1` | የአስተዳደር ፖሊሲ ቅጽበታዊ ገጽ እይታ። | `min_replicas`፣ `max_retention_epochs`፣ `allowed_profiles`፣ `pin_fee_basis_points`። |

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

## ቋሚዎች እና CI

- የቋሚዎች ማውጫ፡- `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` መደብሮች የተፈረሙ የማኒፌክት/ተለዋጭ ስም/በ`cargo run -p iroha_core --example gen_pin_snapshot` የታደሱ ቅጽበታዊ ገጽ እይታዎች።
- CI ደረጃ፡- I18NI0000086X ቅጽበተ-ፎቶውን ያድሳል እና ልዩነቶች ከታዩ አይሳካም ፣የ CI መጫዎቻዎች የተስተካከሉ እንዲሆኑ ያደርጋል።
- የውህደት ፈተናዎች (`crates/iroha_core/tests/pin_registry.rs`) የደስታ መንገድን እና የተባዛ-ተለዋጭ ስም አለመቀበልን፣ ቅጽል ስም ማፅደቅ/ማቆያ ጠባቂዎች፣ ያልተዛመደ ሹንከር እጀታዎች፣ ቅጂ-ቆጠራ ማረጋገጥ እና ተተኪ-ጠባቂ ውድቀቶች (ያልታወቀ/ቅድመ-ፀደቀ/ጡረታ/ራስ ጠቋሚዎች)። ለሽፋን ዝርዝሮች `register_manifest_rejects_*` ጉዳዮችን ይመልከቱ።
- የዩኒት ሙከራዎች አሁን በ `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` ውስጥ ተለዋጭ ማረጋገጫን ፣ ማቆያ ጠባቂዎችን እና ተተኪ ቼኮችን ይሸፍናሉ ። ባለብዙ ሆፕ ተከታይ ማወቂያ አንዴ የመንግስት ማሽን መሬት።
- ወርቃማው JSON በተመልካችነት ቧንቧዎች ለሚጠቀሙባቸው ዝግጅቶች።

## ቴሌሜትሪ እና ታዛቢነት

መለኪያዎች (Prometheus)
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- ነባር አቅራቢ ቴሌሜትሪ (I18NI0000096X፣ `torii_sorafs_fee_projection_nanos`) ከጫፍ እስከ ጫፍ ዳሽቦርዶች ወሰን ውስጥ ይቆያል።

መዝገቦች፡
- የተዋቀረ I18NT0000011X የክስተት ዥረት ለአስተዳደር ኦዲቶች (የተፈረመ?)።

ማንቂያዎች፡
- ከ SLA በላይ የሆኑ የማባዛት ትዕዛዞች በመጠባበቅ ላይ።
- ተለዋጭ ስም ጊዜው ያበቃል < ደፍ።
- የማቆየት ጥሰቶች (ማለቂያው ከማለቁ በፊት ያልታደሰ መግለጫ)።

ዳሽቦርዶች፡
- Grafana JSON I18NI0000098X ትራኮች የህይወት ኡደት ድምርን፣ተለዋጭ ሽፋን፣የኋላ ሎግ ሙሌት፣ SLA ሬሾ፣የዘገየ እና የላላ ተደራቢዎች፣እና ለጥሪ ግምገማ ያመለጡ የትዕዛዝ መጠኖች።

## Runbooks & Documentation

- የመመዝገቢያ ሁኔታ ዝመናዎችን ለማካተት I18NI00000099ን ያዘምኑ።
- የኦፕሬተር መመሪያ፡ `docs/source/sorafs/runbooks/pin_registry_ops.md` (አሁን የታተመ) መለኪያዎችን፣ ማንቂያዎችን፣ ማሰማራትን፣ ምትኬን እና የመልሶ ማግኛ ፍሰቶችን የሚሸፍን ነው።
- የአስተዳደር መመሪያ፡ የፖሊሲ መለኪያዎችን ይግለጹ፣ የተፈቀደ የስራ ሂደት፣ የክርክር አያያዝ።
- ለእያንዳንዱ የመጨረሻ ነጥብ (Docusaurus ሰነዶች) የኤፒአይ ማመሳከሪያ ገጾች።

## ጥገኛ እና ቅደም ተከተል

1. የተሟላ የማረጋገጫ እቅድ ተግባራት (ManiifestValidator ውህደት).
2. Norito schema + የፖሊሲ ነባሪዎችን ያጠናቅቁ።
3. ውል + አገልግሎት, ሽቦ ቴሌሜትሪ ተግባራዊ ያድርጉ.
4. የቤት እቃዎችን እንደገና ማደስ, የመዋሃድ ስብስቦችን ያሂዱ.
5. ሰነዶች/ runbooks ያዘምኑ እና የመንገድ ካርታ እቃዎች እንደተጠናቀቁ ምልክት ያድርጉ።

በSF-4 ስር ያለው እያንዳንዱ የፍኖተ ካርታ ማረጋገጫ ዝርዝር ይህ እቅድ መሻሻል ሲደረግ ማጣቀስ አለበት።
የREST የፊት ገጽታ አሁን የተረጋገጡ የዝርዝር የመጨረሻ ነጥቦችን ይላካል፡

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` እና `GET /v1/sorafs/replication` ንቁውን ያጋልጣሉ
  ቅጽል ካታሎግ እና የማባዛት ቅደም ተከተል የኋላ ሎግ ወጥነት ባለው ገጽ እና
  የሁኔታ ማጣሪያዎች.

CLI እነዚህን ጥሪዎች ያጠቃልላል (`iroha app sorafs pin list`፣ `pin show`፣ `alias list`፣
`replication list`) ስለዚህ ኦፕሬተሮች የመዝገብ ኦዲቶችን ሳይነኩ መፃፍ ይችላሉ
ዝቅተኛ ደረጃ ኤፒአይዎች።
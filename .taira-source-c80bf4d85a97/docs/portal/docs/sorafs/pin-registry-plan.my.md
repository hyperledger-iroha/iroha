---
lang: my
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

::: Canonical Source ကို သတိပြုပါ။
:::

#SoraFS Pin Registry Implementation Plan (SF-4)

SF-4 သည် Pin Registry စာချုပ်နှင့် သိမ်းဆည်းသော ဝန်ဆောင်မှုများကို ပံ့ပိုးပေးသည်။
ကတိကဝတ်များကို ထင်ရှားစွာပြပါ၊ ပင်နံပါတ်မူဝါဒများကို ကျင့်သုံးရန်နှင့် APIs များကို Torii၊ ဂိတ်ဝေးများ၊
တီးမှုတ်သူများ၊ ဤစာတမ်းသည် အတည်ပြုခြင်းအစီအစဥ်အား တိကျခိုင်မာစွာဖြင့် ချဲ့ထွင်ထားသည်။
အကောင်အထည်ဖော်ခြင်းလုပ်ငန်းများ၊ ကွင်းဆက်ယုတ္တိ၊ အိမ်ရှင်ဘက်ဆိုင်ရာ ဝန်ဆောင်မှုများ၊ ပြင်ဆင်မှုများ၊
နှင့် လုပ်ငန်းဆောင်ရွက်မှု လိုအပ်ချက်များ။

## နယ်ပယ်

1. **Registry state machine**- Norito သည် manifests၊ aliases အတွက် သတ်မှတ်ထားသော မှတ်တမ်းများ၊
   ဆက်ခံသည့် ကွင်းဆက်များ၊ ထိန်းသိမ်းထားသည့် ခေတ်များနှင့် အုပ်ချုပ်မှု မက်တာဒေတာ။
2. **စာချုပ်အကောင်အထည်ဖော်ခြင်း**- pin lifecycle အတွက် အဆုံးအဖြတ်ပေးသော CRUD လုပ်ဆောင်ချက်များ
   (`ReplicationOrder`၊ `Precommit`၊ `Completion`၊ နှင်ထုတ်ခြင်း)။
3. **ဝန်ဆောင်မှုမျက်နှာစာ**- Torii ၏ မှတ်ပုံတင်မှုမှ ကျောထောက်နောက်ခံပြုထားသော gRPC/REST အဆုံးမှတ်များ
   pagination နှင့် အထောက်အထား အပါအဝင် SDK များကို စားသုံးပါသည်။
4. **Tooling & fixtures**- CLI helpers, test vectors, and documentation to keep
   ထင်ရှားသော၊ နာမည်တူနှင့် အုပ်ချုပ်မှုစာအိတ်များကို ထပ်တူပြုထားသည်။
5. **Telemetry & ops**- စာရင်းသွင်းကျန်းမာရေးအတွက် မက်ထရစ်များ၊ သတိပေးချက်များ၊ နှင့် runbooks။

## ဒေတာမော်ဒယ်

### Core Records (Norito)

| ဖွဲ့စည်းပုံ | ဖော်ပြချက် | လယ်ကွင်းများ |
|--------|----------------|--------|
| `PinManifestRecord` | Chain-authoritative manifest lifecycle entry. The envelope digest and exact 36-byte CIDv1/dag-cbor/BLAKE3-256 content root are distinct commitments. | `digest`, `root_cid`, `chunker`, `chunk_digest_sha3_256`, `por_root`, `content_length`, `policy`, `submitted_by`, `submitted_epoch`, `alias`, `successor_of`, `metadata`, `status`, `retirement_reason`, `council_envelope_digest`, `pin_fee_payment`. |
| `PinManifestFinalizedRecordV1` | Immutable read result binding one native manifest record to the finalized block used for the query. | `finalized_cursor` (`height`, `block_hash`), `manifest`. |
| `AliasBindingV1` | Maps alias -> ထင်ရှားသော CID။ | `alias`, `manifest_cid`, `bound_at`, `expiry_epoch`။ |
| `ReplicationOrderV1` | မန်နီးဖက်စ်ကို ထိုးရန် ပံ့ပိုးပေးသူများအတွက် ညွှန်ကြားချက်။ | `order_id`, `manifest_cid`, `providers`, `redundancy`, `deadline`, `policy_hash`။ |
| `ReplicationReceiptV1` | ဝန်ဆောင်မှုပေးသူ အသိအမှတ်ပြုမှု။ | `order_id`, `provider_id`, `status`, `timestamp`, `por_sample_digest`။ |
| `ManifestPolicyV1` | အုပ်ချုပ်မှုမူဝါဒ လျှပ်တစ်ပြက်။ | `min_replicas`, `max_retention_epochs`, `allowed_profiles`, `pin_fee_basis_points`။ |

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

## တန်ဆာပလာများနှင့် CI

- Fixtures directory- `crates/iroha_core/tests/fixtures/sorafs_pin_registry/` စတိုးဆိုင်များမှ လက်မှတ်ရေးထိုးထားသော manifest/alias/order snapshots များကို `cargo run -p iroha_core --example gen_pin_snapshot` မှ ပြန်လည်ထုတ်ပေးပါသည်။
- CI အဆင့်- `ci/check_sorafs_fixtures.sh` သည် လျှပ်တစ်ပြက်ရိုက်ချက်အား ပြန်လည်ထုတ်ပေးပြီး CI ပစ္စည်းများကို ချိန်ညှိထားခြင်းဖြင့် ကွဲပြားမှုများပေါ်လာပါက မအောင်မြင်ပါ။
- ပေါင်းစည်းခြင်းစမ်းသပ်မှုများ (`crates/iroha_core/tests/pin_registry.rs`) သည် ပျော်ရွှင်သောလမ်းကြောင်းနှင့် ထပ်တူထပ်တူသော-အမည်တူများကို ပယ်ချခြင်း၊ အမည်တူခွင့်ပြုချက်/ထိန်းသိမ်းခြင်းအစောင့်များ၊ ကိုက်ညီမှုမရှိသော chunker လက်ကိုင်များ၊ ပုံစံတူ-ရေတွက်မှုအတည်ပြုချက်နှင့် ဆက်ခံ-စောင့်ကြပ်မှုကျရှုံးမှုများ (အမည်မသိ/ကြိုတင်အတည်ပြုထားသော/အငြိမ်းစား/အငြိမ်းစား/ကိုယ်ကိုတိုင်ညွှန်ပြချက်များ)၊ လွှမ်းခြုံမှုအသေးစိတ်အတွက် `register_manifest_rejects_*` အမှုများကို ကြည့်ပါ။
- ယခု ယူနစ်စမ်းသပ်မှုများသည် `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` တွင် နာမည်အရင်းအတည်ပြုခြင်း၊ ထိန်းသိမ်းစောင့်ကြပ်ခြင်းနှင့် ဆက်ခံခြင်းစစ်ဆေးမှုများကို အကျုံးဝင်ပါသည်။ နိုင်ငံပိုင်စက်များ ဆင်းသက်သည်နှင့် တစ်ပြိုင်နက် multi-hop succession detection။
- မြင်နိုင်စွမ်းရှိသော ပိုက်လိုင်းများအသုံးပြုသည့် ဖြစ်ရပ်များအတွက် Golden JSON။

## Telemetry & Observability

မက်ထရစ်များ (Prometheus)
- `torii_sorafs_registry_manifests_total{status="pending|approved|retired"}`
- `torii_sorafs_registry_aliases_total`
- `torii_sorafs_registry_orders_total{status="pending|completed|expired"}`
- `torii_sorafs_replication_sla_total{outcome="met|missed|pending"}`
- `torii_sorafs_replication_completion_latency_epochs{stat="avg|p95|max|count"}`
- `torii_sorafs_replication_deadline_slack_epochs{stat="avg|p95|max|count"}`
- လက်ရှိပံ့ပိုးပေးသူ တယ်လီမီတာ (`torii_sorafs_capacity_*`၊ `torii_sorafs_fee_projection_nanos`) သည် အဆုံးမှအဆုံးအထိ ဒက်ရှ်ဘုတ်များအတွက် နယ်ပယ်တွင် ကျန်ရှိနေပါသည်။

မှတ်တမ်းများ-
- အုပ်ချုပ်မှုစာရင်းစစ်များအတွက် ဖွဲ့စည်းတည်ဆောက်ထားသော Norito ဖြစ်ရပ်စီးကြောင်း (လက်မှတ်ထိုး?)။

သတိပေးချက်များ-
- SLA ထက်ကျော်လွန်သော ပုံတူကူးယူမှုများကို ဆိုင်းငံ့ထားသည်။
- Alias ​​သက်တမ်းကုန် < သတ်မှတ်ချက်။
- ထိန်းသိမ်းထားသော ချိုးဖောက်မှုများ (သက်တမ်းမကုန်မီ သက်တမ်းမတိုးမီ ဖော်ပြချက်)။

ဒက်ရှ်ဘုတ်များ-
- Grafana JSON `docs/source/grafana_sorafs_pin_registry.json` သည် ဘဝလည်ပတ်မှုစုစုပေါင်း၊ အမည်များ အကျုံးဝင်မှု၊ မှတ်တမ်းများ ပြည့်ဝမှု၊ SLA အချိုး၊ latency နှင့် slack ထပ်ဆင့်မှုများနှင့် လွတ်သွားသော အမှာစာနှုန်းများကို မှတ်သားထားသည်။

## Runbooks & Documentation

- မှတ်ပုံတင်ခြင်းအခြေအနေမွမ်းမံမှုများပါဝင်ရန် `docs/source/sorafs/migration_ledger.md` ကို အပ်ဒိတ်လုပ်ပါ။
- အော်ပရေတာလမ်းညွှန်- `docs/source/sorafs/runbooks/pin_registry_ops.md` (ယခုထုတ်ဝေသည်) မက်ထရစ်များ၊ သတိပေးချက်၊ အသုံးချမှု၊ အရန်သိမ်းခြင်းနှင့် ပြန်လည်ရယူခြင်းစီးဆင်းမှုများကို အကျုံးဝင်သည်။
- အုပ်ချုပ်မှုလမ်းညွှန်- မူဝါဒသတ်မှတ်ချက်များ၊ ခွင့်ပြုချက်လုပ်ငန်းအသွားအလာ၊ အငြင်းပွားမှုကိုင်တွယ်ပုံကို ဖော်ပြပါ။
- အဆုံးမှတ်တစ်ခုစီအတွက် API ရည်ညွှန်းစာမျက်နှာများ (Docusaurus docs)။

## မှီခိုမှုနှင့် စီစစ်ခြင်း။

1. တရားဝင်အတည်ပြုခြင်းအစီအစဥ်များကို အပြီးသတ်ပါ (ManifestValidator ပေါင်းစပ်မှု)။
2. Norito schema + မူဝါဒ ပုံသေများကို အပြီးသတ်ပါ။
3. စာချုပ် + ဝန်ဆောင်မှု၊ ဝါယာကြိုး တယ်လီမီတာကို အကောင်အထည်ဖော်ပါ။
4. ပြင်ဆင်မှုများ ပြန်ထုတ်ပါ၊ ပေါင်းစည်းမှုအစုံများကို ဖွင့်ပါ။
5. docs/runbooks များကို အပ်ဒိတ်လုပ်ပြီး လမ်းပြမြေပုံပါ အရာများ ပြီးမြောက်ကြောင်း အမှတ်အသားပြုပါ။

SF-4 အောက်တွင် လမ်းပြမြေပုံ စစ်ဆေးရေးစာရင်း အကြောင်းအရာတစ်ခုစီသည် တိုးတက်မှုလုပ်ဆောင်သည့်အခါ ဤအစီအစဉ်ကို ကိုးကားသင့်သည်။
REST façade သည် ယခုအခါ အတည်ပြုစာရင်းဝင်သည့် အဆုံးမှတ်များနှင့်အတူ ပို့ဆောင်ပေးသည်-

- `GET /v1/sorafs/pin` returns the attested manifest catalogue.
- `GET /v1/sorafs/pin/{digest_hex}` returns exact `PinManifestFinalizedRecordV1` JSON with `finalized_cursor.height`, `finalized_cursor.block_hash`, and native `manifest`.
- `GET /v1/sorafs/aliases` နှင့် `GET /v1/sorafs/replication` တက်ကြွမှုကို ဖော်ထုတ်ပါ
  တသမတ်တည်း pagination နှင့် alias catalog နှင့် replication order backlog
  အခြေအနေ စစ်ထုတ်မှုများ။

CLI သည် ဤခေါ်ဆိုမှုများကို အဆုံးသတ်သည် (`iroha app sorafs pin list`၊ `pin show`၊ `alias list`၊
`replication list`) ထို့ကြောင့် အော်ပရေတာများသည် script registry စစ်ဆေးမှုများကို မထိဘဲ လုပ်နိုင်သည်
အောက်အဆင့် API များ။
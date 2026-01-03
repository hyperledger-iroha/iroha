---
lang: ur
direction: rtl
source: docs/source/nexus_lanes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 94891050512eaf78f4c0381c0facbeed445a7e7323297070ae537e4d38ca7fe4
source_last_modified: "2025-12-13T05:07:11.953030+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/nexus_lanes.md -->

# Nexus lane ماڈل اور WSV پارٹیشننگ

> **اسٹیٹس:** NX-1 ڈیلیوریبل - lanes کی taxonomy، configuration کی geometry اور storage layout نفاذ کے لئے تیار ہیں۔  
> **مالکان:** Nexus Core WG, Governance WG  
> **متعلقہ roadmap آئٹم:** NX-1

یہ دستاویز Nexus کی multi-lane consensus layer کے لئے ہدفی architecture محفوظ کرتی ہے۔ مقصد ایک واحد deterministic world state پیدا کرنا ہے جبکہ انفرادی data spaces (lanes) کو عوامی یا نجی validator sets کے ساتھ isolated workloads چلانے کی اجازت دی جاتی ہے۔

> **Cross-lane proofs:** یہ نوٹ geometry اور storage پر فوکس کرتا ہے۔ ہر lane کے settlement commitments، relay pipeline، اور roadmap **NX-4** کے لئے درکار merge-ledger proofs کو [nexus_cross_lane.md](nexus_cross_lane.md) میں بیان کیا گیا ہے۔

## Concepts

- **Lane:** Nexus ledger کا logical shard جس کا اپنا validator set اور execution backlog ہوتا ہے۔ اسے stable `LaneId` سے شناخت کیا جاتا ہے۔
- **Data Space:** governance bucket جو ایک یا زیادہ lanes کو گروپ کرتا ہے اور compliance، routing اور settlement policies شیئر کرتا ہے۔ ہر dataspace `fault_tolerance (f)` بھی declare کرتا ہے جس سے lane-relay committees کا سائز (`3f+1`) طے ہوتا ہے۔
- **Lane Manifest:** governance-controlled metadata جو validators، DA policy، gas token، settlement rules اور routing permissions بیان کرتا ہے۔
- **Global Commitment:** ایسا proof جو lane جاری کرتا ہے اور نئے state roots، settlement data اور اختیاری cross-lane transfers کا خلاصہ دیتا ہے۔ global NPoS ring commitments کو order کرتا ہے۔

## Lane taxonomy

Lane types اپنی visibility، governance surface اور settlement hooks کو canonical انداز میں بیان کرتی ہیں۔ configuration geometry (`LaneConfig`) یہ attributes capture کرتی ہے تاکہ nodes، SDKs اور tooling layout کو bespoke logic کے بغیر سمجھ سکیں۔

| Lane type | Visibility | Validator membership | WSV exposure | Default governance | Settlement policy | Typical use |
|-----------|------------|----------------------|--------------|--------------------|-------------------|-------------|
| `default_public` | public | Permissionless (global stake) | Full state replica | SORA Parliament | `xor_global` | Baseline public ledger |
| `public_custom` | public | Permissionless يا stake-gated | Full state replica | Stake weighted module | `xor_lane_weighted` | High-throughput public applications |
| `private_permissioned` | restricted | Fixed validator set (governance approved) | Commitments & proofs | Federated council | `xor_hosted_custody` | CBDC, consortium workloads |
| `hybrid_confidential` | restricted | Mixed membership; wraps ZK proofs | Commitments + selective disclosure | Programmable money module | `xor_dual_fund` | Privacy-preserving programmable money |

تمام lane types کو اعلان کرنا ہوگا:

- Dataspace alias - readable grouping جو compliance policies کو باندھتا ہے۔
- Governance handle - identifier جو `Nexus.governance.modules` کے ذریعے resolve ہوتا ہے۔
- Settlement handle - identifier جو settlement router XOR buffers سے debit کرنے کے لئے consume کرتا ہے۔
- اختیاری telemetria metadata (description, contact, business domain) جو `/status` اور dashboards پر ظاہر ہوتی ہے۔

## Lane configuration geometry (`LaneConfig`)

`LaneConfig` validated lane catalog سے نکلنے والی runtime geometry ہے۔ یہ governance manifests کو replace نہیں کرتا؛ بلکہ ہر configured lane کے لئے deterministic storage identifiers اور telemetria hints فراہم کرتا ہے۔

```text
LaneConfigEntry {
    lane_id: LaneId,           // stable identifier
    alias: String,             // human-readable alias
    slug: String,              // sanitised alias for file/metric keys
    kura_segment: String,      // Kura segment directory: lane_{id:03}_{slug}
    merge_segment: String,     // Merge-ledger segment: lane_{id:03}_merge
    key_prefix: [u8; 4],       // Big-endian LaneId prefix for WSV key spaces
    shard_id: ShardId,         // WSV/Kura shard binding (defaults to lane_id)
    visibility: LaneVisibility,// public vs restricted lanes
    storage_profile: LaneStorageProfile,
    proof_scheme: DaProofScheme,// DA proof policy (merkle_sha256 default)
}
```

- `LaneConfig::from_catalog` geometry کو ہر config load پر دوبارہ compute کرتا ہے (`State::set_nexus`).
- Aliases کو lowercase slugs میں sanitize کیا جاتا ہے؛ مسلسل non-alphanumeric characters `_` میں collapse ہو جاتے ہیں۔ اگر alias سے خالی slug بنے تو `lane{id}` استعمال ہوتا ہے۔
- Key prefixes WSV میں per-lane key ranges کو جدا رکھتے ہیں، چاہے backend shared ہو۔
- `shard_id` catalog metadata key `da_shard_id` سے derive ہوتا ہے (default `lane_id`) اور persisted shard cursor journal کو drive کرتا ہے تاکہ DA replay restarts/resharding کے بعد deterministic رہے۔
- Kura segment names hosts کے درمیان deterministic ہیں؛ auditors segment directories اور manifests کو bespoke tooling کے بغیر cross-check کر سکتے ہیں۔
- Merge segments (`lane_{id:03}_merge`) میں latest merge-hint roots اور global state commitments محفوظ ہوتے ہیں۔
- جب governance کسی lane alias کو rename کرتی ہے تو nodes `blocks/lane_{id:03}_{slug}` directories (اور tiered snapshots) کو خودکار طور پر relabel کرتے ہیں تاکہ auditors کو canonical slug نظر آئے اور manual cleanup نہ ہو۔

## World-state partitioning

- Nexus کا logical world state per-lane state spaces کا union ہے۔ public lanes full state persist کرتی ہیں؛ private/confidential lanes Merkle/commitment roots کو merge ledger میں export کرتی ہیں۔
- MV storage ہر key کو `LaneConfigEntry::key_prefix` کے 4-byte lane prefix کے ساتھ prefix کرتا ہے، جس سے keys `[00 00 00 01] ++ PackedKey` جیسی بنتی ہیں۔
- Shared tables (accounts, assets, triggers, governance records) اس طرح lane prefix کے حساب سے گروپڈ entries رکھتی ہیں، جس سے range scans deterministic رہتے ہیں۔
- Merge-ledger metadata اسی layout کو mirror کرتا ہے: ہر lane `lane_{id:03}_merge` میں merge-hint roots اور reduced global state roots لکھتی ہے، جس سے lane retire ہونے پر targeted retention/eviction ممکن ہوتی ہے۔
- Cross-lane indexes (account aliases, asset registries, governance manifests) explicit `(LaneId, DataSpaceId)` pairs store کرتے ہیں۔ یہ indexes shared column families میں رہتے ہیں مگر lane prefix اور explicit dataspace ids استعمال کرتے ہیں تاکہ lookups deterministic رہیں۔
- Merge workflow public data اور private commitments کو `(lane_id, dataspace_id, height, state_root, settlement_root, proof_root)` tuples سے combine کرتا ہے جو merge-ledger entries سے derive ہوتے ہیں۔

## Kura اور WSV partitioning

- **Kura segments**
  - `lane_{id:03}_{slug}` - lane کے لئے primary block segment (blocks, indexes, receipts).
  - `lane_{id:03}_merge` - merge-ledger segment جو reduced state roots اور settlement artefacts ریکارڈ کرتا ہے۔
  - Global segments (consensus evidence, telemetria caches) shared رہتے ہیں کیونکہ وہ lane-neutral ہیں؛ ان کی keys میں lane prefixes شامل نہیں ہوتے۔
- Runtime lane catalog updates دیکھتا ہے: نئی lanes کے block اور merge-ledger directories خودکار طور پر `kura/blocks/` اور `kura/merge_ledger/` کے تحت provision ہوتے ہیں، جبکہ retired lanes `kura/retired/{blocks,merge_ledger}/lane_{id:03}_*` میں archive ہوتی ہیں۔
- Tiered-state snapshots اسی lifecycle کو follow کرتے ہیں؛ ہر lane `cold_store_root/lanes/lane_{id:03}_{slug}` میں لکھتی ہے اور retirements directory tree کو `cold_store_root/retired/lanes/` میں منتقل کر دیتی ہیں۔
- **Key prefixes** - `LaneId` سے نکلا 4-byte prefix ہمیشہ MV encoded keys کے ساتھ prepend ہوتا ہے۔ کوئی host-specific hashing استعمال نہیں ہوتا، لہذا ordering تمام nodes میں ایک جیسی ہے۔
- **Block log layout** - block data, index اور hashes `kura/blocks/lane_{id:03}_{slug}/` کے تحت nest ہوتے ہیں۔ Merge-ledger journals اسی slug کو reuse کرتے ہیں (`kura/merge/lane_{id:03}_{slug}.log`)، جس سے per-lane recovery flows isolate رہتے ہیں۔
- **Retention policy** - public lanes full block bodies retain کرتی ہیں؛ commitment-only lanes checkpoints کے بعد پرانے bodies compact کر سکتی ہیں کیونکہ commitments authoritative ہیں۔ Confidential lanes ciphertext journals dedicated segments میں رکھتی ہیں تاکہ دوسرے workloads block نہ ہوں۔
- **Tooling** - `cargo xtask nexus-lane-maintenance --config <path> [--compact-retired]` `<store>/blocks` اور `<store>/merge_ledger` کو derived `LaneConfig` کے ساتھ inspect کرتا ہے، active vs retired segments report کرتا ہے اور retired directories/logs کو `<store>/retired/...` کے تحت archive کرتا ہے تاکہ evidence deterministic رہے۔ Maintenance utilities (`kagami`, CLI admin commands) کو metrics, Prometheus labels یا Kura segments archive کرتے وقت slugged namespace reuse کرنا چاہیے۔

## Routing اور APIs

- Torii REST/gRPC endpoints optional `lane_id` لیتے ہیں؛ absence کا مطلب `lane_default` ہے۔
- SDKs lane selectors surface کرتے ہیں اور user-friendly aliases کو lane catalog کے ذریعے `LaneId` سے map کرتے ہیں۔
- Routing rules validated catalog پر operate کرتے ہیں اور lane اور dataspace دونوں select کر سکتے ہیں۔ `LaneConfig` dashboards اور logs کے لئے telemetria-friendly aliases دیتا ہے۔

## Settlement اور fees

- ہر lane global validator set کو XOR fees دیتی ہے۔ lanes native gas tokens collect کر سکتی ہیں مگر انہیں commitments کے ساتھ XOR equivalents escrow کرنے ہوں گے۔
- Settlement proofs میں amount، conversion metadata اور escrow proof شامل ہوتا ہے (مثلا global fee vault میں transfer)۔
- Unified settlement router (NX-3) buffers کو انہی lane prefixes کے ساتھ debit کرتا ہے، تاکہ settlement telemetria storage geometry کے ساتھ align رہے۔

## Governance

- Lanes catalog کے ذریعے اپنا governance module declare کرتی ہیں۔ `LaneConfigEntry` اصل alias اور slug لے کر چلتی ہے تاکہ telemetria اور audit trails readable رہیں۔
- Nexus registry signed lane manifests distribute کرتا ہے جن میں `LaneId`, dataspace binding, governance handle, settlement handle اور metadata شامل ہوتا ہے۔
- Runtime-upgrade hooks governance policies (`gov_upgrade_id` default) enforce کرتے رہتے ہیں اور telemetria bridge کے ذریعے diffs log کرتے ہیں (`nexus.config.diff` events)۔
- Lane manifests admin-managed lanes کے لئے dataspace validator pool define کرتے ہیں؛ stake-elected lanes اپنا validator pool public-lane staking records سے derive کرتی ہیں۔

## Telemetria اور status

- `/status` lane aliases، dataspace bindings، governance handles اور settlement profiles دکھاتا ہے، جو catalog اور `LaneConfig` سے derived ہوتے ہیں۔
- Scheduler metrics (`nexus_scheduler_lane_teu_*`) lane aliases/slugs دکھاتی ہیں تاکہ operators backlog اور TEU pressure کو جلد map کر سکیں۔
- `nexus_lane_configured_total` derived lane entries کی تعداد گنتا ہے اور config تبدیلی پر recalculated ہوتا ہے۔ telemetria geometry change پر signed diffs emit کرتی ہے۔
- Dataspace backlog gauges alias/description metadata شامل کرتے ہیں تاکہ operators queue pressure کو business domains سے جوڑ سکیں۔

## Configuration اور Norito types

- `LaneCatalog`, `LaneConfig`, اور `DataSpaceCatalog` `iroha_data_model::nexus` میں رہتے ہیں اور manifests اور SDKs کے لئے Norito-compatible structures فراہم کرتے ہیں۔
- `LaneConfig` `iroha_config::parameters::actual::Nexus` میں رہتا ہے اور catalog سے خودکار طور پر derive ہوتا ہے؛ اسے Norito encoding کی ضرورت نہیں کیونکہ یہ اندرونی runtime helper ہے۔
- User-facing configuration (`iroha_config::parameters::user::Nexus`) declarative lane اور dataspace descriptors کو قبول کرتی ہے؛ parsing اب geometry derive کرتا ہے اور invalid aliases یا duplicate lane ids کو reject کرتا ہے۔
- `DataSpaceMetadata.fault_tolerance` lane-relay committee sizing کنٹرول کرتا ہے؛ committee membership ہر epoch میں dataspace validator pool سے deterministic طور پر sample ہوتا ہے، VRF epoch seed کو `(dataspace_id, lane_id)` کے ساتھ باندھ کر۔

## Outstanding work

- Settlement router updates (NX-3) کو نئی geometry کے ساتھ integrate کریں تاکہ XOR buffer debits اور receipts lane slug کے ساتھ tag ہوں۔
- Merge algorithm (ordering, pruning, conflict detection) finalize کریں اور cross-lane replay کے لئے regression fixtures attach کریں۔
- Whitelists/blacklists اور programmable-money policies کے لئے compliance hooks شامل کریں (NX-12 کے تحت tracked)۔

---

*یہ دستاویز NX-2 سے NX-18 تک کے کام کے ساتھ evolve ہوگی۔ براہ کرم کھلے سوالات roadmap یا governance tracker میں درج کریں۔*

</div>

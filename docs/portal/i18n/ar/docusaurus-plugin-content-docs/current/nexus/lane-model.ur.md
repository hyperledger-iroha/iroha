---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/lane-model.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-lane-model
title: Nexus lane model
description: Sora Nexus کے لئے lanes کی منطقی taxonomy، configuration geometry، اور world-state merge کے اصول۔
---

# Nexus lane model اور WSV partitioning

> **Status:** NX-1 deliverable - lane taxonomy، configuration geometry، اور storage layout نفاذ کے لئے تیار ہیں۔  
> **Owners:** Nexus Core WG, Governance WG  
> **Roadmap reference:** `roadmap.md` میں NX-1

یہ پورٹل صفحہ canonical `docs/source/nexus_lanes.md` brief کی عکاسی کرتا ہے تاکہ Sora Nexus آپریٹرز، SDK owners اور reviewers mono-repo tree میں جائے بغیر lane guidance پڑھ سکیں۔ ہدفی architecture world state کی determinism برقرار رکھتا ہے جبکہ انفرادی data spaces (lanes) کو public یا private validator sets کے ساتھ isolated workloads چلانے دیتا ہے۔

## Concepts

- **Lane:** Nexus ledger کا منطقی shard، اپنے validator set اور execution backlog کے ساتھ۔ اسے ایک مستحکم `LaneId` سے شناخت کیا جاتا ہے۔
- **Data Space:** governance bucket جو ایک یا زیادہ lanes کو گروپ کرتا ہے جو compliance، routing، اور settlement policies شیئر کرتے ہیں۔
- **Lane Manifest:** governance-controlled metadata جو validators، DA policy، gas token، settlement rules، اور routing permissions بیان کرتا ہے۔
- **Global Commitment:** ایک proof جو lane جاری کرتی ہے، نئے state roots، settlement data، اور optional cross-lane transfers کا خلاصہ دیتی ہے۔ global NPoS ring commitments کو ترتیب دیتا ہے۔

## Lane taxonomy

Lane types اپنی visibility، governance surface اور settlement hooks کو canonical طور پر بیان کرتے ہیں۔ configuration geometry (`LaneConfig`) ان attributes کو capture کرتی ہے تاکہ nodes، SDKs اور tooling بغیر bespoke logic کے layout کو سمجھ سکیں۔

| Lane type | Visibility | Validator membership | WSV exposure | Default governance | Settlement policy | Typical use |
|-----------|------------|----------------------|--------------|--------------------|-------------------|-------------|
| `default_public` | public | Permissionless (global stake) | Full state replica | SORA Parliament | `xor_global` | Baseline public ledger |
| `public_custom` | public | Permissionless or stake-gated | Full state replica | Stake weighted module | `xor_lane_weighted` | High-throughput public applications |
| `private_permissioned` | restricted | Fixed validator set (governance approved) | Commitments & proofs | Federated council | `xor_hosted_custody` | CBDC, consortium workloads |
| `hybrid_confidential` | restricted | Mixed membership; wraps ZK proofs | Commitments + selective disclosure | Programmable money module | `xor_dual_fund` | Privacy-preserving programmable money |

تمام lane types کو درج ذیل declare کرنا ہوگا:

- Dataspace alias - انسان کے لئے پڑھنے کے قابل grouping جو compliance policies کو bind کرتی ہے۔
- Governance handle - ایسا identifier جو `Nexus.governance.modules` کے ذریعے resolve ہوتا ہے۔
- Settlement handle - ایسا identifier جو settlement router XOR buffers کو debit کرنے کے لئے استعمال کرتا ہے۔
- Optional telemetry metadata (description، contact، business domain) جو `/status` اور dashboards کے ذریعے ظاہر ہوتے ہیں۔

## Lane configuration geometry (`LaneConfig`)

`LaneConfig` validated lane catalog سے derived runtime geometry ہے۔ یہ governance manifests کو replace نہیں کرتا؛ اس کے بجائے ہر configured lane کے لئے deterministic storage identifiers اور telemetry hints فراہم کرتا ہے۔

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

- `LaneConfig::from_catalog` geometry کو دوبارہ compute کرتا ہے جب configuration load ہو (`State::set_nexus`).
- Aliases کو lowercase slugs میں sanitize کیا جاتا ہے؛ مسلسل non-alphanumeric characters `_` میں collapse ہوتے ہیں۔ اگر alias empty slug دے تو ہم `lane{id}` fallback کرتے ہیں۔
- `shard_id` catalog metadata key `da_shard_id` سے derive ہوتا ہے (default `lane_id`) اور persisted shard cursor journal کو drive کرتا ہے تاکہ restarts/resharding میں DA replay deterministic رہے۔
- Key prefixes یہ یقینی بناتے ہیں کہ WSV per-lane key ranges کو جدا رکھے، چاہے backend shared ہو۔
- Kura segment names hosts کے درمیان deterministic ہوتے ہیں؛ auditors بغیر bespoke tooling کے segment directories اور manifests cross-check کر سکتے ہیں۔
- Merge segments (`lane_{id:03}_merge`) اس lane کے latest merge-hint roots اور global state commitments محفوظ کرتے ہیں۔

## World-state partitioning

- Logical Nexus world state per-lane state spaces کا union ہے۔ Public lanes full state persist کرتی ہیں؛ private/confidential lanes Merkle/commitment roots کو merge ledger میں export کرتی ہیں۔
- MV storage ہر key کو `LaneConfigEntry::key_prefix` کے 4-byte prefix سے prefix کرتا ہے، جس سے `[00 00 00 01] ++ PackedKey` جیسے keys بنتے ہیں۔
- Shared tables (accounts, assets, triggers, governance records) entries کو lane prefix کے حساب سے group کرتی ہیں، جس سے range scans deterministic رہتے ہیں۔
- Merge-ledger metadata اسی layout کو mirror کرتا ہے: ہر lane `lane_{id:03}_merge` میں merge-hint roots اور reduced global state roots لکھتی ہے، جس سے lane retire ہونے پر targeted retention یا eviction ممکن ہوتی ہے۔
- Cross-lane indexes (account aliases, asset registries, governance manifests) explicit lane prefixes store کرتے ہیں تاکہ operators entries جلد reconcile کر سکیں۔
- **Retention policy** - public lanes مکمل block bodies رکھتی ہیں؛ commitment-only lanes checkpoints کے بعد پرانے bodies compact کر سکتی ہیں کیونکہ commitments authoritative ہیں۔ Confidential lanes ciphertext journals کو dedicated segments میں رکھتی ہیں تاکہ دوسرے workloads block نہ ہوں۔
- **Tooling** - maintenance utilities (`kagami`, CLI admin commands) کو metrics expose کرتے، Prometheus labels بناتے یا Kura segments archive کرتے وقت slugged namespace refer کرنا چاہیے۔

## Routing & APIs

- Torii REST/gRPC endpoints optional `lane_id` قبول کرتے ہیں؛ عدم موجودگی `lane_default` کو ظاہر کرتی ہے۔
- SDKs lane selectors فراہم کرتے ہیں اور user-friendly aliases کو lane catalog کے ذریعے `LaneId` سے map کرتے ہیں۔
- Routing rules validated catalog پر operate کرتے ہیں اور lane اور dataspace دونوں منتخب کر سکتے ہیں۔ `LaneConfig` dashboards اور logs کے لئے telemetry-friendly aliases فراہم کرتا ہے۔

## Settlement & fees

- ہر lane global validator set کو XOR fees ادا کرتی ہے۔ Lanes native gas tokens جمع کر سکتی ہیں مگر commitments کے ساتھ XOR equivalents escrow کرنا لازم ہے۔
- Settlement proofs میں amount، conversion metadata، اور escrow proof شامل ہوتے ہیں (مثلا global fee vault کو transfer)۔
- Unified settlement router (NX-3) buffers کو انہی lane prefixes کے ساتھ debit کرتا ہے، لہذا settlement telemetry storage geometry کے ساتھ align ہوتی ہے۔

## Governance

- Lanes اپنی governance module کو catalog کے ذریعے declare کرتی ہیں۔ `LaneConfigEntry` اصل alias اور slug ساتھ رکھتا ہے تاکہ telemetry اور audit trails readable رہیں۔
- Nexus registry signed lane manifests تقسیم کرتا ہے جن میں `LaneId`, dataspace binding, governance handle, settlement handle اور metadata شامل ہوتے ہیں۔
- Runtime-upgrade hooks governance policies (`gov_upgrade_id` بطور default) نافذ کرتے رہتے ہیں اور telemetry bridge (`nexus.config.diff` events) کے ذریعے diffs log کرتے ہیں۔

## Telemetry & status

- `/status` lane aliases، dataspace bindings، governance handles اور settlement profiles کو expose کرتا ہے، جو catalog اور `LaneConfig` سے derive ہوتے ہیں۔
- Scheduler metrics (`nexus_scheduler_lane_teu_*`) lane aliases/slugs دکھاتے ہیں تاکہ operators backlog اور TEU pressure کو جلد map کر سکیں۔
- `nexus_lane_configured_total` derived lane entries کی تعداد شمار کرتا ہے اور configuration تبدیل ہونے پر دوبارہ compute ہوتا ہے۔ Telemetry lane geometry بدلنے پر signed diffs emit کرتی ہے۔
- Dataspace backlog gauges alias/description metadata شامل کرتے ہیں تاکہ operators queue pressure کو business domains سے جوڑ سکیں۔

## Configuration & Norito types

- `LaneCatalog`, `LaneConfig`, اور `DataSpaceCatalog` `iroha_data_model::nexus` میں رہتے ہیں اور manifests و SDKs کے لئے Norito format structures فراہم کرتے ہیں۔
- `LaneConfig` `iroha_config::parameters::actual::Nexus` میں رہتا ہے اور catalog سے خودکار طور پر derive ہوتا ہے؛ اسے Norito encoding کی ضرورت نہیں کیونکہ یہ internal runtime helper ہے۔
- User-facing configuration (`iroha_config::parameters::user::Nexus`) declarative lane اور dataspace descriptors کو قبول کرتی رہتی ہے؛ parsing اب geometry derive کرتا ہے اور invalid aliases یا duplicate lane IDs کو reject کرتا ہے۔

## Outstanding work

- settlement router updates (NX-3) کو نئی geometry کے ساتھ integrate کریں تاکہ XOR buffer debits اور receipts lane slug کے مطابق tag ہوں۔
- Admin tooling کو extend کریں تاکہ column families list ہوں، retired lanes compact ہوں، اور slugged namespace کے ساتھ per-lane block logs inspect ہوں۔
- Merge algorithm (ordering, pruning, conflict detection) finalize کریں اور cross-lane replay کے لئے regression fixtures شامل کریں۔
- Whitelists/blacklists اور programmable-money policies کے لئے compliance hooks شامل کریں (NX-12 میں ٹریک)۔

---

*یہ صفحہ NX-2 سے NX-18 کے اترنے کے ساتھ NX-1 follow-ups کو ٹریک کرتا رہے گا۔ براہ کرم کھلے سوالات `roadmap.md` یا governance tracker میں سامنے لائیں تاکہ پورٹل canonical docs کے ساتھ aligned رہے۔*

---
lang: ur
direction: rtl
source: docs/source/nexus_refactor_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 44b7100fddd377c97dfcab678ce425ec35edfa4a1276f9b6a22aa2c64135a94d
source_last_modified: "2025-11-02T04:40:40.017979+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/nexus_refactor_plan.md -->

# Sora Nexus Ledger ریفیکٹر پلان

یہ دستاویز Sora Nexus Ledger ("Iroha 3") ریفیکٹر کے فوری roadmap کو محفوظ کرتی ہے۔ یہ موجودہ
repo layout اور genesis/WSV bookkeeping، Sumeragi consensus، smart-contract triggers، snapshot
queries، pointer-ABI host bindings اور Norito codecs میں دیکھی گئی regressions کو ظاہر کرتی ہے۔
مقصد یہ ہے کہ ایک مربوط اور قابلِ آزمائش architecture پر converge کیا جائے بغیر یہ کوشش کئے کہ
تمام fixes ایک ہی monolithic patch میں اتارے جائیں۔

## 0. رہنما اصول
- مختلف hardware پر determinism برقرار رکھیں؛ acceleration صرف opt-in feature flags کے ذریعے اور
  یکساں fallbacks کے ساتھ استعمال کریں۔
- Norito serialization layer ہے۔ state/schema میں کوئی بھی تبدیلی Norito encode/decode round-trip
  tests اور fixtures updates کے ساتھ ہونی چاہئے۔
- configuration `iroha_config` کے ذریعے گزرتی ہے (user -> actual -> defaults)۔ production paths
  سے ad-hoc environment toggles ہٹائیں۔
- ABI policy V1 رہتی ہے اور ناقابلِ مذاکرات ہے۔ hosts کو unknown pointer types/syscalls کو
  deterministically reject کرنا چاہئے۔
- `cargo test --workspace` اور golden tests (`ivm`, `norito`, `integration_tests`) ہر milestone کے
  لئے baseline gate ہیں۔

## 1. Repository topology snapshot
- `crates/iroha_core`: Sumeragi actors، WSV، genesis loader، pipelines (query, overlay, zk lanes)،
  smart-contract host glue.
- `crates/iroha_data_model`: on-chain data اور queries کے لئے authoritative schema.
- `crates/iroha`: client API جو CLI، tests اور SDK استعمال کرتے ہیں۔
- `crates/iroha_cli`: operator CLI، فی الحال `iroha` میں موجود متعدد APIs کو mirror کرتی ہے۔
- `crates/ivm`: Kotodama bytecode VM، pointer-ABI host integration entry points۔
- `crates/norito`: serialization codec جس میں JSON adapters اور AoS/NCB backends شامل ہیں۔
- `integration_tests`: cross-component assertions جو genesis/bootstrap، Sumeragi، triggers،
  pagination وغیرہ کو cover کرتے ہیں۔
- docs پہلے ہی Sora Nexus Ledger کے goals بتاتی ہیں (`nexus.md`, `new_pipeline.md`, `ivm.md`)،
  مگر implementation fragmented ہے اور code کے مقابلے میں جزوی طور پر stale ہے۔

## 2. Refactor pillars اور milestones

### Phase A - Foundations اور observability
1. **WSV Telemetry + snapshots**
   - `state` میں canonical snapshot API قائم کریں (trait `WorldStateSnapshot`) جو queries، Sumeragi
     اور CLI استعمال کرتے ہیں۔
   - `scripts/iroha_state_dump.sh` کے ذریعے `iroha state dump --format norito` استعمال کر کے
     deterministic snapshots بنائیں۔
2. **Genesis/bootstrap determinism**
   - genesis ingestion کو ایک ہی Norito-powered pipeline میں refactor کریں (`iroha_core::genesis`).
   - integration/regression coverage شامل کریں جو genesis اور پہلے block کو replay کرے اور arm64/x86_64
     پر identical WSV roots assert کرے (tracking: `integration_tests/tests/genesis_replay_determinism.rs`).
3. **Cross-crate fixity tests**
   - `integration_tests/tests/genesis_json.rs` کو expand کریں تاکہ WSV، pipeline اور ABI invariants
     ایک ہی harness میں validate ہوں۔
   - `cargo xtask check-shape` scaffold متعارف کریں جو schema drift پر panic کرے (DevEx tooling backlog
     میں track؛ `scripts/xtask/README.md` action item دیکھیں)۔

### Phase B - WSV اور query surface
1. **State storage transactions**
   - `state/storage_transactions.rs` کو ایک transactional adapter میں collapse کریں جو commit ordering
     اور conflict detection نافذ کرے۔
   - unit tests اب verify کرتے ہیں کہ assets/world/triggers کی modifications failure پر rollback ہوں۔
2. **Query model refactor**
   - pagination/cursor logic کو reusable components میں منتقل کریں (`crates/iroha_core/src/query/`)۔
     Norito representations کو `iroha_data_model` میں align کریں۔
   - triggers، assets اور roles کے لئے snapshot queries شامل کریں، deterministic ordering کے ساتھ
     (tracking: `crates/iroha_core/tests/snapshot_iterable.rs`)۔
3. **Snapshot consistency**
   - یقینی بنائیں کہ `iroha ledger query` CLI وہی snapshot path استعمال کرے جو Sumeragi/fetchers کرتے ہیں۔
   - CLI snapshot regression tests `tests/cli/state_snapshot.rs` میں ہیں (slow runs کے لئے feature-gated)۔

### Phase C - Sumeragi pipeline
1. **Topology اور epoch management**
   - `EpochRosterProvider` کو trait میں extract کریں جس کی implementations WSV stake snapshots پر مبنی ہوں۔
   - `WsvEpochRosterAdapter::from_peer_iter` ایک سادہ اور mock-friendly constructor دیتا ہے۔
2. **Consensus flow simplification**
   - `crates/iroha_core/src/sumeragi/*` کو modules میں reorganize کریں: `pacemaker`, `aggregation`,
     `availability`, `witness` اور shared types `consensus` کے تحت۔
   - ad-hoc message passing کو typed Norito envelopes سے replace کریں اور view-change property tests
     شامل کریں (Sumeragi messaging backlog میں track)۔
3. **Lane/proof integration**
   - lane proofs کو DA commitments کے ساتھ align کریں اور RBC gating کو uniform بنائیں۔
   - end-to-end integration test `integration_tests/tests/extra_functional/seven_peer_consistency.rs`
     اب RBC-enabled path verify کرتا ہے۔

### Phase D - Smart contracts اور pointer-ABI hosts
1. **Host boundary audit**
   - pointer type checks (`ivm::pointer_abi`) اور host adapters (`iroha_core::smartcontracts::ivm::host`) کو consolidate کریں۔
   - pointer table expectations اور host manifest bindings `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs`
     اور `ivm_host_mapping.rs` سے cover ہوتے ہیں، جو golden TLV mappings exercise کرتے ہیں۔
2. **Trigger execution sandbox**
   - triggers کو ایک مشترک `TriggerExecutor` کے ذریعے چلانے کے لئے refactor کریں جو gas، pointer validation
     اور event journaling نافذ کرے۔
   - call/time triggers کے لئے regression tests شامل کریں جو failure paths cover کریں
     (tracking: `crates/iroha_core/tests/trigger_failure.rs`).
3. **CLI اور client alignment**
   - یقینی بنائیں کہ CLI operations (`audit`, `gov`, `sumeragi`, `ivm`) shared `iroha` client functions پر
     انحصار کریں تاکہ drift نہ ہو۔
   - CLI JSON snapshot tests `tests/cli/json_snapshot.rs` میں ہیں؛ انہیں up to date رکھیں تاکہ core command
     output canonical JSON reference سے match کرے۔

### Phase E - Norito codec hardening
1. **Schema registry**
   - `crates/norito/src/schema/` کے تحت Norito schema registry بنائیں تاکہ core data types کے canonical
     encodings حاصل ہوں۔
   - sample payload encoding verify کرنے والے doc tests شامل کریں (`norito::schema::SamplePayload`).
2. **Golden fixtures refresh**
   - `crates/norito/tests/*` golden fixtures کو نئے WSV schema کے مطابق update کریں جب refactor land ہو۔
   - `scripts/norito_regen.sh` helper `norito_regen_goldens` کے ذریعے Norito JSON goldens کو deterministically
     regenerate کرتا ہے۔
3. **IVM/Norito integration**
   - Kotodama manifest serialization کو end-to-end Norito کے ذریعے validate کریں تاکہ pointer ABI metadata
     consistent رہے۔
   - `crates/ivm/tests/manifest_roundtrip.rs` manifests کے لئے Norito encode/decode parity برقرار رکھتا ہے۔

## 3. Cross-cutting concerns
- **Testing Strategy**: ہر phase unit tests -> crate tests -> integration tests کو promote کرتی ہے۔ failing
  tests موجودہ regressions کو capture کرتے ہیں؛ نئے tests دوبارہ نمودار ہونے سے روکتے ہیں۔
- **Documentation**: ہر phase کے بعد `status.md` update کریں اور open items کو `roadmap.md` میں منتقل کریں
  جبکہ completed tasks کو prune کریں۔
- **Performance Benchmarks**: `iroha_core`, `ivm`, `norito` میں موجود benches برقرار رکھیں؛ post-refactor
  baseline measurements شامل کریں تاکہ regressions نہ ہوں۔
- **Feature Flags**: crate-level toggles صرف ان backends کے لئے رکھیں جنہیں external toolchains چاہیے
  (`cuda`, `zk-verify-batch`). CPU SIMD paths ہمیشہ build ہوتے ہیں اور runtime پر select ہوتے ہیں؛
  unsupported hardware کے لئے deterministic scalar fallbacks دیں۔

## 4. فوری اگلے اقدامات
- Phase A scaffolding (snapshot trait + telemetry wiring) - roadmap updates میں actionable tasks دیکھیں۔
- `sumeragi`, `state` اور `ivm` کے حالیہ defect audit نے یہ highlights ظاہر کئے:
  - `sumeragi`: dead-code allowances view-change proof broadcast، VRF replay state اور EMA telemetry export کو
    guard کرتے ہیں۔ یہ Phase C کے consensus flow simplification اور lane/proof integration deliverables تک gated رہتے ہیں۔
  - `state`: `Cell` cleanup اور telemetry routing Phase A WSV telemetry track میں جاتے ہیں، جبکہ SoA/parallel-apply
    notes Phase C pipeline optimization backlog میں شامل ہوتے ہیں۔
  - `ivm`: CUDA toggle exposure، envelope validation اور Halo2/Metal coverage Phase D host-boundary work اور
    cross-cutting GPU acceleration theme کے ساتھ align ہوتے ہیں؛ kernels dedicated GPU backlog میں رہتے ہیں۔
- Cross-team RFC تیار کریں جو اس پلان کو summarize کرے تاکہ invasive code changes سے پہلے sign-off ہو۔

## 5. کھلے سوالات
- کیا RBC کو P1 کے بعد optional رہنا چاہئے، یا Nexus ledger lanes کے لئے لازمی ہونا چاہئے؟ فیصلہ درکار ہے۔
- کیا ہم P1 میں DS composability groups enforce کریں یا lane proofs mature ہونے تک disabled رکھیں؟
- ML-DSA-87 parameters کے لئے canonical location کیا ہے؟ امیدوار: نیا crate `crates/fastpq_isi` (pending creation)۔

---

_آخری اپ ڈیٹ: 2025-09-12_

</div>

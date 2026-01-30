---
lang: es
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: nexus-refactor-plan
title: Sora Nexus لیجر ری فیکٹر پلان
description: `docs/source/nexus_refactor_plan.md` کا آئینہ، جو Iroha 3 کوڈ بیس کی مرحلہ وار صفائی کے کام کی تفصیل دیتا ہے۔
---

:::note کینونیکل ماخذ
یہ صفحہ `docs/source/nexus_refactor_plan.md` کی عکاسی کرتا ہے۔ لوکلائزڈ ایڈیشنز پورٹل تک پہنچنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# Sora Nexus Ledger Refactor Plan

یہ دستاویز Sora Nexus Ledger ("Iroha 3") کے ری فیکٹر کے لئے فوری roadmap کو محفوظ کرتی ہے۔ یہ موجودہ ریپوزٹری layout اور genesis/WSV bookkeeping، Sumeragi consensus، smart-contract triggers، snapshot queries، pointer-ABI host bindings اور Norito codecs میں دیکھی گئی regressions کی عکاسی کرتی ہے۔ مقصد یہ ہے کہ تمام fixes کو ایک بڑے monolithic patch میں اتارنے کے بجائے ایک مربوط، قابل ٹیسٹ architecture تک پہنچا جائے۔

## 0. رہنما اصول
- مختلف ہارڈویئر پر deterministic رویہ برقرار رکھیں; acceleration صرف opt-in feature flags کے ذریعے اور ایک جیسے fallbacks کے ساتھ استعمال کریں۔
- Norito serialization layer ہے۔ کسی بھی state/schema تبدیلی میں Norito encode/decode round-trip tests اور fixture updates شامل ہونے چاہئیں۔
- configuration `iroha_config` (user -> actual -> defaults) کے ذریعے گزرتی ہے۔ پروڈکشن paths سے ad-hoc environment toggles ہٹا دیں۔
- ABI policy V1 پر قائم اور غیر قابل گفت و شنید ہے۔ hosts کو نامعلوم pointer types/syscalls کو deterministic انداز میں رد کرنا ہوگا۔
- `cargo test --workspace` اور golden tests (`ivm`, `norito`, `integration_tests`) ہر milestone کے لئے بنیادی gate رہیں گے۔

## 1. ریپوزٹری ٹاپولوجی اسنیپ شاٹ
- `crates/iroha_core`: Sumeragi actors، WSV، genesis loader، pipelines (query, overlay, zk lanes)، اور smart-contract host glue۔
- `crates/iroha_data_model`: on-chain data اور queries کے لئے authoritative schema۔
- `crates/iroha`: client API جو CLI، tests، SDK میں استعمال ہوتا ہے۔
- `crates/iroha_cli`: آپریٹر CLI، جو اس وقت `iroha` کی متعدد APIs کو mirror کرتا ہے۔
- `crates/ivm`: Kotodama bytecode VM، pointer-ABI host integration entry points۔
- `crates/norito`: serialization codec جس میں JSON adapters اور AoS/NCB backends شامل ہیں۔
- `integration_tests`: cross-component assertions جو genesis/bootstrap، Sumeragi، triggers، pagination وغیرہ کو کور کرتے ہیں۔
- Docs پہلے ہی Sora Nexus Ledger اہداف (`nexus.md`, `new_pipeline.md`, `ivm.md`) بیان کرتے ہیں، مگر implementation ٹکڑوں میں ہے اور کوڈ کے مقابلے میں جزوی طور پر پرانی ہے۔

## 2. ری فیکٹر ستون اور milestones

### Phase A - Foundations and Observability
1. **WSV Telemetry + Snapshots**
   - `state` میں canonical snapshot API (trait `WorldStateSnapshot`) قائم کریں جسے queries، Sumeragi اور CLI استعمال کریں۔
   - `scripts/iroha_state_dump.sh` استعمال کریں تاکہ `iroha state dump --format norito` کے ذریعے deterministic snapshots بنیں۔
2. **Genesis/Bootstrap Determinism**
   - genesis ingestion کو اس طرح ری فیکٹر کریں کہ یہ ایک واحد Norito-powered pipeline (`iroha_core::genesis`) سے گزرے۔
   - integration/regression coverage شامل کریں جو genesis اور پہلے بلاک کو replay کرے اور arm64/x86_64 کے درمیان یکساں WSV roots ثابت کرے (ٹریک: `integration_tests/tests/genesis_replay_determinism.rs`)۔
3. **Cross-crate Fixity Tests**
   - `integration_tests/tests/genesis_json.rs` کو بڑھائیں تاکہ WSV، pipeline اور ABI invariants کو ایک harness میں validate کیا جا سکے۔
   - `cargo xtask check-shape` scaffold متعارف کریں جو schema drift پر panic کرے (DevEx tooling backlog میں ٹریک; `scripts/xtask/README.md` کی action item دیکھیں)۔

### Phase B - WSV اور Query Surface
1. **State Storage Transactions**
   - `state/storage_transactions.rs` کو ایک transactional adapter میں سمیٹیں جو commit ordering اور conflict detection نافذ کرے۔
   - unit tests اب تصدیق کرتے ہیں کہ asset/world/triggers کی تبدیلیاں ناکامی پر rollback ہوں۔
2. **Query Model Refactor**
   - pagination/cursor logic کو `crates/iroha_core/src/query/` کے تحت reusable components میں منتقل کریں۔ `iroha_data_model` میں Norito representations کو align کریں۔
   - triggers، assets اور roles کے لئے deterministic ordering کے ساتھ snapshot queries شامل کریں (موجودہ coverage `crates/iroha_core/tests/snapshot_iterable.rs` میں ٹریک ہے)۔
3. **Snapshot Consistency**
   - یقینی بنائیں کہ `iroha ledger query` CLI وہی snapshot path استعمال کرے جو Sumeragi/fetchers استعمال کرتے ہیں۔
   - CLI snapshot regression tests `tests/cli/state_snapshot.rs` میں ہیں (slow runs کے لئے feature-gated)۔

### Phase C - Sumeragi Pipeline
1. **Topology اور Epoch Management**
   - `EpochRosterProvider` کو ایک trait میں نکالیں جس کی implementations WSV stake snapshots پر مبنی ہوں۔
   - `WsvEpochRosterAdapter::from_peer_iter` benches/tests کے لئے ایک سادہ mock-friendly constructor فراہم کرتا ہے۔
2. **Consensus Flow Simplification**
   - `crates/iroha_core/src/sumeragi/*` کو ماڈیولز میں ری آرگنائز کریں: `pacemaker`, `aggregation`, `availability`, `witness` اور مشترکہ types کو `consensus` کے تحت رکھیں۔
   - ad-hoc message passing کو typed Norito envelopes سے بدلیں اور view-change property tests متعارف کریں (Sumeragi messaging backlog میں ٹریک)۔
3. **Lane/Proof Integration**
   - lane proofs کو DA commitments کے ساتھ align کریں اور یقینی بنائیں کہ RBC gating یکساں ہو۔
   - end-to-end integration test `integration_tests/tests/extra_functional/seven_peer_consistency.rs` اب RBC-enabled path کو verify کرتا ہے۔

### Phase D - Smart Contracts اور Pointer-ABI Hosts
1. **Host Boundary Audit**
   - pointer-type checks (`ivm::pointer_abi`) اور host adapters (`iroha_core::smartcontracts::ivm::host`) کو consolidate کریں۔
   - pointer table expectations اور host manifest bindings کو `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` اور `ivm_host_mapping.rs` کور کرتے ہیں، جو golden TLV mappings کو exercise کرتے ہیں۔
2. **Trigger Execution Sandbox**
   - triggers کو اس طرح ری فیکٹر کریں کہ وہ مشترکہ `TriggerExecutor` کے ذریعے چلیں جو gas، pointer validation، اور event journaling نافذ کرتا ہے۔
   - call/time triggers کے لئے regression tests شامل کریں جو failure paths کو کور کرتے ہیں (ٹریک: `crates/iroha_core/tests/trigger_failure.rs`)۔
3. **CLI اور Client Alignment**
   - یقینی بنائیں کہ CLI operations (`audit`, `gov`, `sumeragi`, `ivm`) drift سے بچنے کے لئے shared `iroha` client functions پر انحصار کریں۔
   - CLI JSON snapshot tests `tests/cli/json_snapshot.rs` میں ہیں؛ انہیں اپ ٹو ڈیٹ رکھیں تاکہ core command output canonical JSON reference سے match کرتا رہے۔

### Phase E - Norito Codec Hardening
1. **Schema Registry**
   - `crates/norito/src/schema/` کے تحت Norito schema registry بنائیں تاکہ core data types کے لئے canonical encodings دستیاب ہوں۔
   - sample payload encoding کو verify کرنے والے doc tests شامل کریں (`norito::schema::SamplePayload`)۔
2. **Golden Fixtures Refresh**
   - `crates/norito/tests/*` کے golden fixtures کو اپ ڈیٹ کریں تاکہ refactor کے بعد نئی WSV schema سے match ہوں۔
   - `scripts/norito_regen.sh` helper `norito_regen_goldens` کے ذریعے Norito JSON goldens کو deterministic طور پر regenerate کرتا ہے۔
3. **IVM/Norito Integration**
   - Kotodama manifest serialization کو Norito کے ذریعے end-to-end validate کریں، تاکہ pointer ABI metadata مستقل رہے۔
   - `crates/ivm/tests/manifest_roundtrip.rs` manifests کے لئے Norito encode/decode parity برقرار رکھتا ہے۔

## 3. Cross-cutting امور
- **Testing Strategy**: ہر phase unit tests -> crate tests -> integration tests کو فروغ دیتا ہے۔ failing tests موجودہ regressions کو پکڑتے ہیں؛ نئے tests انہیں واپس آنے سے روکتے ہیں۔
- **Documentation**: ہر phase کے اترنے کے بعد `status.md` اپ ڈیٹ کریں اور کھلے items کو `roadmap.md` میں منتقل کریں، جبکہ مکمل شدہ کام prune کریں۔
- **Performance Benchmarks**: `iroha_core`, `ivm` اور `norito` میں موجود benches برقرار رکھیں؛ refactor کے بعد baseline measurements شامل کریں تاکہ regressions نہ ہوں۔
- **Feature Flags**: crate-level toggles صرف ان backends کے لئے رکھیں جنہیں بیرونی toolchains چاہیے (`cuda`, `zk-verify-batch`)۔ CPU SIMD paths ہمیشہ build ہوتے اور runtime پر منتخب ہوتے ہیں؛ unsupported hardware کے لئے deterministic scalar fallbacks فراہم کریں۔

## 4. فوری اگلے اقدامات
- Phase A scaffolding (snapshot trait + telemetry wiring) - roadmap updates میں actionable tasks دیکھیں۔
- `sumeragi`, `state`, اور `ivm` کی حالیہ defect audit نے درج ذیل highlights سامنے لائے:
  - `sumeragi`: dead-code allowances view-change proof broadcast، VRF replay state، اور EMA telemetry export کو guard کرتے ہیں۔ یہ Phase C کے consensus flow simplification اور lane/proof integration deliverables اترنے تک gated رہیں گے۔
  - `state`: `Cell` cleanup اور telemetry routing Phase A کے WSV telemetry track پر منتقل ہوتے ہیں، جبکہ SoA/parallel-apply نوٹس Phase C pipeline optimization backlog میں شامل ہوتے ہیں۔
  - `ivm`: CUDA toggle exposure، envelope validation، اور Halo2/Metal coverage Phase D کے host-boundary کام اور cross-cutting GPU acceleration theme سے میپ ہوتے ہیں؛ kernels تیار ہونے تک dedicated GPU backlog پر رہتے ہیں۔
- invasive code changes اترنے سے پہلے اس پلان کا خلاصہ دینے والا cross-team RFC تیار کریں تاکہ sign-off مل سکے۔

## 5. کھلے سوالات
- کیا RBC کو P1 کے بعد بھی optional رہنا چاہیے، یا Nexus ledger lanes کے لئے لازمی ہے؟ stakeholder فیصلہ درکار ہے۔
- کیا ہم P1 میں DS composability groups نافذ کریں یا lane proofs کے mature ہونے تک انہیں disable رکھیں؟
- ML-DSA-87 parameters کی canonical location کیا ہے؟ امیدوار: نیا crate `crates/fastpq_isi` (pending creation)۔

---

_آخری اپ ڈیٹ: 2025-09-12_

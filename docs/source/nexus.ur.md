---
lang: ur
direction: rtl
source: docs/source/nexus.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c8da33b0abb8a6d46dbaaed657c8338a9d723a97f6f28ff29a62caf84c0dbfd6
source_last_modified: "2025-12-27T07:56:34.355655+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- اردو ترجمہ برائے docs/source/nexus.md -->

#! Iroha 3 - Sora Nexus Ledger: فنی ڈیزائن وضاحت

یہ دستاویز Iroha 3 کے لئے Sora Nexus Ledger کی مجوزہ معماری بیان کرتی ہے، جو Iroha 2 کو ایک واحد عالمی لاجکلی یونفائیڈ لیجر کی طرف لے جاتی ہے جو Data Spaces ‏(DS) کے گرد منظم ہے۔ Data Spaces مضبوط پرائیویسی ڈومینز ("private data spaces") اور اوپن پارٹیسپیشن ("public data spaces") فراہم کرتے ہیں۔ ڈیزائن عالمی لیجر میں کمپوزایبلٹی برقرار رکھتے ہوئے نجی DS ڈیٹا کے لئے سخت isolation اور confidentiality یقینی بناتا ہے اور Kura (block storage) اور WSV (World State View) میں erasure coding کے ذریعے data availability scaling متعارف کراتا ہے۔

ایک ہی ریپوزٹری Iroha 2 (self-hosted networks) اور Iroha 3 (SORA Nexus) دونوں بناتی ہے۔ execution مشترکہ Iroha Virtual Machine (IVM) اور Kotodama toolchain سے چلتی ہے، لہذا contracts اور bytecode artifacts self-hosted deployments اور Nexus global ledger دونوں میں portable رہتے ہیں۔

Goals
- ایک عالمی منطقی لیجر جو کئی validators اور Data Spaces سے مل کر بنتا ہے۔
- permissioned آپریشن کے لئے Private Data Spaces (مثلا CBDC) جہاں ڈیٹا DS سے باہر نہ جائے۔
- Public Data Spaces جو اوپن پارٹیسپیشن اور Ethereum جیسے permissionless access دیں۔
- Data Spaces کے درمیان composable smart contracts، مگر private DS assets کے لئے explicit permissions کے ساتھ۔
- Performance isolation تاکہ public activity private DS کے اندرونی txs کو degrade نہ کرے۔
- Data availability at scale: erasure-coded Kura/WSV تاکہ بہت بڑے data کو handle کیا جا سکے اور private DS کا ڈیٹا محفوظ رہے۔

Non-Goals (Initial Phase)
- Token economics یا validator incentives کی تعریف؛ scheduling اور staking pluggable ہیں۔
- نئی ABI version متعارف کرانا یا syscalls/pointer-ABI surfaces بڑھانا نہیں؛ ABI v1 فکس ہے اور runtime upgrades host ABI نہیں بدلتے۔

Terminology
- Nexus Ledger: عالمی منطقی لیجر جو Data Space (DS) بلاکس کو ایک ordered history اور state commitment میں compose کرتا ہے۔
- Data Space (DS): ایک bounded execution/storage domain جس کے اپنے validators، governance، privacy class، DA policy، quotas اور fee policy ہیں۔ دو کلاسیں: public DS اور private DS۔
- Private Data Space: permissioned validators اور access control؛ tx/state ڈیٹا DS سے باہر نہیں جاتا؛ صرف commitments/metadata globally anchor ہوتے ہیں۔
- Public Data Space: permissionless participation؛ مکمل ڈیٹا اور state عوامی۔
- Data Space Manifest (DS Manifest): Norito-encoded manifest جو DS parameters بیان کرتا ہے (validators/QC keys, privacy class, ISI policy, DA params, retention, quotas, ZK policy, fees)؛ hash nexus chain پر anchor ہوتا ہے؛ default DS QC کیلئے ML-DSA-87 (Dilithium5-class)۔
- Space Directory: global on-chain directory contract جو DS manifests، versions اور governance/rotation events track کرتا ہے۔
- DSID: Data Space کا عالمی unique identifier، تمام objects/references کیلئے namespace۔
- Anchor: DS block/header کا cryptographic commitment جو nexus chain میں شامل ہو کر DS history کو global ledger سے bind کرتا ہے۔
- Kura: Iroha block storage، یہاں erasure-coded blobs اور commitments کے ساتھ توسیع۔
- WSV: Iroha World State View، یہاں versioned, snapshot-capable erasure-coded state segments کے ساتھ توسیع۔
- IVM: Iroha Virtual Machine for smart contracts (Kotodama bytecode `.to`)۔
 - AIR: Algebraic Intermediate Representation، STARK-style proofs کیلئے algebraic execution view (field traces + transition/boundary constraints)۔

Data Spaces Model
- Identity: `DataSpaceId (DSID)` DS کی شناخت اور namespacing؛ دو granularities:
  - Domain-DS: `ds::domain::<domain_name>`
  - Asset-DS: `ds::asset::<domain_name>::<asset_name>`
  دونوں ساتھ چلتے ہیں؛ transactions متعدد DSIDs کو atomically touch کر سکتی ہیں۔
- Manifest lifecycle: DS creation/updates/retirement Space Directory میں؛ ہر slot artifact latest manifest hash کو reference کرتا ہے۔
- Classes: public DS (open participation, public DA) اور private DS (permissioned, confidential DA)؛ hybrid policies manifest flags سے ممکن۔
- Policies per DS: ISI permissions، DA params `(k,m)`، encryption، retention، quotas، ZK/optimistic proof policy، fees۔
- Governance: membership/rotation manifest governance section سے، on-chain proposals/multisig یا external governance anchored by nexus txs/attestations۔

Dataspace-aware gossip
- Gossip batches اب plane tag (public vs restricted) لاتے ہیں؛ restricted batches commit topology کے online peers کو unicast ہوتے ہیں (`transaction_gossip_restricted_target_cap`) جبکہ public batches `transaction_gossip_public_target_cap` استعمال کرتے ہیں (`null` = broadcast)۔ Targets کی reshuffle cadence `transaction_gossip_public_target_reshuffle_ms` اور `transaction_gossip_restricted_target_reshuffle_ms` سے آتی ہے۔ جب commit topology میں online peers نہیں ہوں تو `transaction_gossip_restricted_public_payload` (default `refuse`) کے ذریعے restricted payloads کو public overlay پر forward یا refuse کیا جا سکتا ہے۔ Telemetry fallback attempts، forward/drop counts اور policy دکھاتا ہے۔
- Unknown dataspaces اگر `transaction_gossip_drop_unknown_dataspace` enable ہو تو re-queue ہوتے ہیں؛ ورنہ restricted targeting پر fall back۔
- Receive-side validation ایسے entries drop کرتی ہے جن میں lane/dataspace مقامی catalog سے نہیں ملتے، plane tag derived visibility سے mismatch ہو، یا advertised route مقامی طور پر re-derive کی گئی routing decision سے match نہ کرے۔

Capability manifests & UAID
- Universal accounts: ہر participant کیلئے deterministic UAID (`UniversalAccountId` in `crates/iroha_data_model/src/nexus/manifest.rs`) جو تمام dataspaces پر لاگو ہوتا ہے۔ `AssetPermissionManifest` UAID کو dataspace، activation/expiry epochs اور allow/deny `ManifestEntry` rules سے bind کرتا ہے۔ Deny rules ہمیشہ جیتتے ہیں؛ `ManifestVerdict::Denied` یا `Allowed` grant metadata کے ساتھ نکلتا ہے۔
- UAID portfolio snapshots `GET /v1/accounts/{uaid}/portfolio` کے ذریعے؛ aggregator `iroha_core::nexus::portfolio` میں۔
- Allowances: deterministic `AllowanceWindow` buckets (`PerSlot`, `PerMinute`, `PerDay`) اور optional `max_amount`۔
- Audit telemetry: `SpaceDirectoryEvent::{ManifestActivated, ManifestExpired, ManifestRevoked}` events اور `SpaceDirectoryEventFilter` کے ذریعے Torii/data-event subscribers کو updates ملتے ہیں۔

### UAID manifest operations

Space Directory operations CLI اور Torii دونوں سے دستیاب ہیں۔ دونوں راستے `CanPublishSpaceDirectoryManifest{dataspace}` permission enforce کرتے ہیں اور lifecycle events world state میں record کرتے ہیں۔

#### CLI workflow (`iroha app space-directory manifest ...`)

1. **Encode manifest JSON**:

   ```bash
   iroha app space-directory manifest encode \
     --json dataspace/capability.json \
     --out artifacts/capability.manifest.to \
     --hash-out artifacts/capability.manifest.hash
   ```

2. **Publish/replace manifests**:

   ```bash
   iroha app space-directory manifest publish \
     --manifest artifacts/capability.manifest.to \
     --reason "Retail wave 4 on-boarding"
   ```

3. **Expire/Revok**e:

   ```bash
   iroha app space-directory manifest expire \
     --uaid uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11 \
     --dataspace 11 \
     --expired-epoch 4600

   iroha app space-directory manifest revoke \
     --uaid uaid:0f4d86b20839a8ddbe8a1a3d21cf1c502d49f3f79f0fa1cd88d5f24c56c0ab11 \
     --dataspace 11 \
     --revoked-epoch 9216 \
     --reason "Fraud investigation NX-16-R05"
   ```

4. **Audit bundles**:

   ```bash
   iroha app space-directory manifest audit-bundle \
     --manifest-json dataspace/capability.json \
     --profile dataspace/profiles/cbdc_profile.json \
     --out-dir artifacts/capability_bundle
   ```

#### Torii APIs

- `GET /v1/space-directory/uaids/{uaid}`
- `GET /v1/space-directory/uaids/{uaid}/portfolio`
- `GET /v1/space-directory/uaids/{uaid}/manifests?dataspace={id}`
- `POST /v1/space-directory/manifests`
- `POST /v1/space-directory/manifests/revoke`

JS SDK میں `ToriiClient.getUaidPortfolio` وغیرہ دستیاب ہیں؛ Swift/Python مستقبل میں۔

Recent SDK/AMX updates
- NX-11: relay envelope verification helpers (Rust/Python/JS bindings)۔
- NX-17: `enforce_amx_budget` کے ذریعے 30 ms / 140 ms budgets کا نفاذ۔

High-Level Architecture
1) Global Composition Layer (Nexus Chain)
- 1s Nexus Blocks کے canonical order سے atomic cross-DS txs finalize ہوتے ہیں۔
- Minimal metadata + aggregated proofs/QC؛ DSIDs، roots، DA commitments، DS QC (ML-DSA-87)۔
- Consensus: single global pipelined BFT committee (22 nodes, 3f+1) VRF/stake سے منتخب۔

2) Data Space Layer (Public/Private)
- DS fragments execute، DS-local WSV update، per-block validity artifacts تیار۔
- Private DS میں data encrypted، commitments/proofs ہی باہر۔
- Public DS مکمل data bodies + PQ proofs۔

3) AMX (Atomic Cross-Data-Space)
- One tx can touch multiple DS; commit atomically in 1s block or abort۔
- Prepare-Commit: DS parallel execution + FASTPQ-ISI proofs + DA commitments؛ DA cert <=300 ms۔
- Consistency via read-write sets + conflict detection؛ privacy preserved۔

4) DA with Erasure Coding
- Kura bodies + WSV snapshots erasure-coded؛ public widely sharded، private encrypted within private validators۔
- DA commitments recorded in DS artifacts and Nexus blocks۔

Block and Commit Structure
- Data Space Proof Artifact fields: dsid, slot, pre/post roots, ds_tx_set_hash, kura/wsv commitments, manifest_hash, ds_qc, ds_validity_proof.
- Nexus Block fields: block_number, parent_hash, slot_time, tx_list, ds_artifacts[], nexus_qc.

Consensus and Scheduling
- Nexus Chain BFT: 22-node committee، 1s block/finality۔
- DS consensus: per-DS BFT، lane-relay committee `3f+1`، VRF epoch sampling bound to `(dataspace_id, lane_id)`۔
- Scheduling: DS artifacts verify + DA cert <=300 ms۔
- Performance isolation via per-DS quotas۔

Data Model and Namespacing
- DS-qualified IDs: `dsid` qualifies domains/accounts/assets/roles.
- Global references `(dsid, object_id, version_hint)`.
- Norito serialization everywhere، no serde in production۔

Smart Contracts and IVM Extensions
- `dsid` in execution context؛ Kotodama contracts run inside DS.
- AMX syscalls: `amx_begin`, `amx_touch`, `amx_commit`, `verify_space_proof`, `use_asset_handle`.
- Fees paid in DS gas token؛ policies extendable۔
- Deterministic syscalls with declared read/write sets۔

Post-Quantum Validity Proofs (FASTPQ-ISI)
- hash-based PQ proofs، no trusted setup؛ production backend only۔
- AIR design, SMT commitments, FRI commitments, optional recursion۔
- Performance targets and DS manifest config (`zk.policy`, `zk.hash`, `zk.fri`, `state.commitment`, `zk.recursion`, `attestation.*`).
- Fallbacks for heavy ISIs (general STARK)؛ non-PQ options not default۔

AIR Primer
- execution trace matrix، transition/boundary constraints، lookups/permutations، FRI verification، transfer example۔

ABI Stability (ABI v1)
- ABI v1 surface fixed ہے؛ نئے syscalls یا pointer-ABI types شامل نہیں ہوتے۔
- runtime upgrades میں `abi_version = 1` اور `added_syscalls`/`added_pointer_types` خالی رہیں۔
- ABI goldens (syscall list, ABI hash, pointer type IDs) فکس رہتے ہیں۔

Privacy Model
- private DS data stays private؛ public exposure limited to commitments/proofs؛ optional ZK proofs؛ ISI policies enforce access control۔

Performance Isolation and QoS
- per-DS mempools/consensus/storage؛ quotas； IVM budgets؛ async cross-DS calls۔

Data Availability and Storage Design
- Reed-Solomon parameters for public (`k=32, m=16`) and private (`k=16, m=8`) DS؛ DA commitments؛ VRF-sampled attesters; Kura/WSV integrations; retention/pruning rules۔

Networking and Node Roles
- Global Validators، Data Space Validators، DA Nodes roles defined۔

System-Level Improvements
- DAG mempool، DS quotas، PQ attestations، DA attesters، recursion، lane scaling، deterministic acceleration، lane activation thresholds۔

Fees and Economics
- DS gas unit; round-robin inclusion; optional global fee market۔

Cross-Data-Space Workflow (Example)
- AMX flow: P/S execution, proofs, DA cert, commit or abort۔

Security Considerations
- Deterministic execution, access control, confidentiality, DoS resistance۔

Changes to Iroha Components
- `iroha_data_model`, `ivm`, `iroha_core`, `kura`, `WSV`, `irohad` updates۔

Configuration and Determinism
- `iroha_config` only; deterministic hardware acceleration with feature flags۔

### Runtime Lane Lifecycle Control

- `POST /v1/nexus/lifecycle` additions/retire بغیر restart؛ validation اور safety کیلئے shared state-view lock استعمال ہوتا ہے۔
- Propagation: queue routing/limits اور lane manifests updated catalog سے rebuild ہوتے ہیں، اور consensus/DA/RBC workers state snapshots کے ذریعے refreshed lane config پڑھتے ہیں، لہذا scheduling اور validator selection بغیر restart کے بدل جاتا ہے (in-flight کام پچھلی config پر مکمل ہوتا ہے)۔
- Storage cleanup: Kura/tiered WSV geometry reconcile (create/retire/relabel) ہوتی ہے، DA shard cursor mappings sync/persist ہوتے ہیں، اور retired lanes lane relay caches اور DA commitment/confidential-compute/pin-intent stores سے prune ہوتے ہیں۔

Migration Path
- 5-step path from Iroha 2 to Iroha 3 with AMX, DA, PQ proofs۔

Testing Strategy
- Unit/IVM/integration/security tests as listed۔

### NX-18 Telemetry & Runbook Assets

- Grafana dashboard, CI gate, evidence bundler, release automation, runbook، telemetry helpers۔

Open Questions
- Questions on signatures, gas economics, DA attesters, default params, DS granularity, heavy ISIs, cross-DS conflicts۔

Appendix: Compliance with Repository Policies
- Norito-only wire/JSON, ABI v1 only with fixed syscall/pointer-ABI surface, determinism preserved, no serde/env in production paths۔

</div>

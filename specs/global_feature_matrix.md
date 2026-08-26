# Global Feature Matrix

Legend: `◉` fully implemented · `○` mostly implemented · `▲` partially implemented · `△` implementation just started · `✖︎` not started

## Consensus & Networking

| Feature | Status | Notes | Evidence |
|---------|--------|-------|----------|
| Signed committee and manifest context | ◉ | Consensus mode, validator set, quorum, cadence, and RS16 geometry are signed chain context; peers reject mismatched manifests instead of consulting local Sumeragi switches. | `sumeragi.md`; `sumeragi_v2.md` |
| Deterministic pacemaker | ◉ | The view timeout is derived from signed cadence and view number. Local EMA/RTT/jitter/backoff knobs are not authoritative inputs. | `sumeragi_pacemaker.md` |
| NEW_VIEW gating & highest QC tracking | ◉ | Authenticated control flow carries timeout certificates and evidence; the highest justified QC advances monotonically. | `sumeragi.md`; `sumeragi_v2.md` |
| Availability-certified commit | ◉ | Revision-4 commits require the signed manifest, RS16 availability proof, and a `2f + 1` commit QC before local application. | `sumeragi.md`; `sumeragi_v2.md` |
| Commit QC state-root binding | ◉ | Commit QCs bind the canonical block and state transition in the signed consensus context. | `sumeragi.md`; `sumeragi_v2.md` |
| Evidence propagation & audit endpoints | ◉ | Authenticated evidence is governed by `SumeragiNposParameters.reconfig`; Torii exposes read-only evidence and consensus diagnostics. | `sumeragi_evidence_api.md`; `governance_api.md` |
| Consensus observability | ◉ | `/status` is authoritative. Older collector/RBC and adaptive-pacemaker fields are legacy-labeled, non-authoritative observations and may remain zero. | `telemetry.md`; `references/operator_aids.md` |
| Consensus fingerprint verification | ◉ | Peers derive and compare the canonical signed-context fingerprint; obsolete local K/r or DA tables cannot select protocol behavior. | `sumeragi.md`; `references/configuration.md` |

## Pipeline, Kura & State

| Feature | Status | Notes | Evidence |
|---------|--------|-------|----------|
| Quarantine lane caps & telemetry | ◉ | Config knobs, deterministic overflow handling, and telemetry counters implemented. | status.md:263 |
| Pipeline worker pool knob | ◉ | `[pipeline].workers` threaded through state init with env parsing tests. | status.md:264 |
| Snapshot query lane (stored/ephemeral cursors) | ◉ | Stored cursor mode with Torii integration and blocking worker pools. | status.md:265; status.md:371; status.md:501 |
| Static DAG fingerprint recovery sidecars | ◉ | Sidecars stored in Kura, validated on startup, warnings emitted on mismatches. | status.md:106; status.md:349 |
| Kura block store hash decoding hardening | ◉ | Hash reads switched to raw 32-byte handling with Norito-independent roundtrip tests. | status.md:608; status.md:668 |
| Norito columnar telemetry | ◉ | AoS vs NCB columnar selection metrics are available in Norito. | status.md:156 |
| Snapshot WSV queries via Torii | ◉ | Torii snapshot query lane uses blocking worker pool, deterministic semantics. | status.md:501 |
| Trigger by-call execution chaining | ◉ | Data triggers chain immediately after by-call execution with deterministic order. | status.md:668 |

## Norito Serialization & Tooling

| Feature | Status | Notes | Evidence |
|---------|--------|-------|----------|
| Norito JSON migration (workspace) | ◉ | Serde removed from production; inventory + guardrails keep the workspace Norito-only. | status.md:112; status.md:124 |
| Serde deny-list & CI guardrails | ◉ | Guard workflows/scripts prevent new direct Serde usage across workspace. | status.md:218 |
| Norito codec goldens & AoS/NCB tests | ◉ | AoS/NCB goldens, truncation tests, and doc sync added. | status.md:140-147; status.md:149-150; status.md:332; status.md:666 |
| Norito feature matrix tooling | ◉ | `scripts/run_norito_feature_matrix.sh` covers the minimal and canonical codec feature families and supports downstream smoke tests. | status.md:146; status.md:152 |
| Norito language bindings (Python/Java) | ◉ | Python and Java Norito codecs maintained with sync scripts. | status.md:74; status.md:81 |
| Norito Stage-1 SIMD structural classifiers | ◉ | NEON/AVX2 stage-1 classifiers with cross-arch goldens and randomized corpora tests. | status.md:241 |

## Governance & Runtime Upgrades

| Feature | Status | Notes | Evidence |
|---------|--------|-------|----------|
| Runtime upgrade admission (ABI gating) | ◉ | Active ABI set enforced at admission with structured errors and tests. | status.md:196 |
| Protected namespace deploy gating | ▲ | Deploy metadata requirements and gating wired; policy/UX still evolving. | status.md:171 |
| Torii governance read endpoints | ◉ | `/v1/gov/*` read APIs routed with router tests. | status.md:212 |
| Verifying-key registry lifecycle & events | ◉ | VK register/update/deprecate, events, CLI filters, and retention semantics implemented. | status.md:236-239; status.md:595; status.md:603 |

## Zero-Knowledge Infrastructure

| Feature | Status | Notes | Evidence |
|---------|--------|-------|----------|
| Attachment storage APIs | ◉ | `POST/GET/LIST/DELETE` attachment endpoints with deterministic ids and tests. | status.md:231 |
| Background prover worker & report TTL | ▲ | Prover stub behind feature flag; TTL GC and config knobs wired; full pipeline pending. | status.md:212; status.md:233 |
| Envelope hash binding in CoreHost | ◉ | Verify envelope hashes bound through CoreHost and exposed via audit pulses. | status.md:250 |
| Shielded root history gating | ◉ | Root snapshots threaded into CoreHost with bounded history and empty-root config. | status.md:303 |
| ZK ballot execution & governance locks | ○ | Nullifier derivation, lock updates, verification toggles implemented; full proof lifecycle still maturing. | status.md:126-128; status.md:194-195 |
| Proof attachment pre-verify & dedup | ◉ | Stateless, non-authoritative backend-tag checks and deduplication; ledger execution always performs guarded cryptographic verification before recording a result. | crates/iroha_core/src/state.rs; crates/iroha_core/src/smartcontracts/isi/world.rs |
| ZK Torii proof fetch endpoint | ◉ | `/v1/zk/proof/{backend}/{hash}` exposes proof records (status, height, vk_ref/commitment). | status.md:94 |

## IVM & Kotodama Integration

| Feature | Status | Notes | Evidence |
|---------|--------|-------|----------|
| CoreHost syscall→ISI bridge | ○ | Pointer TLV decoding and syscall queueing operational; coverage gaps/parity tests planned. | status.md:299-307; status.md:477-486 |
| Pointer constructors & domain builtins | ◉ | Kotodama builtins emit typed Norito TLVs and SCALLs, with IR/e2e tests and docs. | status.md:299-301 |
| Pointer-ABI strict validation & doc sync | ◉ | TLV policy enforced across host/IVM with golden tests and generated docs. | status.md:227; status.md:317; status.md:344; status.md:366; status.md:527 |
| ZK syscall gating via CoreHost | ◉ | Per-op queues gate verified envelopes and enforce hash matching before ISI execution. | crates/iroha_core/src/smartcontracts/ivm/host.rs:213; crates/iroha_core/src/smartcontracts/ivm/host.rs:279 |
| Kotodama pointer-ABI docs & grammar | ◉ | Grammar/docs synced with live constructors and SCALL mappings. | status.md:299-301 |
| ISO 20022 schema-driven engine & Torii bridge | ◉ | Canonical ISO 20022 schemas embedded, deterministic XML parsing, and `/v1/iso20022/messages/{MsgId}` API exposed. | status.md:65-70 |

## Hardware Acceleration

| Feature | Status | Notes | Evidence |
|---------|--------|-------|----------|
| SIMD tail/misalignment parity tests | ◉ | Randomized parity tests ensure SIMD vector ops match scalar semantics for arbitrary alignment. | status.md:243 |
| Metal/CUDA fallback & self-tests | ◉ | GPU backends run golden self-tests and fall back to scalar/SIMD on mismatch; parity suites cover SHA-256/Keccak/AES. | status.md:244-246 |

## Network Time & Consensus Modes

| Feature | Status | Notes | Evidence |
|---------|--------|-------|----------|
| Network Time Service (NTS) | ✖︎ | Design exists in `new_pipeline.md`; implementation not yet tracked in status updates. | new_pipeline.md |
| Nominated PoS consensus mode | ✖︎ | Nexus design documents closed-set and NPoS modes; core implementation pending. | new_pipeline.md; nexus.md |

## Nexus Ledger Roadmap

| Feature | Status | Notes | Evidence |
|---------|--------|-------|----------|
| Space Directory contract scaffold | ✖︎ | Global registry contract for DS manifests/governance not implemented yet. | nexus.md |
| Data Space manifest format & lifecycle | ✖︎ | Norito manifest schema, versioning, and governance flow remain on the roadmap. | nexus.md |
| DS governance & validator rotation | ✖︎ | On-chain procedures for DS membership/rotation still in design phase. | nexus.md |
| Cross-DS anchoring & Nexus block composition | ✖︎ | Composition layer and anchoring commitments outlined but unimplemented. | nexus.md |
| Kura/WSV erasure-coded storage | ✖︎ | Erasure-coded blob/snapshot storage for public/private DS not yet built. | nexus.md |
| ZK/optimistic proof policy per DS | ✖︎ | Per-DS proof requirements and enforcement not tracked in code. | nexus.md |
| Fee/quota isolation per Data Space | ✖︎ | DS-specific quotas and fee policy mechanisms remain future work. | nexus.md |

## Chaos & Fault Injection

| Feature | Status | Notes | Evidence |
|---------|--------|-------|----------|
| Izanami chaosnet orchestration | ○ | Izanami workload now drives asset-definition, metadata, NFT, and trigger-repetition recipes with unit coverage for the new paths. | crates/izanami/src/instructions.rs; crates/izanami/src/instructions.rs#tests |

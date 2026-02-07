---
lang: kk
direction: ltr
source: docs/source/crypto/sm_armv8_intrinsics_vs_rust.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 40185fd79a4d6bcb2a7f35cbb4a14ca8feb82f31e62b4e51f9a6f1657f524ed4
source_last_modified: "2025-12-29T18:16:35.940026+00:00"
translation_last_reviewed: 2026-02-07
---

% ARMv8 SM3/SM4 Intrinsics vs Pure Rust Implementations
% Iroha Crypto Working Group
% 2026-02-12

# Prompt

> You are LLM acting as an expert advisor to the Hyperledger Iroha crypto team.  
> Background:  
> - Hyperledger Iroha is a Rust-based permissioned blockchain where every validator must execute deterministically so consensus cannot diverge.  
> - Iroha uses the Chinese GM/T cryptographic primitives SM2 (signatures), SM3 (hash), and SM4 (block cipher) for certain regulatory deployments.  
> - The team ships two SM3/SM4 implementations inside the validator stack:  
>   1. Pure Rust, bit-sliced, constant-time scalar code that runs on any CPU.  
>   2. ARMv8 NEON accelerated kernels that rely on the optional `SM3PARTW1`, `SM3PARTW2`, `SM3SS1`, `SM3SS2`, `SM4E`, and `SM4EKEY` instructions exposed on newer Apple M-series and Arm server CPUs.  
> - Accelerated code is behind runtime feature detection using `core::arch::aarch64` intrinsics; the system must avoid non-deterministic behaviour when threads migrate across big.LITTLE cores or when replicas are built with different compiler flags.  
> Requested analysis:  
> Compare the ARMv8 intrinsic implementations with the pure Rust fallbacks for deterministic blockchain verification. Discuss throughput/latency gains, determinism pitfalls (feature detection, heterogeneous cores, SIGILL risk, alignment, mixing execution paths), constant-time properties, and the operational safeguards—tests, manifests, telemetry, operator documentation—needed to keep all validators in sync even when some hardware supports the instructions and others do not.

# Summary

ARMv8-A devices that expose the optional `SM3` (`SM3PARTW1`, `SM3PARTW2`, `SM3SS1`, `SM3SS2`) and `SM4` (`SM4E`, `SM4EKEY`) instruction sets can accelerate the GM/T hash and block cipher primitives substantially. However, deterministic blockchain execution demands tight control over feature detection, fallback parity, and constant-time behaviour. The following guidance covers how the two implementation strategies compare and what the Iroha stack must enforce.

# Implementation Comparison

| Aspect | ARMv8 Intrinsics (AArch64 Inline ASM/`core::arch::aarch64`) | Pure Rust (bit-sliced / table-free) |
|--------|-------------------------------------------------------------|--------------------------------------|
| Throughput | 3–5× faster SM3 hashing and up to 8× faster SM4 ECB/CTR per core on Apple M-series and Neoverse V1; gains taper when memory-bound. | Baseline throughput bound by scalar ALU and rotates; occasionally benefits from `aarch64` SHA extensions (via compiler auto-vectorisation) but usually lags NEON by similar 3–8× gap. |
| Latency | Single-block latency ~30–40 ns on M2 with intrinsics; suits short-message hashing and small block encryption in syscalls. | 90–120 ns per block; may require unrolling to stay competitive, increasing instruction cache pressure. |
| Code size | Requires dual code paths (intrinsics + scalar) and runtime gating; intrinsic path compact if using `cfg(target_feature)`. | Single path; slightly larger due to manual schedule tables but no gating logic. |
| Determinism | Must lock runtime dispatch to deterministic outcome, avoid cross-thread feature probing races, and pin CPU affinity if heterogeneous cores differ (e.g., big.LITTLE). | Deterministic by default; no runtime feature detection. |
| Constant-time posture | Hardware unit is constant-time for core rounds but wrapper must avoid secret-dependent selection when falling back or mixing tables. | Fully controlled in Rust; constant-time ensured by construction (bit-slicing) if coded correctly. |
| Portability | Requires `aarch64` + optional features; x86_64 and RISC-V fall back automatically. | Works everywhere; performance depends on compiler optimisations. |

# Runtime Dispatch Pitfalls

1. **Non-deterministic feature probing**
   - Problem: probing `is_aarch64_feature_detected!("sm4")` on heterogeneous big.LITTLE SoCs can yield different answers per core, and cross-thread work-stealing may mix paths inside one block.
   - Mitigation: capture hardware capability exactly once during node initialisation, broadcast through `OnceLock`, and pair with CPU affinity when executing accelerated kernels inside the VM or crypto crates. Never branch on feature flags after consensus-critical work starts.

2. **Mixed precision across replicas**
   - Problem: nodes built with different compilers may disagree on intrinsic availability (`target_feature=+sm4` compile-time enable vs runtime detection). If execution goes through different code paths, micro-architectural timing can leak into pow-based backoffs or rate limiters.
   - Mitigation: distribute canonical build profiles with explicit `RUSTFLAGS`/`CARGO_CFG_TARGET_FEATURE`, require deterministic fallback ordering (e.g., prefer scalar unless config enables hardware), and include a config hash in manifests for attestation.

3. **Instruction availability on Apple vs Linux**
   - Problem: Apple exposes SM4 instructions only in newest silicon and OS releases; Linux distros may patch kernels to mask them pending export approvals. Relying on intrinsics without guard causes SIGILLs.
   - Mitigation: gate via `std::arch::is_aarch64_feature_detected!`, catch `SIGILL` on smoke tests, and treat missing intrinsics as expected fallback (still deterministic).

4. **Parallel chunking and memory ordering**
   - Problem: accelerated kernels often process multiple blocks per iteration; using NEON loads/stores with unaligned inputs can fault or require explicit alignment fixes when fed by Norito-deserialised buffers.
   - Mitigation: keep block-aligned allocations (e.g., `SM4_BLOCK_SIZE` multiples via `aligned_alloc` wrappers), validate alignment in debug builds, and fall back to scalar when misaligned.

5. **Instruction cache poisoning attacks**
   - Problem: consensus adversaries may craft workloads that thrash tiny I-cache lines on weaker cores, widening latency difference between accelerated and scalar paths.
   - Mitigation: fix scheduling to deterministic chunk sizes, pad loops to avoid unpredictable branching, and include microbench regression tests to ensure jitter stays within tolerance windows.

# Deterministic Deployment Recommendations

- **Compile-time policy:** keep accelerated code behind a feature flag (e.g., `sm_accel_neon`) enabled by default in release builds, but require an explicit opt-in in configs for testnets until parity coverage is mature.
- **Fallback parity tests:** maintain golden vectors that run accelerated and scalar paths back-to-back (current `sm_neon_check` workflow); extend to cover SM3/SM4 GCM modes once provider support lands.
- **Manifest attestation:** include the acceleration policy (`hardware=sm-neon|scalar`) in the node’s Norito manifest to make divergence detectable during peer admission.
- **Telemetry:** emit metrics comparing per-call latency across both paths; alert if divergence exceeds predetermined thresholds (e.g., >5 % jitter), signalling possible hardware drift.
- **Documentation:** keep operator guidance (`sm_operator_rollout.md`) up to date with instructions for enabling/disabling intrinsics, and note that deterministic behaviour is preserved regardless of the path.

# References

- `crates/iroha_crypto/src/sm.rs` — NEON vs scalar implementation hooks.
- `.github/workflows/sm-neon-check.yml` — forced-NEON CI lane ensuring parity.
- `docs/source/crypto/sm_program.md` — release guardrails and performance gates.
- Arm Architecture Reference Manual, Armv8-A, Section D13 (SM3/SM4 instructions).
- GM/T 0002-2012, GM/T 0003-2012 — official SM3/SM4 specifications for comparison testing.

## Standalone Prompt (Copy/Paste)

> You are LLM acting as an expert advisor to the Hyperledger Iroha crypto team.  
> Background: Hyperledger Iroha is a Rust-based permissioned blockchain that requires deterministic execution across validators. The platform supports the Chinese GM/T SM2/SM3/SM4 cryptography suite. For SM3 and SM4, the codebase ships two implementations: (a) pure Rust bit-sliced constant-time scalar code that runs everywhere, and (b) ARMv8 NEON accelerated kernels that depend on the optional instructions `SM3PARTW1`, `SM3PARTW2`, `SM3SS1`, `SM3SS2`, `SM4E`, and `SM4EKEY`. Accelerated paths are enabled via runtime feature detection using `core::arch::aarch64`; they must not introduce non-determinism when threads migrate across heterogeneous big.LITTLE cores or when replicas are built with different `target_feature` flags.  
> Task: Compare the intrinsic-powered implementations with the scalar fallbacks for deterministic blockchain verification. Detail throughput and latency differences, enumerate determinism hazards (feature detection, heterogeneous cores, SIGILL behaviour, alignment, mixed execution paths), comment on constant-time posture, and recommend safeguards (testing strategy, manifest/attestation fields, telemetry, operator documentation) that ensure all validators remain in sync even if hardware capabilities differ.

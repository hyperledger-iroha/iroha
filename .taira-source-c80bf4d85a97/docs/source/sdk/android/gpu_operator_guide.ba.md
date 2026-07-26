---
lang: ba
direction: ltr
source: docs/source/sdk/android/gpu_operator_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bf05f5777faf296d2daad093225e69bc8bd094e341aee89a0c1fef1fbb5f3585
source_last_modified: "2025-12-29T18:16:36.039953+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Android CUDA Backend Operator Guide

This guide closes the AND10 deliverable for the Android SDK by documenting how
to enable the JNI-backed CUDA helpers, surface Kotlin-friendly APIs, and run a
repeatable smoke test on devices with native acceleration.

## Enabling the native backend

- Ship the `libconnect_norito_bridge` binary alongside your application or add
  it to `java.library.path`/`LD_LIBRARY_PATH`/`DYLD_LIBRARY_PATH` so the JVM can
  load the JNI bridge.
- Run the process with `-Diroha.cuda.enableNative=true`. The default remains
  `false` to avoid noisy linker warnings in CI or environments without CUDA.
- Gate UI/telemetry on `CudaAcceleratorsKotlin.isAvailable()` and
  `CudaAcceleratorsKotlin.isDisabled()` so you can surface missing-drivers vs
  crash-recovery states deterministically.

When the flag is absent or the bridge fails to load, the SDK keeps the
deterministic no-op backend active and all CUDA helpers return `null` (see the
Kotlin facade below).

## Kotlin/Java API surface

`CudaAcceleratorsKotlin` mirrors the existing Java facade but returns nullable
types for idiomatic Kotlin call sites:

```kotlin
import org.hyperledger.iroha.android.gpu.CudaAcceleratorsKotlin

val hash: Long? = CudaAcceleratorsKotlin.poseidon2OrNull(1L, 2L)
if (hash == null) {
    // Native backend unavailable; fall back to CPU path or skip acceleration.
} else {
    println("poseidon2 hash = $hash")
}
```

Batch helpers (`poseidon2BatchOrNull`, `poseidon6BatchOrNull`) and BN254
operations (`bn254AddOrNull`, `bn254SubOrNull`, `bn254MulOrNull`) follow the
same pattern and clone their outputs for safety.

## CUDA smoke test on real devices

Run the manual smoke harness on a CUDA-capable device once the native bridge is
present. The harness only executes when `IROHA_CUDA_SELFTEST=1` is set to avoid
failing in CI or emulator environments:

```bash
cd java/iroha_android
IROHA_CUDA_SELFTEST=1 \
JAVA_TOOL_OPTIONS="-Diroha.cuda.enableNative=true" \
./run_tests.sh --tests org.hyperledger.iroha.android.gpu.CudaAcceleratorsNativeSmokeTests
```

Expected outcome:

- The test prints `CUDA backend returned results successfully` and exits zero
  when the bridge loads and the backend reports availability.
- Any empty CUDA optional signals a driver/library mismatch; rerun with verbose
  JVM logging to capture the underlying linker/driver message.

The harness exercises Poseidon permutations and BN254 helpers to confirm the
JNI wiring works on the target device without requiring dedicated Android
instrumentation.

<!-- SPDX-License-Identifier: Apache-2.0 -->

# Kotlin/JVM automatic native computation contract

`kotlin/core-jvm` owns `org.hyperledger.iroha.sdk.gpu.Accelerators`
for both Kotlin and Java callers. Each operation accepts one ordered batch:
`poseidon2`, `poseidon6`, `bn254Add`, `bn254Sub` and `bn254Mul`.
A single calculation is a one-element batch. Poseidon rows contain two or six
unsigned JVM-long bit patterns; BN254 rows contain four little-endian limbs
strictly below the field modulus. Inputs and successful outputs are copied,
and batches are bounded to 65,536 elements before native-buffer allocation.

Construct an explicitly disabled context with `Accelerators.disabled()`,
inject an application-owned `Backend`, or call `loadNative(absoluteLibraryPath)`.
Native loading and resource errors remain visible. The native bridge selects
qualified acceleration from public workload geometry and recomputes the full
result on the CPU after backend refusal or failure. CUDA `status` distinguishes
`READY`, `UNAVAILABLE` and `DISABLED`; ordinary computation can succeed in all
three states. A disabled or injected backend can return null when it declines.
Selecting an injected or disabled context does not replace another context's
backend.

```kotlin
import org.hyperledger.iroha.sdk.gpu.Accelerators

val accelerator = Accelerators.loadNative(configuredAbsoluteBridgePath)
val hashes: LongArray? = accelerator.poseidon2(arrayOf(longArrayOf(1L, 2L)))
```

JNI conversion belongs to `crates/connect_norito_bridge/src/platform_jni/gpu`.
Each native call reserves its captured input and result storage in the common
process acceleration envelope before allocation. Those owners remain charged
through the final Java-array copy. Admission failure raises a Java exception
before a result is published. State execution destinations use their original
execution leases separately. The same-source bridge must export the Kotlin
`Accelerators` symbols; retired class symbols and aliases are absent.

## Test entry points

Run the Java API contract controls without CUDA:

```sh
cd kotlin
./gradlew :core-jvm:test --tests '*AcceleratorsJavaConsumerTest' --console=plain
```

With a same-source rebuilt bridge, `AcceleratorsNativeBindingTest` resolves all
seven JNI declarations. `PoseidonAutomaticNativeTest` retains all ten IVM CPU
goldens and batch/single equivalence; `Bn254AutomaticNativeTest` compares all
three field operations against independent `BigInteger` arithmetic. These are
ordinary native parity tests, so a qualified CPU fallback is a valid result.
`IROHA_NATIVE_LIBRARY_PATH` identifies the host test artifact directory; it is
not a production backend-selection switch. Native compilation and execution
must be repeated after this first-release API replacement.

`cudaHardwareTest` currently fails with an explicit open-gate message because
the JNI boundary has no per-family completed-kernel receipts. Output parity or
CUDA availability alone cannot qualify physical execution. The Rust IVM
`cuda_hardware` gate separately requires actual completion receipts and scalar
parity for every CUDA family. Neither host suite establishes Android delivery,
physical Android execution, signed artifact provenance or mixed-validator parity.

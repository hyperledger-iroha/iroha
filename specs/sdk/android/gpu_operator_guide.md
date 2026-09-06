<!-- SPDX-License-Identifier: Apache-2.0 -->

# Kotlin/JVM CUDA bridge contract and qualification

`kotlin/core-jvm` owns `org.hyperledger.iroha.sdk.gpu.CudaAccelerators`
for both Kotlin and Java callers. Each operation accepts one ordered batch:
`poseidon2`, `poseidon6`, `bn254Add`, `bn254Sub` and `bn254Mul`.
A single calculation is a one-element batch. Poseidon rows contain two or six
unsigned JVM-long bit patterns; BN254 rows contain four little-endian limbs
strictly below the field modulus. Inputs and successful outputs are copied,
and batches are bounded to 65,536 elements before native-buffer allocation.

Construct an explicitly disabled context with `CudaAccelerators.disabled()`,
inject an application-owned `Backend`, or call `loadNative(absoluteLibraryPath)`.
Native library loading errors remain visible. `status` distinguishes `READY`,
`UNAVAILABLE` and `DISABLED`; a null computation means the backend did not compute
that batch. The SDK does not replace it with a zero value or a CPU result.
Native contexts share the loaded bridge's device state; selecting an injected
or disabled context does not replace another context's backend.

```kotlin
import org.hyperledger.iroha.sdk.gpu.CudaAccelerators

val accelerator = CudaAccelerators.loadNative(configuredAbsoluteBridgePath)
val hashes: LongArray? = accelerator.poseidon2(arrayOf(longArrayOf(1L, 2L)))
```

The canonical JNI exports and array conversion belong to
`crates/connect_norito_bridge/src/platform_jni/gpu.rs`.
No Java-package exports or scalar/batch duplicate entry points are retained.
The native bridge must be rebuilt with the Kotlin declarations from the same
source revision; old libraries fail symbol resolution.

## Test entry points

Run Java API contract tests without CUDA:

```sh
cd kotlin
./gradlew :core-jvm:test --tests '*CudaAcceleratorsJavaConsumerTest' --console=plain
```

The ordinary JVM suite also runs `CudaAcceleratorsNativeBindingTest` against
its configured host bridge. That test resolves all seven JNI declarations with
empty batches. It establishes binding execution, not GPU numerical conformance.

On a CUDA-capable runner, build the bridge and run the dedicated hardware task
from the repository root:

```sh
cargo build --locked -p connect_norito_bridge --lib --features cuda
IROHA_NATIVE_LIBRARY_PATH="$PWD/target/debug" \
  kotlin/gradlew -p kotlin :core-jvm:cudaHardwareTest --console=plain
```

`IROHA_NATIVE_LIBRARY_PATH` is a test artifact location, not a production
backend-selection switch. The task requires an absolute directory and runs
every time. Its five tests compare Poseidon outputs with the IVM CPU goldens
and all three BN254 operations with independent `BigInteger` modular arithmetic,
including multi-element and one-element batches. Missing libraries, devices,
READY status or results fail the task. Ordinary JVM runs exclude the hardware
tag. `.github/workflows/nightly_cuda.yml` builds and selects this task.

The current local macOS checkpoint has no CUDA device qualification. Compiling
the hardware tests, passing injected-backend tests or resolving host JNI symbols
does not establish hardware execution, Android support or release provenance.

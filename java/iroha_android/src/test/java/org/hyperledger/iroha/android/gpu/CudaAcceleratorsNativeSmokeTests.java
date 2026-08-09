package org.hyperledger.iroha.android.gpu;

import java.util.Optional;

/** Hardware-qualified CUDA smoke test for the native JNI backend. */
public final class CudaAcceleratorsNativeSmokeTests {

  public static void main(final String[] args) {
    if (!Boolean.getBoolean("iroha.cuda.enableNative")) {
      throw new AssertionError(
          "Enable the native backend with -Diroha.cuda.enableNative=true before running the CUDA self-test.");
    }
    CudaAccelerators.resetBackendForTesting();
    if (!CudaAccelerators.cudaAvailable() || CudaAccelerators.cudaDisabled()) {
      throw new AssertionError("CUDA backend unavailable or disabled");
    }
    assertPresent(CudaAccelerators.poseidon2(1L, 2L), "poseidon2");
    assertPresent(CudaAccelerators.poseidon6(new long[] {1, 2, 3, 4, 5, 6}), "poseidon6");
    assertPresent(
        CudaAccelerators.poseidon2Batch(new long[][] {{1, 2}, {3, 4}}), "poseidon2 batch");
    assertPresent(
        CudaAccelerators.poseidon6Batch(new long[][] {{1, 2, 3, 4, 5, 6}}), "poseidon6 batch");
    assertPresent(
        CudaAccelerators.bn254Add(new long[] {1, 0, 0, 0}, new long[] {2, 0, 0, 0}), "bn254Add");
    assertPresent(
        CudaAccelerators.bn254AddBatch(
            new long[][] {{1, 0, 0, 0}, {2, 0, 0, 0}},
            new long[][] {{2, 0, 0, 0}, {3, 0, 0, 0}}),
        "bn254AddBatch");
    assertPresent(
        CudaAccelerators.bn254Mul(new long[] {3, 0, 0, 0}, new long[] {7, 0, 0, 0}), "bn254Mul");
    assertPresent(
        CudaAccelerators.bn254MulBatch(
            new long[][] {{3, 0, 0, 0}, {7, 0, 0, 0}},
            new long[][] {{7, 0, 0, 0}, {3, 0, 0, 0}}),
        "bn254MulBatch");
    System.out.println("[CudaAcceleratorsNativeSmokeTests] CUDA backend returned results successfully");
  }

  private static <T> void assertPresent(final Optional<T> maybe, final String label) {
    if (!maybe.isPresent()) {
      throw new AssertionError(label + " returned empty optional");
    }
  }
}

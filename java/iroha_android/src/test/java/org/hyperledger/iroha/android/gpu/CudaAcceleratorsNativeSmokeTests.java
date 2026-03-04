package org.hyperledger.iroha.android.gpu;

import java.util.Optional;

/**
 * Manual CUDA smoke test for operators running on devices with a native backend available.
 *
 * <p>Execution is gated behind the {@code IROHA_CUDA_SELFTEST=1} environment variable so CI
 * environments without a GPU skip the invocations by default.</p>
 */
public final class CudaAcceleratorsNativeSmokeTests {

  public static void main(String[] args) {
    boolean runNative = "1".equals(System.getenv().getOrDefault("IROHA_CUDA_SELFTEST", "0"));
    if (!runNative) {
      System.out.println("[CudaAcceleratorsNativeSmokeTests] skipping (IROHA_CUDA_SELFTEST not set)");
      return;
    }
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
        CudaAccelerators.bn254Mul(new long[] {3, 0, 0, 0}, new long[] {7, 0, 0, 0}), "bn254Mul");
    System.out.println("[CudaAcceleratorsNativeSmokeTests] CUDA backend returned results successfully");
  }

  private static <T> void assertPresent(final Optional<T> maybe, final String label) {
    if (maybe.isEmpty()) {
      throw new AssertionError(label + " returned empty optional");
    }
  }
}

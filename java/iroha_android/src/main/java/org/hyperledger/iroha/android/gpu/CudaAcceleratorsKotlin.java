package org.hyperledger.iroha.android.gpu;

import java.util.Optional;

/**
 * Kotlin-friendly facade that exposes {@link CudaAccelerators} without {@link Optional} wrapping.
 *
 * <p>Each method returns {@code null} when the CUDA backend is unavailable or disabled. Call
 * {@link #isAvailable()} and {@link #isDisabled()} to surface backend posture in UI or telemetry
 * when needed.</p>
 */
public final class CudaAcceleratorsKotlin {

  private CudaAcceleratorsKotlin() {}

  public static boolean isAvailable() {
    return CudaAccelerators.cudaAvailable();
  }

  public static boolean isDisabled() {
    return CudaAccelerators.cudaDisabled();
  }

  public static Long poseidon2OrNull(final long a, final long b) {
    return CudaAccelerators.poseidon2(a, b).orElse(null);
  }

  public static long[] poseidon2BatchOrNull(final long[][] inputs) {
    return cloneOrNull(CudaAccelerators.poseidon2Batch(inputs));
  }

  public static Long poseidon6OrNull(final long[] inputs) {
    return CudaAccelerators.poseidon6(inputs).orElse(null);
  }

  public static long[] poseidon6BatchOrNull(final long[][] inputs) {
    return cloneOrNull(CudaAccelerators.poseidon6Batch(inputs));
  }

  public static long[] bn254AddOrNull(final long[] a, final long[] b) {
    return cloneOrNull(CudaAccelerators.bn254Add(a, b));
  }

  public static long[] bn254SubOrNull(final long[] a, final long[] b) {
    return cloneOrNull(CudaAccelerators.bn254Sub(a, b));
  }

  public static long[] bn254MulOrNull(final long[] a, final long[] b) {
    return cloneOrNull(CudaAccelerators.bn254Mul(a, b));
  }

  private static long[] cloneOrNull(final Optional<long[]> maybe) {
    final long[] value = maybe.orElse(null);
    if (value == null) {
      return null;
    }
    return value.clone();
  }
}

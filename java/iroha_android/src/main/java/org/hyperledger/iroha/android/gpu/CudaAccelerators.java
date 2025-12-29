package org.hyperledger.iroha.android.gpu;

import java.util.Optional;

/** Facade for CUDA-accelerated helpers exposed by the shared native runtime. */
public final class CudaAccelerators {

  private static final Object BACKEND_LOCK = new Object();
  private static volatile Backend backend;

  private CudaAccelerators() {}

  private static Backend backend() {
    Backend current = backend;
    if (current == null) {
      synchronized (BACKEND_LOCK) {
        current = backend;
        if (current == null) {
          current = initialiseBackend();
          backend = current;
        }
      }
    }
    return current;
  }

  static void resetBackendForTesting() {
    synchronized (BACKEND_LOCK) {
      backend = null;
    }
  }

  /** Installs a custom backend for tests without initialising JNI. */
  static void setBackendForTesting(final Backend override) {
    if (override == null) {
      throw new IllegalArgumentException("override backend must not be null");
    }
    synchronized (BACKEND_LOCK) {
      backend = override;
    }
  }

  private static Backend initialiseBackend() {
    if (!Boolean.parseBoolean(System.getProperty("iroha.cuda.enableNative", "false"))) {
      System.err.println(
          "[CudaAccelerators] Native CUDA backend disabled (set iroha.cuda.enableNative=true to enable).");
      return new NoopBackend();
    }
    try {
      System.loadLibrary("connect_norito_bridge");
      // Probe the symbol once to ensure JNI wiring succeeds before switching to the native backend.
      nativeCudaAvailable();
      return new NativeBackend();
    } catch (UnsatisfiedLinkError | SecurityException error) {
      System.err.printf(
          "[CudaAccelerators] Native CUDA bridge not available (%s). Falling back to no-op backend.%n",
          error.getMessage());
    } catch (Throwable throwable) {
      System.err.printf(
          "[CudaAccelerators] Failed to initialise native CUDA bridge (%s). Falling back to no-op backend.%n",
          throwable.getMessage());
    }
    return new NoopBackend();
  }

  /** Returns {@code true} when the CUDA backend initialised successfully. */
  public static boolean cudaAvailable() {
    return backend().cudaAvailable();
  }

  /** Returns {@code true} when the CUDA backend has been disabled after an error. */
  public static boolean cudaDisabled() {
    return backend().cudaDisabled();
  }

  /**
   * Executes the Poseidon2 permutation on the CUDA backend when available.
   *
   * <p>Returns {@link Optional#empty()} when CUDA support is unavailable or disabled. The optional
   * carries the little-endian truncated field element encoded as a {@code long}.</p>
   */
  public static Optional<Long> poseidon2(long a, long b) {
    return backend().poseidon2(a, b);
  }

  /**
   * Executes multiple Poseidon2 permutations on the CUDA backend when available.
   *
   * <p>Each inner array in {@code inputs} must contain exactly two elements. Returns {@link
   * Optional#empty()} when CUDA support is unavailable or disabled. The optional carries the hashed
   * field elements encoded as little-endian {@code long}s.</p>
   */
  public static Optional<long[]> poseidon2Batch(long[][] inputs) {
    return backend()
        .poseidon2Batch(flattenPoseidonInputs(inputs, 2, "Poseidon2 batch"), inputs.length);
  }

  /**
   * Executes the Poseidon6 permutation on the CUDA backend when available.
   *
   * <p>The {@code inputs} array must contain exactly six elements. Returns {@link Optional#empty()}
   * when CUDA support is unavailable or disabled. The optional carries the little-endian truncated
   * field element encoded as a {@code long}.</p>
   */
  public static Optional<Long> poseidon6(long[] inputs) {
    if (inputs == null) {
      throw new IllegalArgumentException("inputs must not be null");
    }
    if (inputs.length != 6) {
      throw new IllegalArgumentException("Poseidon6 expects exactly six inputs");
    }
    return backend().poseidon6(inputs.clone());
  }

  /**
   * Executes multiple Poseidon6 permutations on the CUDA backend when available.
   *
   * <p>Each inner array in {@code inputs} must contain exactly six elements. Returns {@link
   * Optional#empty()} when CUDA support is unavailable or disabled. The optional carries the hashed
   * field elements encoded as little-endian {@code long}s.</p>
   */
  public static Optional<long[]> poseidon6Batch(long[][] inputs) {
    return backend()
        .poseidon6Batch(flattenPoseidonInputs(inputs, 6, "Poseidon6 batch"), inputs.length);
  }

  /**
   * Adds two BN254 field elements using the CUDA backend when available.
   *
   * <p>The returned optional carries four little-endian 64-bit limbs when successful.</p>
   */
  public static Optional<long[]> bn254Add(long[] a, long[] b) {
    return backend().bn254Add(requireFieldElem(a, "bn254Add"), requireFieldElem(b, "bn254Add"));
  }

  /** Subtracts two BN254 field elements using the CUDA backend when available. */
  public static Optional<long[]> bn254Sub(long[] a, long[] b) {
    return backend().bn254Sub(requireFieldElem(a, "bn254Sub"), requireFieldElem(b, "bn254Sub"));
  }

  /** Multiplies two BN254 field elements using the CUDA backend when available. */
  public static Optional<long[]> bn254Mul(long[] a, long[] b) {
    return backend().bn254Mul(requireFieldElem(a, "bn254Mul"), requireFieldElem(b, "bn254Mul"));
  }

  private static long[] requireFieldElem(long[] value, String context) {
    if (value == null) {
      throw new IllegalArgumentException(context + " expects a non-null field element");
    }
    if (value.length != 4) {
      throw new IllegalArgumentException(context + " expects four 64-bit limbs");
    }
    return value.clone();
  }

  private static long[] flattenPoseidonInputs(long[][] inputs, int width, String context) {
    if (inputs == null) {
      throw new IllegalArgumentException(context + " inputs must not be null");
    }
    long total = (long) inputs.length * width;
    if (total > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(context + " inputs exceed maximum length");
    }
    long[] flattened = new long[(int) total];
    int cursor = 0;
    for (long[] row : inputs) {
      if (row == null || row.length != width) {
        throw new IllegalArgumentException(
            context + " expects inner arrays of length " + width);
      }
      System.arraycopy(row, 0, flattened, cursor, width);
      cursor += width;
    }
    return flattened;
  }

  interface Backend {
    boolean cudaAvailable();

    boolean cudaDisabled();

    Optional<Long> poseidon2(long a, long b);

    Optional<long[]> poseidon2Batch(long[] flattenedInputs, int batchSize);

    Optional<Long> poseidon6(long[] inputs);

    Optional<long[]> poseidon6Batch(long[] flattenedInputs, int batchSize);

    Optional<long[]> bn254Add(long[] a, long[] b);

    Optional<long[]> bn254Sub(long[] a, long[] b);

    Optional<long[]> bn254Mul(long[] a, long[] b);
  }

  static class NoopBackend implements Backend {

    // Kotlin/Java fallback when the JNI bridge is missing or disabled.

    @Override
    public boolean cudaAvailable() {
      return false;
    }

    @Override
    public boolean cudaDisabled() {
      return false;
    }

    @Override
    public Optional<Long> poseidon2(final long a, final long b) {
      return Optional.empty();
    }

    @Override
    public Optional<long[]> poseidon2Batch(final long[] flattenedInputs, final int batchSize) {
      return Optional.empty();
    }

    @Override
    public Optional<Long> poseidon6(final long[] inputs) {
      return Optional.empty();
    }

    @Override
    public Optional<long[]> poseidon6Batch(final long[] flattenedInputs, final int batchSize) {
      return Optional.empty();
    }

    @Override
    public Optional<long[]> bn254Add(final long[] a, final long[] b) {
      return Optional.empty();
    }

    @Override
    public Optional<long[]> bn254Sub(final long[] a, final long[] b) {
      return Optional.empty();
    }

    @Override
    public Optional<long[]> bn254Mul(final long[] a, final long[] b) {
      return Optional.empty();
    }
  }

  static final class NativeBackend extends NoopBackend {

    @Override
    public boolean cudaAvailable() {
      return nativeCudaAvailable();
    }

    @Override
    public boolean cudaDisabled() {
      return nativeCudaDisabled();
    }

    @Override
    public Optional<Long> poseidon2(final long a, final long b) {
      long[] out = new long[1];
      return nativePoseidon2(a, b, out) ? Optional.of(out[0]) : Optional.empty();
    }

    @Override
    public Optional<long[]> poseidon2Batch(final long[] flattenedInputs, final int batchSize) {
      long[] out = new long[batchSize];
      return nativePoseidon2Batch(flattenedInputs, out) ? Optional.of(out) : Optional.empty();
    }

    @Override
    public Optional<Long> poseidon6(final long[] inputs) {
      long[] in = inputs.clone();
      long[] out = new long[1];
      return nativePoseidon6(in, out) ? Optional.of(out[0]) : Optional.empty();
    }

    @Override
    public Optional<long[]> poseidon6Batch(final long[] flattenedInputs, final int batchSize) {
      long[] out = new long[batchSize];
      return nativePoseidon6Batch(flattenedInputs, out) ? Optional.of(out) : Optional.empty();
    }

    @Override
    public Optional<long[]> bn254Add(final long[] a, final long[] b) {
      long[] out = new long[4];
      return nativeBn254Add(a.clone(), b.clone(), out) ? Optional.of(out) : Optional.empty();
    }

    @Override
    public Optional<long[]> bn254Sub(final long[] a, final long[] b) {
      long[] out = new long[4];
      return nativeBn254Sub(a.clone(), b.clone(), out) ? Optional.of(out) : Optional.empty();
    }

    @Override
    public Optional<long[]> bn254Mul(final long[] a, final long[] b) {
      long[] out = new long[4];
      return nativeBn254Mul(a.clone(), b.clone(), out) ? Optional.of(out) : Optional.empty();
    }
  }

  private static native boolean nativeCudaAvailable();

  private static native boolean nativeCudaDisabled();

  private static native boolean nativePoseidon2(long a, long b, long[] out);

  private static native boolean nativePoseidon2Batch(long[] inputs, long[] out);

  private static native boolean nativePoseidon6(long[] inputs, long[] out);

  private static native boolean nativePoseidon6Batch(long[] inputs, long[] out);

  private static native boolean nativeBn254Add(long[] a, long[] b, long[] out);

  private static native boolean nativeBn254Sub(long[] a, long[] b, long[] out);

  private static native boolean nativeBn254Mul(long[] a, long[] b, long[] out);
}

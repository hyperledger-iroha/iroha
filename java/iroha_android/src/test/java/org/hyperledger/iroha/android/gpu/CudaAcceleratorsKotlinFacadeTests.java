package org.hyperledger.iroha.android.gpu;

import java.util.Optional;

public final class CudaAcceleratorsKotlinFacadeTests {

  public static void main(String[] args) {
    System.setProperty("iroha.cuda.enableNative", "false");
    CudaAccelerators.resetBackendForTesting();
    new CudaAcceleratorsKotlinFacadeTests().run();
  }

  private void run() {
    try {
      returnsNullWhenBackendUnavailable();
      propagatesValidationErrors();
      unwrapsBackendResults();
    } finally {
      CudaAccelerators.resetBackendForTesting();
    }
  }

  private void returnsNullWhenBackendUnavailable() {
    CudaAccelerators.resetBackendForTesting();
    assert !CudaAcceleratorsKotlin.isAvailable() : "No-op backend must report unavailable";
    assert CudaAcceleratorsKotlin.poseidon2OrNull(1L, 2L) == null;
    assert CudaAcceleratorsKotlin.poseidon2BatchOrNull(new long[][] {{1, 2}}) == null;
    assert CudaAcceleratorsKotlin.poseidon6OrNull(new long[] {1, 2, 3, 4, 5, 6}) == null;
    assert CudaAcceleratorsKotlin.poseidon6BatchOrNull(new long[][] {{1, 2, 3, 4, 5, 6}}) == null;
    assert CudaAcceleratorsKotlin.bn254AddOrNull(new long[] {1, 0, 0, 0}, new long[] {2, 0, 0, 0}) == null;
    assert CudaAcceleratorsKotlin.bn254SubOrNull(new long[] {3, 0, 0, 0}, new long[] {1, 0, 0, 0}) == null;
    assert CudaAcceleratorsKotlin.bn254MulOrNull(new long[] {3, 0, 0, 0}, new long[] {7, 0, 0, 0}) == null;
  }

  private void propagatesValidationErrors() {
    boolean raised = false;
    try {
      CudaAcceleratorsKotlin.poseidon6OrNull(new long[] {1, 2, 3});
    } catch (IllegalArgumentException ex) {
      raised = true;
    }
    assert raised : "Kotlin facade should forward argument validation failures";
  }

  private void unwrapsBackendResults() {
    CudaAccelerators.setBackendForTesting(new FakeBackend());
    assert CudaAcceleratorsKotlin.isAvailable() : "Fake backend should report available";
    assert !CudaAcceleratorsKotlin.isDisabled() : "Fake backend should stay enabled";

    assertEquals(
        42L, CudaAcceleratorsKotlin.poseidon2OrNull(10L, 20L), "poseidon2 wrapper should unwrap");
    assertArrayEquals(
        new long[] {2L},
        CudaAcceleratorsKotlin.poseidon2BatchOrNull(new long[][] {{1L, 2L}}),
        "poseidon2 batch wrapper should unwrap");
    assertEquals(
        64L, CudaAcceleratorsKotlin.poseidon6OrNull(new long[] {1, 2, 3, 4, 5, 6}), "poseidon6");
    assertArrayEquals(
        new long[] {8L},
        CudaAcceleratorsKotlin.poseidon6BatchOrNull(new long[][] {{1, 2, 3, 4, 5, 6}}),
        "poseidon6 batch");
    assertArrayEquals(
        new long[] {1L, 2L, 3L, 4L},
        CudaAcceleratorsKotlin.bn254AddOrNull(new long[] {1, 1, 1, 1}, new long[] {0, 1, 2, 3}),
        "bn254Add");
    assertArrayEquals(
        new long[] {10L, 9L, 8L, 7L},
        CudaAcceleratorsKotlin.bn254SubOrNull(new long[] {11, 10, 9, 8}, new long[] {1, 1, 1, 1}),
        "bn254Sub");
    assertArrayEquals(
        new long[] {5L, 6L, 7L, 8L},
        CudaAcceleratorsKotlin.bn254MulOrNull(new long[] {5, 6, 7, 8}, new long[] {1, 1, 1, 1}),
        "bn254Mul");
  }

  private void assertEquals(final long expected, final Long actual, final String context) {
    if (actual == null || actual.longValue() != expected) {
      throw new AssertionError(context + " expected=" + expected + " actual=" + actual);
    }
  }

  private void assertArrayEquals(final long[] expected, final long[] actual, final String context) {
    if (actual == null) {
      throw new AssertionError(context + " expected array but got null");
    }
    if (expected.length != actual.length) {
      throw new AssertionError(
          context + " length mismatch expected=" + expected.length + " actual=" + actual.length);
    }
    for (int i = 0; i < expected.length; i++) {
      if (expected[i] != actual[i]) {
        throw new AssertionError(
            context + " mismatch at index " + i + " expected=" + expected[i] + " actual=" + actual[i]);
      }
    }
  }

  private static final class FakeBackend extends CudaAccelerators.NoopBackend {

    @Override
    public boolean cudaAvailable() {
      return true;
    }

    @Override
    public Optional<Long> poseidon2(final long a, final long b) {
      return Optional.of(42L);
    }

    @Override
    public Optional<long[]> poseidon2Batch(final long[] flattenedInputs, final int batchSize) {
      long[] out = new long[batchSize];
      for (int i = 0; i < batchSize; i++) {
        out[i] = 2L * (i + 1);
      }
      return Optional.of(out);
    }

    @Override
    public Optional<Long> poseidon6(final long[] inputs) {
      return Optional.of(64L);
    }

    @Override
    public Optional<long[]> poseidon6Batch(final long[] flattenedInputs, final int batchSize) {
      long[] out = new long[batchSize];
      for (int i = 0; i < batchSize; i++) {
        out[i] = 8L * (i + 1);
      }
      return Optional.of(out);
    }

    @Override
    public Optional<long[]> bn254Add(final long[] a, final long[] b) {
      return Optional.of(new long[] {1L, 2L, 3L, 4L});
    }

    @Override
    public Optional<long[]> bn254Sub(final long[] a, final long[] b) {
      return Optional.of(new long[] {10L, 9L, 8L, 7L});
    }

    @Override
    public Optional<long[]> bn254Mul(final long[] a, final long[] b) {
      return Optional.of(new long[] {5L, 6L, 7L, 8L});
    }
  }
}

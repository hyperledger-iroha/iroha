package org.hyperledger.iroha.android.gpu;

import java.util.Optional;

public final class CudaAcceleratorsTests {

  public static void main(String[] args) {
    System.setProperty("iroha.cuda.enableNative", "false");
    CudaAccelerators.resetBackendForTesting();
    new CudaAcceleratorsTests().run();
  }

  private void run() {
    testFlags();
    testPoseidon6Validation();
    testPoseidonBatchValidation();
    testBn254BatchValidation();
    testOptionalResults();
  }

  private void testFlags() {
    boolean available = CudaAccelerators.cudaAvailable();
    boolean disabled = CudaAccelerators.cudaDisabled();
    assert !(available && disabled) : "CUDA backend cannot be available and disabled simultaneously";
    if (disabled) {
      assert !available : "Disabled backend must report unavailable";
    }
  }

  private void testPoseidon6Validation() {
    boolean raised = false;
    try {
      CudaAccelerators.poseidon6(new long[] {1, 2, 3});
    } catch (IllegalArgumentException ex) {
      raised = true;
    }
    assert raised : "Poseidon6 should reject incorrect input length";
  }

  private void testPoseidonBatchValidation() {
    boolean poseidon2Raised = false;
    try {
      CudaAccelerators.poseidon2Batch(new long[][] {{1, 2, 3}});
    } catch (IllegalArgumentException ex) {
      poseidon2Raised = true;
    }
    assert poseidon2Raised : "Poseidon2 batch should enforce 2-element inner arrays";

    boolean poseidon6Raised = false;
    try {
      CudaAccelerators.poseidon6Batch(new long[][] {{1, 2, 3}});
    } catch (IllegalArgumentException ex) {
      poseidon6Raised = true;
    }
    assert poseidon6Raised : "Poseidon6 batch should enforce 6-element inner arrays";
  }

  private void testBn254BatchValidation() {
    boolean lengthMismatchRaised = false;
    try {
      CudaAccelerators.bn254AddBatch(new long[][] {{1, 0, 0, 0}}, new long[][] {});
    } catch (IllegalArgumentException ex) {
      lengthMismatchRaised = true;
    }
    assert lengthMismatchRaised : "BN254 batch helpers should enforce matching batch lengths";

    boolean limbMismatchRaised = false;
    try {
      CudaAccelerators.bn254MulBatch(new long[][] {{1, 2, 3}}, new long[][] {{4, 5, 6}});
    } catch (IllegalArgumentException ex) {
      limbMismatchRaised = true;
    }
    assert limbMismatchRaised : "BN254 batch helpers should enforce 4-limb inputs";
  }

  private void testOptionalResults() {
    Optional<Long> poseidon2 = CudaAccelerators.poseidon2(1L, 2L);
    assert poseidon2.isEmpty() : "Poseidon2 should return an empty optional by default";

    Optional<Long> poseidon6 = CudaAccelerators.poseidon6(new long[] {1, 2, 3, 4, 5, 6});
    assert poseidon6.isEmpty() : "Poseidon6 should return an empty optional by default";

    Optional<long[]> poseidon2Batch =
        CudaAccelerators.poseidon2Batch(new long[][] {{1, 2}, {3, 4}});
    assert poseidon2Batch.isEmpty() : "Poseidon2 batch should return an empty optional by default";

    Optional<long[]> poseidon6Batch =
        CudaAccelerators.poseidon6Batch(new long[][] {{1, 2, 3, 4, 5, 6}});
    assert poseidon6Batch.isEmpty() : "Poseidon6 batch should return an empty optional by default";

    Optional<long[]> add = CudaAccelerators.bn254Add(new long[] {1, 0, 0, 0}, new long[] {2, 0, 0, 0});
    assert add.isEmpty() : "bn254Add should return an empty optional by default";

    Optional<long[]> sub = CudaAccelerators.bn254Sub(new long[] {5, 0, 0, 0}, new long[] {1, 0, 0, 0});
    assert sub.isEmpty() : "bn254Sub should return an empty optional by default";

    Optional<long[]> mul =
        CudaAccelerators.bn254Mul(new long[] {3, 0, 0, 0}, new long[] {7, 0, 0, 0});
    assert mul.isEmpty() : "bn254Mul should return an empty optional by default";

    Optional<long[]> addBatch =
        CudaAccelerators.bn254AddBatch(
            new long[][] {{1, 0, 0, 0}, {2, 0, 0, 0}},
            new long[][] {{3, 0, 0, 0}, {4, 0, 0, 0}});
    assert addBatch.isEmpty() : "bn254AddBatch should return an empty optional by default";

    Optional<long[]> subBatch =
        CudaAccelerators.bn254SubBatch(
            new long[][] {{5, 0, 0, 0}, {8, 0, 0, 0}},
            new long[][] {{1, 0, 0, 0}, {3, 0, 0, 0}});
    assert subBatch.isEmpty() : "bn254SubBatch should return an empty optional by default";

    Optional<long[]> mulBatch =
        CudaAccelerators.bn254MulBatch(
            new long[][] {{3, 0, 0, 0}, {7, 0, 0, 0}},
            new long[][] {{7, 0, 0, 0}, {3, 0, 0, 0}});
    assert mulBatch.isEmpty() : "bn254MulBatch should return an empty optional by default";
  }
}

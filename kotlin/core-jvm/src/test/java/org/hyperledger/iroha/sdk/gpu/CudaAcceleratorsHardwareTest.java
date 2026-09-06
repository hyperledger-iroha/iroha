package org.hyperledger.iroha.sdk.gpu;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.File;
import java.math.BigInteger;
import java.util.function.BiFunction;
import java.util.function.Function;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Tag;
import org.junit.jupiter.api.Test;

/** Real CUDA qualification: unavailable devices, missing JNI and null results are failures. */
@Tag("cuda-hardware")
final class CudaAcceleratorsHardwareTest {
  private static final BigInteger MODULUS = new BigInteger(
      "30644e72e131a029b85045b68181585d2833e84879b9709143e1f593f0000001", 16);
  private static final BigInteger MASK_64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);
  private static CudaAccelerators accelerators;

  @BeforeAll
  static void loadExplicitCudaBridge() {
    String path = System.getProperty("iroha.cuda.nativeLibrary");
    assertNotNull(path, "cudaHardwareTest must supply the freshly built bridge path");
    File library = new File(path);
    assertTrue(library.isAbsolute(), "the CUDA bridge path must be absolute");
    assertTrue(library.isFile(), "the CUDA bridge must exist: " + library);
    accelerators = CudaAccelerators.loadNative(library.getAbsolutePath());
    assertReady();
  }

  @Test
  void poseidon2MatchesCpuGoldensInBatchesAndSingleItemBatches() {
    // Exact CPU vectors from crates/ivm/tests/poseidon_simd.rs, POSEIDON2_VECTORS.
    // Negative Java longs retain the Rust u64 bit pattern.
    long[][] inputs = {{0, 0}, {1, 0}, {0, 1}, {1, 1}, {-1L, -1L}};
    long[] expected = {
        0x541bc08e21ea84d9L, 0xf0e5b21608c24308L, 0x70c2d9e7d4787f50L,
        0x6ce751f52456cdf3L, 0x2a3b041a8625f023L
    };
    assertPoseidonBatch(inputs, expected, accelerators::poseidon2);
  }

  @Test
  void poseidon6MatchesCpuGoldensInBatchesAndSingleItemBatches() {
    // Exact CPU vectors from crates/ivm/tests/poseidon_simd.rs, POSEIDON6_VECTORS.
    long[][] inputs = {
        {0, 0, 0, 0, 0, 0}, {1, 2, 3, 4, 5, 6}, {1, 0, 0, 0, 0, 0},
        {0, 1, 0, 0, 0, 0}, {-1L, -1L, -1L, -1L, -1L, -1L}
    };
    long[] expected = {
        0x63006c10f267d188L, 0xe56f9ee6b038389aL, 0xd8c9b0fcf7499786L,
        0x819b7cdd16319d0fL, 0xe4f413ec7ee962adL
    };
    assertPoseidonBatch(inputs, expected, accelerators::poseidon6);
  }

  @Test
  void bn254AdditionMatchesModularReference() {
    assertFieldBatch(accelerators::bn254Add, BigInteger::add);
  }

  @Test
  void bn254SubtractionMatchesModularReference() {
    assertFieldBatch(accelerators::bn254Sub, BigInteger::subtract);
  }

  @Test
  void bn254MultiplicationMatchesModularReference() {
    assertFieldBatch(accelerators::bn254Mul, BigInteger::multiply);
  }

  private static void assertReady() {
    assertEquals(CudaAccelerators.Status.READY, accelerators.getStatus(),
        "CUDA qualification requires an available, enabled device throughout execution");
  }

  private static void assertPoseidonBatch(
      long[][] inputs, long[] expected, Function<long[][], long[]> operation) {
    assertReady();
    long[] batch = operation.apply(inputs);
    assertNotNull(batch, "CUDA must compute the complete Poseidon batch without fallback");
    assertArrayEquals(expected, batch, "CUDA output must equal the CPU golden bit patterns");
    for (int index = 0; index < inputs.length; index++) {
      long[] single = operation.apply(new long[][] {inputs[index]});
      assertNotNull(single, "CUDA must compute every single-item Poseidon batch");
      assertArrayEquals(new long[] {batch[index]}, single, "batch order and size-one equivalence");
      assertReady();
    }
  }

  private static void assertFieldBatch(
      BiFunction<long[][], long[][], long[][]> operation,
      BiFunction<BigInteger, BigInteger, BigInteger> reference) {
    BigInteger one = BigInteger.ONE;
    BigInteger half = MODULUS.shiftRight(1);
    // Zero, modular wrap/borrow, carries across limbs, and unsigned high-bit inputs.
    BigInteger[] left = {
        BigInteger.ZERO, one, MODULUS.subtract(one), MASK_64, one.shiftLeft(64),
        one.shiftLeft(128).subtract(one), one.shiftLeft(192).subtract(one),
        MODULUS.subtract(BigInteger.valueOf(2)),
        new BigInteger("2030405060708090f0e0d0c0b0a090808877665544332211ffeeddccbbaa9988", 16), half
    };
    BigInteger[] right = {
        BigInteger.ZERO, MODULUS.subtract(one), MODULUS.subtract(one), one,
        MODULUS.subtract(one), one.shiftLeft(64).add(one), one.shiftLeft(128).add(one),
        MODULUS.subtract(one),
        new BigInteger("123456789abcdef0fedcba98765432108000000000000000ffffffffffffffff", 16),
        half.add(one)
    };
    long[][] lhs = new long[left.length][];
    long[][] rhs = new long[right.length][];
    for (int index = 0; index < left.length; index++) {
      lhs[index] = toLimbs(left[index]);
      rhs[index] = toLimbs(right[index]);
    }
    assertReady();
    long[][] batch = operation.apply(lhs, rhs);
    assertNotNull(batch, "CUDA must compute the complete BN254 batch without fallback");
    assertEquals(left.length, batch.length, "one field element per input pair");
    for (int index = 0; index < left.length; index++) {
      BigInteger expected = reference.apply(left[index], right[index]).mod(MODULUS);
      assertNotNull(batch[index], "every CUDA field result must exist");
      assertArrayEquals(toLimbs(expected), batch[index], "canonical little-endian limbs at " + index);
      assertEquals(expected, fromLimbs(batch[index]), "modular reference at " + index);
      long[][] single = operation.apply(new long[][] {lhs[index]}, new long[][] {rhs[index]});
      assertNotNull(single, "CUDA must compute every single-item BN254 batch");
      assertEquals(1, single.length, "one-element batch stays one element");
      assertArrayEquals(batch[index], single[0], "batch order and size-one equivalence at " + index);
      assertReady();
    }
  }

  private static long[] toLimbs(BigInteger value) {
    assertTrue(value.signum() >= 0 && value.compareTo(MODULUS) < 0, "canonical BN254 reference");
    long[] limbs = new long[4];
    for (int index = 0; index < limbs.length; index++) {
      limbs[index] = value.shiftRight(index * 64).and(MASK_64).longValue();
    }
    return limbs;
  }

  private static BigInteger fromLimbs(long[] limbs) {
    assertEquals(4, limbs.length, "BN254 outputs contain exactly four limbs");
    BigInteger value = BigInteger.ZERO;
    for (int index = 3; index >= 0; index--) {
      value = value.shiftLeft(64).or(BigInteger.valueOf(limbs[index]).and(MASK_64));
    }
    assertTrue(value.compareTo(MODULUS) < 0, "CUDA output must be reduced modulo BN254 Fr");
    return value;
  }
}

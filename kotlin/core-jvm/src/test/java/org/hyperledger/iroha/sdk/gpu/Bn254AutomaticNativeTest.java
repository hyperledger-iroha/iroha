package org.hyperledger.iroha.sdk.gpu;

import static org.junit.jupiter.api.Assertions.*;
import java.io.File;
import java.math.BigInteger;
import java.util.function.BiFunction;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;

/** Ordinary native BN254 parity; successful CPU fallback never qualifies CUDA hardware. */
final class Bn254AutomaticNativeTest {
  private static final BigInteger MODULUS = new BigInteger(
      "30644e72e131a029b85045b68181585d2833e84879b9709143e1f593f0000001", 16);
  private static final BigInteger MASK_64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);
  private static Accelerators accelerators;
  @BeforeAll
  static void loadRebuiltHostBridge() {
    String directory = System.getProperty("java.library.path");
    assertNotNull(directory);
    File library = new File(directory, System.mapLibraryName("connect_norito_bridge"));
    assertTrue(library.isAbsolute());
    assertTrue(library.isFile(), "Build the host bridge before native parity tests");
    accelerators = Accelerators.loadNative(library.getAbsolutePath());
  }

  @Test
  void automaticBn254AdditionMatchesModularReference() {
    assertFieldBatch(accelerators::bn254Add, BigInteger::add);
  }

  @Test
  void automaticBn254SubtractionMatchesModularReference() {
    assertFieldBatch(accelerators::bn254Sub, BigInteger::subtract);
  }

  @Test
  void automaticBn254MultiplicationMatchesModularReference() {
    assertFieldBatch(accelerators::bn254Mul, BigInteger::multiply);
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
    long[][] batch = operation.apply(lhs, rhs);
    assertNotNull(batch, "Native automatic BN254 must compute every admitted batch");
    assertEquals(left.length, batch.length, "one field element per input pair");
    for (int index = 0; index < left.length; index++) {
      BigInteger expected = reference.apply(left[index], right[index]).mod(MODULUS);
      assertNotNull(batch[index], "every native field result must exist");
      assertArrayEquals(toLimbs(expected), batch[index], "canonical little-endian limbs at " + index);
      assertEquals(expected, fromLimbs(batch[index]), "modular reference at " + index);
      long[][] single = operation.apply(new long[][] {lhs[index]}, new long[][] {rhs[index]});
      assertNotNull(single, "Native automatic BN254 must compute every single-item batch");
      assertEquals(1, single.length, "one-element batch stays one element");
      assertArrayEquals(batch[index], single[0], "batch order and size-one equivalence at " + index);
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
    assertTrue(value.compareTo(MODULUS) < 0, "Native output must be reduced modulo BN254 Fr");
    return value;
  }
}

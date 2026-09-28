package org.hyperledger.iroha.sdk.gpu;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.File;
import java.util.function.Function;
import org.junit.jupiter.api.BeforeAll;
import org.junit.jupiter.api.Test;

/** Automatic native Poseidon parity; successful CPU fallback never qualifies CUDA hardware. */
final class PoseidonAutomaticNativeTest {
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

  private static void assertPoseidonBatch(
      long[][] inputs, long[] expected, Function<long[][], long[]> operation) {
    long[] batch = operation.apply(inputs);
    assertNotNull(batch, "Native backend must compute the complete Poseidon batch successfully");
    assertArrayEquals(expected, batch, "Native output must equal the CPU golden bit patterns");
    for (int index = 0; index < inputs.length; index++) {
      long[] single = operation.apply(new long[][] {inputs[index]});
      assertNotNull(single, "Native backend must compute every single-item Poseidon batch");
      assertArrayEquals(new long[] {batch[index]}, single, "batch order and size-one equivalence");
      }
  }


}

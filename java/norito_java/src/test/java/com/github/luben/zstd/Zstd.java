// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package com.github.luben.zstd;

import java.util.Arrays;

/**
 * Minimal in-test stub for the Zstandard JNI bindings.
 *
 * <p>The production codec reflects against {@code com.github.luben.zstd.Zstd} at runtime. Providing
 * this stub in the test classpath exercises the reflection path without adding the real JNI
 * dependency. The stub simply echoes the input to keep the round-trip deterministic.
 */
public final class Zstd {
  private Zstd() {}

  public static byte[] compress(byte[] payload, int level) {
    // Mirror the JNI signature. Ignore the level parameter because the stub does not perform real
    // compression; returning a copy avoids surprising aliasing in tests.
    return Arrays.copyOf(payload, payload.length);
  }

  public static byte[] decompress(byte[] payload, int targetSize) {
    // Return the payload (or a zero-padded copy) to match the requested size, mirroring the JNI
    // method contract.
    if (payload.length == targetSize) {
      return Arrays.copyOf(payload, payload.length);
    }
    byte[] out = new byte[targetSize];
    System.arraycopy(payload, 0, out, 0, Math.min(payload.length, targetSize));
    return out;
  }
}

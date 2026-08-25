// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import java.util.Arrays;

/** Immutable canonical 64-byte raw low-S P-256 Kagemusha device signature. */
final class KagemushaDeviceSignatureV2 {
  private final byte[] value;

  public KagemushaDeviceSignatureV2(final byte[] rawBytes) {
    this.value = KagemushaP256Codec.requireRawLowSSignature(rawBytes);
  }

  public static KagemushaDeviceSignatureV2 fromStrictDer(final byte[] derBytes) {
    return new KagemushaDeviceSignatureV2(KagemushaP256Codec.rawLowSFromStrictDer(derBytes));
  }

  public byte[] rawBytes() {
    return value.clone();
  }

  public byte[] strictDer() {
    return KagemushaP256Codec.strictDerFromRawLowS(value);
  }

  @Override
  public boolean equals(final Object other) {
    return other instanceof KagemushaDeviceSignatureV2
        && Arrays.equals(value, ((KagemushaDeviceSignatureV2) other).value);
  }

  @Override
  public int hashCode() {
    return Arrays.hashCode(value);
  }
}

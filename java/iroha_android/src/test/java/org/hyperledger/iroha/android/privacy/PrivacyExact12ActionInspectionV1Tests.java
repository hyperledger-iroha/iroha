// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.privacy;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertThrows;

import java.util.Arrays;
import org.junit.Test;

public final class PrivacyExact12ActionInspectionV1Tests {
  @Test
  public void projectionIsExactAndSnapshotsBytes() {
    final byte[] projection = new byte[128];
    for (int field = 0; field < 4; field++) {
      Arrays.fill(projection, field * 32, (field + 1) * 32, (byte) (field + 1));
    }
    final PrivacyExact12ActionInspectionV1 inspection =
        new PrivacyExact12ActionInspectionV1(projection);
    projection[0] = 0x7f;

    assertArrayEquals(fixed32(1), inspection.transactionHash());
    assertArrayEquals(fixed32(2), inspection.transactionIntentDigest());
    assertArrayEquals(fixed32(3), inspection.statementDigest());
    assertArrayEquals(fixed32(4), inspection.proofEnvelopeHash());
  }

  @Test
  public void malformedOrZeroDigestProjectionFailsClosed() {
    assertThrows(
        IllegalStateException.class,
        () -> new PrivacyExact12ActionInspectionV1(new byte[127]));
    final byte[] zeroField = new byte[128];
    Arrays.fill(zeroField, (byte) 1);
    Arrays.fill(zeroField, 64, 96, (byte) 0);
    assertThrows(
        IllegalStateException.class,
        () -> new PrivacyExact12ActionInspectionV1(zeroField));
  }

  private static byte[] fixed32(final int value) {
    final byte[] bytes = new byte[32];
    Arrays.fill(bytes, (byte) value);
    return bytes;
  }
}

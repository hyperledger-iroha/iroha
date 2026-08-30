package org.hyperledger.iroha.android.privacy;

import java.util.Arrays;

/** Native-authenticated public digest projection for one exact signed action wire. */
public final class PrivacyExact12ActionInspectionV1 {
  private static final int HASH_BYTES = 32;
  private static final int PROJECTION_BYTES = 4 * HASH_BYTES;
  private final byte[] projection;

  PrivacyExact12ActionInspectionV1(final byte[] projection) {
    if (projection == null || projection.length != PROJECTION_BYTES) {
      throw new IllegalStateException(
          "native Exact12 action inspection must contain exactly "
              + PROJECTION_BYTES
              + " bytes");
    }
    this.projection = projection.clone();
    for (int offset = 0; offset < PROJECTION_BYTES; offset += HASH_BYTES) {
      int aggregate = 0;
      for (int index = offset; index < offset + HASH_BYTES; index++) {
        aggregate |= projection[index];
      }
      if (aggregate == 0) {
        throw new IllegalStateException(
            "native Exact12 action inspection contains a zero digest");
      }
    }
  }

  public byte[] transactionHash() {
    return Arrays.copyOfRange(projection, 0, 32);
  }

  public byte[] transactionIntentDigest() {
    return Arrays.copyOfRange(projection, 32, 64);
  }

  public byte[] statementDigest() {
    return Arrays.copyOfRange(projection, 64, 96);
  }

  public byte[] proofEnvelopeHash() {
    return Arrays.copyOfRange(projection, 96, 128);
  }
}

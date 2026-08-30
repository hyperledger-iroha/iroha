package org.hyperledger.iroha.android.testing;

import org.hyperledger.iroha.android.model.NetworkId;

/** Deterministic canonical network identities for Java SDK tests. */
public final class TestNetworkIds {
  private static final NetworkId CANONICAL =
      NetworkId.parse(
          "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0");

  private TestNetworkIds() {}

  /** Returns the canonical development network identity. */
  public static NetworkId canonical() {
    return CANONICAL;
  }

  /** Returns a deterministic distinct canonical identity for the supplied seed. */
  public static NetworkId fromSeed(final long seed) {
    final byte[] bytes = new byte[NetworkId.BYTE_LENGTH];
    long state = seed ^ 0x9E37_79B9_7F4A_7C15L;
    for (int index = 0; index < bytes.length; index++) {
      state ^= state >>> 12;
      state ^= state << 25;
      state ^= state >>> 27;
      bytes[index] = (byte) (state * 0x2545_F491_4F6C_DD1DL);
    }
    bytes[bytes.length - 1] |= 0x01;
    return NetworkId.fromBytes(bytes);
  }
}

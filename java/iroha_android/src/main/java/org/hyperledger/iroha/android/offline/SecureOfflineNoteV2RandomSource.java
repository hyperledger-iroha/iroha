package org.hyperledger.iroha.android.offline;

import java.security.SecureRandom;

/** Secure random source for note secrets and payment token nonces. */
public final class SecureOfflineNoteV2RandomSource implements OfflineNoteV2RandomSource {
  private final SecureRandom secureRandom = new SecureRandom();

  @Override
  public byte[] nextBytes(final int length) {
    final byte[] bytes = new byte[length];
    secureRandom.nextBytes(bytes);
    return bytes;
  }
}

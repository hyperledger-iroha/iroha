package org.hyperledger.iroha.android.crypto.keystore;

import java.security.KeyPair;
import java.util.Objects;

/** Result of a keystore-backed key generation attempt. */
public final class KeyGenerationResult {

  private final KeyPair keyPair;
  private final boolean strongBoxBacked;

  public KeyGenerationResult(final KeyPair keyPair, final boolean strongBoxBacked) {
    this.keyPair = Objects.requireNonNull(keyPair, "keyPair");
    this.strongBoxBacked = strongBoxBacked;
  }

  public KeyPair keyPair() {
    return keyPair;
  }

  /** {@code true} when the backend produced a StrongBox-backed key. */
  public boolean strongBoxBacked() {
    return strongBoxBacked;
  }
}

package org.hyperledger.iroha.android.crypto;

import java.security.KeyPair;
import java.util.Objects;

/**
 * Describes the outcome of a key generation request, including the hardware path used.
 */
public final class KeyGenerationOutcome {

  /** Route used by the provider when generating the key. */
  public enum Route {
    STRONGBOX,
    HARDWARE,
    SOFTWARE
  }

  private final KeyPair keyPair;
  private final Route route;

  public KeyGenerationOutcome(final KeyPair keyPair, final Route route) {
    this.keyPair = Objects.requireNonNull(keyPair, "keyPair");
    this.route = Objects.requireNonNull(route, "route");
  }

  public KeyPair keyPair() {
    return keyPair;
  }

  public Route route() {
    return route;
  }
}

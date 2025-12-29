package org.hyperledger.iroha.android.crypto.keystore;

import java.time.Duration;

/**
 * Specification for key generation requests made through {@link KeystoreBackend}.
 *
 * <p>The parameters map closely to Android's {@code KeyGenParameterSpec} but avoid a direct
 * dependency so the desktop JVM build can compile. Android-specific builders will translate these
 * fields into platform constructs in follow-up revisions.
 */
public final class KeyGenParameters {
  private final boolean requireStrongBox;
  private final boolean preferStrongBox;
  private final boolean allowStrongBoxFallback;
  private final boolean userAuthenticationRequired;
  private final Duration userAuthenticationTimeout;
  private final String algorithm;
  private final byte[] attestationChallenge;

  private KeyGenParameters(final Builder builder) {
    this.requireStrongBox = builder.requireStrongBox;
    this.preferStrongBox = builder.preferStrongBox;
    this.allowStrongBoxFallback = builder.allowStrongBoxFallback;
    this.userAuthenticationRequired = builder.userAuthenticationRequired;
    this.userAuthenticationTimeout = builder.userAuthenticationTimeout;
    this.algorithm = builder.algorithm;
    this.attestationChallenge =
        builder.attestationChallenge == null ? null : builder.attestationChallenge.clone();
  }

  public boolean requireStrongBox() {
    return requireStrongBox;
  }

  public boolean allowStrongBoxFallback() {
    return allowStrongBoxFallback;
  }

  public boolean preferStrongBox() {
    return preferStrongBox;
  }

  public boolean userAuthenticationRequired() {
    return userAuthenticationRequired;
  }

  public Duration userAuthenticationTimeout() {
    return userAuthenticationTimeout;
  }

  public String algorithm() {
    return algorithm;
  }

  public byte[] attestationChallenge() {
    return attestationChallenge == null ? null : attestationChallenge.clone();
  }

  public Builder toBuilder() {
    return new Builder()
        .setRequireStrongBox(requireStrongBox)
        .setPreferStrongBox(preferStrongBox)
        .setAllowStrongBoxFallback(allowStrongBoxFallback)
        .setUserAuthenticationRequired(userAuthenticationRequired)
        .setUserAuthenticationTimeout(userAuthenticationTimeout)
        .setAlgorithm(algorithm)
        .setAttestationChallenge(attestationChallenge);
  }

  public static Builder builder() {
    return new Builder();
  }

  public static final class Builder {
    private boolean requireStrongBox = false;
    private boolean preferStrongBox = false;
    private boolean allowStrongBoxFallback = true;
    private boolean userAuthenticationRequired = false;
    private Duration userAuthenticationTimeout = Duration.ZERO;
    private String algorithm = "Ed25519";
    private byte[] attestationChallenge = null;

    public Builder setRequireStrongBox(final boolean requireStrongBox) {
      this.requireStrongBox = requireStrongBox;
      if (requireStrongBox) {
        this.allowStrongBoxFallback = false;
      }
      return this;
    }

    public Builder setPreferStrongBox(final boolean preferStrongBox) {
      this.preferStrongBox = preferStrongBox;
      if (preferStrongBox && !requireStrongBox) {
        this.allowStrongBoxFallback = true;
      }
      return this;
    }

    public Builder setAllowStrongBoxFallback(final boolean allowStrongBoxFallback) {
      this.allowStrongBoxFallback = allowStrongBoxFallback;
      if (!allowStrongBoxFallback) {
        this.requireStrongBox = true;
      }
      return this;
    }

    public Builder setUserAuthenticationRequired(final boolean userAuthenticationRequired) {
      this.userAuthenticationRequired = userAuthenticationRequired;
      return this;
    }

    public Builder setUserAuthenticationTimeout(final Duration userAuthenticationTimeout) {
      if (userAuthenticationTimeout != null) {
        this.userAuthenticationTimeout = userAuthenticationTimeout;
      }
      return this;
    }

    public Builder setAlgorithm(final String algorithm) {
      if (algorithm != null && !algorithm.isBlank()) {
        this.algorithm = algorithm;
      }
      return this;
    }

    public Builder setAttestationChallenge(final byte[] attestationChallenge) {
      if (attestationChallenge != null) {
        this.attestationChallenge = attestationChallenge.clone();
      }
      return this;
    }

    public KeyGenParameters build() {
      return new KeyGenParameters(this);
    }
  }
}

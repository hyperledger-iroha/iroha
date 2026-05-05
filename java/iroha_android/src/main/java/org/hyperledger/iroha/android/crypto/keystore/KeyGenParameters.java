package org.hyperledger.iroha.android.crypto.keystore;

import java.time.Duration;
import org.hyperledger.iroha.android.crypto.SigningAlgorithm;

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
  private final boolean userAuthenticationRequired;
  private final Duration userAuthenticationTimeout;
  private final String algorithm;
  private final byte[] attestationChallenge;
  private final Integer usageCountLimit;

  private KeyGenParameters(final Builder builder) {
    this.requireStrongBox = builder.requireStrongBox;
    this.preferStrongBox = builder.preferStrongBox;
    this.userAuthenticationRequired = builder.userAuthenticationRequired;
    this.userAuthenticationTimeout = builder.userAuthenticationTimeout;
    this.algorithm = builder.algorithm;
    this.attestationChallenge =
        builder.attestationChallenge == null ? null : builder.attestationChallenge.clone();
    this.usageCountLimit = builder.usageCountLimit;
  }

  public boolean requireStrongBox() {
    return requireStrongBox;
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

  public SigningAlgorithm signingAlgorithm() {
    return SigningAlgorithm.fromAlgorithmName(algorithm);
  }

  public byte[] attestationChallenge() {
    return attestationChallenge == null ? null : attestationChallenge.clone();
  }

  public Integer usageCountLimit() {
    return usageCountLimit;
  }

  public Builder toBuilder() {
    return new Builder()
        .setRequireStrongBox(requireStrongBox)
        .setPreferStrongBox(preferStrongBox)
        .setUserAuthenticationRequired(userAuthenticationRequired)
        .setUserAuthenticationTimeout(userAuthenticationTimeout)
        .setSigningAlgorithm(signingAlgorithm())
        .setAttestationChallenge(attestationChallenge)
        .setUsageCountLimit(usageCountLimit);
  }

  public static Builder builder() {
    return new Builder();
  }

  public static final class Builder {
    private boolean requireStrongBox = false;
    private boolean preferStrongBox = false;
    private boolean userAuthenticationRequired = false;
    private Duration userAuthenticationTimeout = Duration.ZERO;
    private String algorithm = "Ed25519";
    private byte[] attestationChallenge = null;
    private Integer usageCountLimit = null;

    public Builder setRequireStrongBox(final boolean requireStrongBox) {
      this.requireStrongBox = requireStrongBox;
      return this;
    }

    public Builder setPreferStrongBox(final boolean preferStrongBox) {
      this.preferStrongBox = preferStrongBox;
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
        this.algorithm = SigningAlgorithm.fromAlgorithmName(algorithm).providerName();
      }
      return this;
    }

    public Builder setSigningAlgorithm(final SigningAlgorithm signingAlgorithm) {
      if (signingAlgorithm != null) {
        this.algorithm = signingAlgorithm.providerName();
      }
      return this;
    }

    public Builder setAttestationChallenge(final byte[] attestationChallenge) {
      if (attestationChallenge != null) {
        this.attestationChallenge = attestationChallenge.clone();
      }
      return this;
    }

    public Builder setUsageCountLimit(final Integer usageCountLimit) {
      if (usageCountLimit == null) {
        this.usageCountLimit = null;
      } else if (usageCountLimit <= 0) {
        throw new IllegalArgumentException("usageCountLimit must be positive");
      } else {
        this.usageCountLimit = usageCountLimit;
      }
      return this;
    }

    public Builder setUsageCountLimit(final int usageCountLimit) {
      return setUsageCountLimit(Integer.valueOf(usageCountLimit));
    }

    public KeyGenParameters build() {
      return new KeyGenParameters(this);
    }
  }
}

package org.hyperledger.iroha.android.crypto;

/**
 * Describes capabilities exposed by a {@link org.hyperledger.iroha.android.IrohaKeyManager.KeyProvider}.
 *
 * <p>The metadata is intentionally generic so it can be computed on desktop JVMs and populated with
 * richer details (attestation/StrongBox) when running on Android. Android-specific providers will
 * extend this to include platform attestation blobs in follow-up revisions.
 */
public final class KeyProviderMetadata {

  /** Indicates the strongest hardware security level available for generated keys. */
  public enum HardwareSecurityLevel {
    NONE,
    TRUSTED_ENVIRONMENT,
    STRONGBOX,
    SECURE_ELEMENT
  }

  private final String name;
  private final boolean hardwareBacked;
  private final boolean strongBoxBacked;
  private final boolean secureElementBacked;
  private final boolean supportsAttestationCertificates;
  private final HardwareSecurityLevel securityLevel;

  private KeyProviderMetadata(
      final Builder builder) {
    this.name = builder.name;
    this.hardwareBacked = builder.hardwareBacked;
    this.strongBoxBacked = builder.strongBoxBacked;
    this.secureElementBacked = builder.secureElementBacked;
    this.supportsAttestationCertificates = builder.supportsAttestationCertificates;
    this.securityLevel = builder.securityLevel;
  }

  public String name() {
    return name;
  }

  public boolean hardwareBacked() {
    return hardwareBacked;
  }

  public boolean strongBoxBacked() {
    return strongBoxBacked;
  }

  public boolean secureElementBacked() {
    return secureElementBacked;
  }

  public boolean supportsAttestationCertificates() {
    return supportsAttestationCertificates;
  }

  public HardwareSecurityLevel securityLevel() {
    return securityLevel;
  }

  /** Returns a builder primed with the current metadata values. */
  public Builder toBuilder() {
    return new Builder()
        .setName(name)
        .setHardwareBacked(hardwareBacked)
        .setStrongBoxBacked(strongBoxBacked)
        .setSecureElementBacked(secureElementBacked)
        .setSupportsAttestationCertificates(supportsAttestationCertificates)
        .setSecurityLevel(securityLevel);
  }

  /** Creates metadata for a purely software provider. */
  public static KeyProviderMetadata software(final String name) {
    return builder(name).build();
  }

  /** Creates metadata for a Trusted Environment backed provider (TEE). */
  public static KeyProviderMetadata trustedEnvironment(final String name) {
    return builder(name)
        .setHardwareBacked(true)
        .setSecurityLevel(HardwareSecurityLevel.TRUSTED_ENVIRONMENT)
        .build();
  }

  /** Creates metadata for a StrongBox backed provider. */
  public static KeyProviderMetadata strongBox(final String name, final boolean supportsAttestation) {
    return builder(name)
        .setHardwareBacked(true)
        .setStrongBoxBacked(true)
        .setSupportsAttestationCertificates(supportsAttestation)
        .setSecurityLevel(HardwareSecurityLevel.STRONGBOX)
        .build();
  }

  /** Creates metadata for a discrete secure element backed provider. */
  public static KeyProviderMetadata secureElement(final String name, final boolean supportsAttestation) {
    return builder(name)
        .setHardwareBacked(true)
        .setSecureElementBacked(true)
        .setSupportsAttestationCertificates(supportsAttestation)
        .setSecurityLevel(HardwareSecurityLevel.SECURE_ELEMENT)
        .build();
  }

  public static Builder builder(final String name) {
    return new Builder().setName(name);
  }

  public static final class Builder {
    private String name = "unknown-provider";
    private boolean hardwareBacked = false;
    private boolean strongBoxBacked = false;
    private boolean secureElementBacked = false;
    private boolean supportsAttestationCertificates = false;
    private HardwareSecurityLevel securityLevel = HardwareSecurityLevel.NONE;

    public Builder setName(final String name) {
      if (name != null && !name.isBlank()) {
        this.name = name;
      }
      return this;
    }

    public Builder setHardwareBacked(final boolean hardwareBacked) {
      this.hardwareBacked = hardwareBacked;
      if (!hardwareBacked) {
        this.strongBoxBacked = false;
        this.secureElementBacked = false;
        this.supportsAttestationCertificates = false;
        this.securityLevel = HardwareSecurityLevel.NONE;
      }
      return this;
    }

    public Builder setStrongBoxBacked(final boolean strongBoxBacked) {
      this.strongBoxBacked = strongBoxBacked;
      if (strongBoxBacked) {
        this.hardwareBacked = true;
        this.securityLevel = HardwareSecurityLevel.STRONGBOX;
      }
      return this;
    }

    public Builder setSecureElementBacked(final boolean secureElementBacked) {
      this.secureElementBacked = secureElementBacked;
      if (secureElementBacked) {
        this.hardwareBacked = true;
        this.securityLevel = HardwareSecurityLevel.SECURE_ELEMENT;
      }
      return this;
    }

    public Builder setSupportsAttestationCertificates(final boolean supportsAttestationCertificates) {
      this.supportsAttestationCertificates = supportsAttestationCertificates;
      return this;
    }

    public Builder setSecurityLevel(final HardwareSecurityLevel securityLevel) {
      if (securityLevel != null) {
        this.securityLevel = securityLevel;
        this.hardwareBacked = securityLevel != HardwareSecurityLevel.NONE;
      }
      return this;
    }

    public KeyProviderMetadata build() {
      return new KeyProviderMetadata(this);
    }
  }
}

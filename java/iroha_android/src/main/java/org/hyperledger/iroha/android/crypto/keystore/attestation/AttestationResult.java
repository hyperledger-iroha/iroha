package org.hyperledger.iroha.android.crypto.keystore.attestation;

import java.security.cert.X509Certificate;
import java.util.ArrayList;
import java.util.List;
import java.util.Objects;

/** Result of a successfully verified Android key attestation bundle. */
public final class AttestationResult {
  private final String alias;
  private final List<X509Certificate> certificateChain;
  private final X509Certificate leafCertificate;
  private final SecurityLevel attestationSecurityLevel;
  private final SecurityLevel keymasterSecurityLevel;
  private final byte[] attestationChallenge;
  private final byte[] uniqueId;
  private final boolean softwareAuthorisationsPresent;
  private final boolean teeAuthorisationsPresent;
  private final boolean strongBoxAuthorisationsPresent;

  public AttestationResult(
      final String alias,
      final List<X509Certificate> certificateChain,
      final SecurityLevel attestationSecurityLevel,
      final SecurityLevel keymasterSecurityLevel,
      final byte[] attestationChallenge,
      final byte[] uniqueId,
      final boolean softwareAuthorisationsPresent,
      final boolean teeAuthorisationsPresent,
      final boolean strongBoxAuthorisationsPresent) {
    this.alias = Objects.requireNonNull(alias, "alias");
    this.certificateChain = List.copyOf(certificateChain);
    if (certificateChain.isEmpty()) {
      throw new IllegalArgumentException("certificateChain must not be empty");
    }
    this.leafCertificate = certificateChain.get(0);
    this.attestationSecurityLevel =
        Objects.requireNonNull(attestationSecurityLevel, "attestationSecurityLevel");
    this.keymasterSecurityLevel =
        Objects.requireNonNull(keymasterSecurityLevel, "keymasterSecurityLevel");
    this.attestationChallenge =
        attestationChallenge == null ? new byte[0] : attestationChallenge.clone();
    this.uniqueId = uniqueId == null ? new byte[0] : uniqueId.clone();
    this.softwareAuthorisationsPresent = softwareAuthorisationsPresent;
    this.teeAuthorisationsPresent = teeAuthorisationsPresent;
    this.strongBoxAuthorisationsPresent = strongBoxAuthorisationsPresent;
  }

  /** Alias associated with the generated key. */
  public String alias() {
    return alias;
  }

  /** Immutable copy of the attestation certificate chain (leaf first). */
  public List<X509Certificate> certificateChain() {
    return new ArrayList<>(certificateChain);
  }

  /** Leaf certificate that carries the attestation extension. */
  public X509Certificate leafCertificate() {
    return leafCertificate;
  }

  /** Security level encoded in the attestation extension. */
  public SecurityLevel attestationSecurityLevel() {
    return attestationSecurityLevel;
  }

  /** Keymaster security level reported in the attestation extension. */
  public SecurityLevel keymasterSecurityLevel() {
    return keymasterSecurityLevel;
  }

  /** Challenge embedded in the attestation extension (copy). */
  public byte[] attestationChallenge() {
    return attestationChallenge.clone();
  }

  /** Unique identifier reported in the attestation extension (copy). */
  public byte[] uniqueId() {
    return uniqueId.clone();
  }

  /** Returns {@code true} when software-enforced authorisations were present. */
  public boolean softwareAuthorisationsPresent() {
    return softwareAuthorisationsPresent;
  }

  /** Returns {@code true} when TEE-enforced authorisations were present. */
  public boolean teeAuthorisationsPresent() {
    return teeAuthorisationsPresent;
  }

  /** Returns {@code true} when StrongBox-enforced authorisations were present. */
  public boolean strongBoxAuthorisationsPresent() {
    return strongBoxAuthorisationsPresent;
  }

  /** Convenience helper indicating StrongBox-backed attestation. */
  public boolean isStrongBoxAttestation() {
    return attestationSecurityLevel == SecurityLevel.STRONG_BOX;
  }

  /** Enumeration of Android key attestation security levels. */
  public enum SecurityLevel {
    SOFTWARE(0),
    TRUSTED_ENVIRONMENT(1),
    STRONG_BOX(2);

    private final int encodedValue;

    SecurityLevel(final int encodedValue) {
      this.encodedValue = encodedValue;
    }

    int encodedValue() {
      return encodedValue;
    }

    static SecurityLevel fromEncoded(final int value) {
      for (final SecurityLevel level : values()) {
        if (level.encodedValue == value) {
          return level;
        }
      }
      throw new IllegalArgumentException("Unknown security level value: " + value);
    }
  }
}

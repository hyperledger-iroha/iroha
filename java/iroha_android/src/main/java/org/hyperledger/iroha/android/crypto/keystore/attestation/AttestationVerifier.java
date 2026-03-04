package org.hyperledger.iroha.android.crypto.keystore.attestation;

import java.io.ByteArrayInputStream;
import java.math.BigInteger;
import java.security.MessageDigest;
import java.security.cert.CertPath;
import java.security.cert.CertPathValidator;
import java.security.cert.CertPathValidatorException;
import java.security.cert.CertificateException;
import java.security.cert.CertificateFactory;
import java.security.cert.PKIXParameters;
import java.security.cert.TrustAnchor;
import java.security.cert.X509Certificate;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Objects;
import java.util.Set;
import org.hyperledger.iroha.android.crypto.keystore.KeyAttestation;

/**
 * Validates Android key attestation certificate chains and extracts metadata required by higher
 * level policy checks.
 */
public final class AttestationVerifier {

  private static final String ATTESTATION_OID = "1.3.6.1.4.1.11129.2.1.17";

  private final Set<TrustAnchor> trustAnchors;
  private final boolean requireStrongBox;

  private AttestationVerifier(final Builder builder) {
    if (builder.trustedRoots.isEmpty()) {
      throw new IllegalStateException("At least one trusted root certificate is required");
    }
    final Set<TrustAnchor> anchors = new LinkedHashSet<>();
    for (final X509Certificate certificate : builder.trustedRoots) {
      anchors.add(new TrustAnchor(certificate, null));
    }
    this.trustAnchors = Set.copyOf(anchors);
    this.requireStrongBox = builder.requireStrongBox;
  }

  /** Creates a verifier that trusts the supplied root certificates. */
  public static Builder builder() {
    return new Builder();
  }

  /** Validates {@code attestation} against the configured policy. */
  public AttestationResult verify(final KeyAttestation attestation)
      throws AttestationVerificationException {
    return verify(attestation, null);
  }

  /**
   * Validates {@code attestation} and checks that the embedded challenge matches {@code
   * expectedChallenge} when provided.
   */
  public AttestationResult verify(final KeyAttestation attestation, final byte[] expectedChallenge)
      throws AttestationVerificationException {
    Objects.requireNonNull(attestation, "attestation");
    final List<X509Certificate> chain = decodeChain(attestation);
    if (chain.isEmpty()) {
      throw new AttestationVerificationException("Attestation certificate chain is empty");
    }
    final X509Certificate leaf = chain.get(0);

    validateCertificatePath(chain);

    final KeyDescription description = parseKeyDescription(leaf);
    if (expectedChallenge != null
        && !MessageDigest.isEqual(expectedChallenge, description.attestationChallenge)) {
      throw new AttestationVerificationException("Attestation challenge mismatch");
    }
    if (requireStrongBox
        && description.attestationSecurityLevel != AttestationResult.SecurityLevel.STRONG_BOX) {
      throw new AttestationVerificationException("StrongBox attestation required by policy");
    }

    return new AttestationResult(
        attestation.alias(),
        chain,
        description.attestationSecurityLevel,
        description.keymasterSecurityLevel,
        description.attestationChallenge,
        description.uniqueId,
        description.softwareAuthorisationsLength > 0,
        description.teeAuthorisationsLength > 0,
        description.strongBoxAuthorisationsLength > 0);
  }

  private List<X509Certificate> decodeChain(final KeyAttestation attestation)
      throws AttestationVerificationException {
    final CertificateFactory factory;
    try {
      factory = CertificateFactory.getInstance("X.509");
    } catch (final CertificateException ex) {
      throw new AttestationVerificationException("Unable to acquire X.509 CertificateFactory", ex);
    }

    final List<X509Certificate> certificates = new ArrayList<>();
    for (final byte[] certificateDer : attestation.certificateChain()) {
      try {
        certificates.add(
            (X509Certificate) factory.generateCertificate(new ByteArrayInputStream(certificateDer)));
      } catch (final CertificateException ex) {
        throw new AttestationVerificationException("Failed to decode attestation certificate", ex);
      }
    }
    return certificates;
  }

  private void validateCertificatePath(final List<X509Certificate> chain)
      throws AttestationVerificationException {
    final CertificateFactory factory;
    try {
      factory = CertificateFactory.getInstance("X.509");
    } catch (final CertificateException ex) {
      throw new AttestationVerificationException("Unable to acquire X.509 CertificateFactory", ex);
    }

    final CertPath certPath;
    try {
      certPath = factory.generateCertPath(chain);
    } catch (final CertificateException ex) {
      throw new AttestationVerificationException("Failed to construct attestation CertPath", ex);
    }

    final CertPathValidator validator;
    try {
      validator = CertPathValidator.getInstance("PKIX");
    } catch (final Exception ex) {
      throw new AttestationVerificationException("Unable to acquire PKIX CertPathValidator", ex);
    }

    final PKIXParameters parameters;
    try {
      parameters = new PKIXParameters(trustAnchors);
    } catch (final Exception ex) {
      throw new AttestationVerificationException("Invalid PKIX parameters", ex);
    }
    parameters.setRevocationEnabled(false);

    try {
      validator.validate(certPath, parameters);
    } catch (final CertPathValidatorException ex) {
      throw new AttestationVerificationException("Attestation certificate path validation failed", ex);
    } catch (final Exception ex) {
      throw new AttestationVerificationException(
          "Unexpected failure validating attestation certificate path", ex);
    }
  }

  private KeyDescription parseKeyDescription(final X509Certificate leaf)
      throws AttestationVerificationException {
    final byte[] extension = leaf.getExtensionValue(ATTESTATION_OID);
    if (extension == null) {
      throw new AttestationVerificationException(
          "Leaf certificate does not contain Android attestation extension");
    }

    final DerReader outer = new DerReader(extension);
    final byte[] octetString = outer.readOctetString();
    if (outer.hasRemaining()) {
      throw new AttestationVerificationException("Unexpected data after attestation extension");
    }

    final DerReader reader = DerReader.sequence(octetString);
    final int attestationVersion = reader.readInteger();
    // attestationVersion is currently unused but read to advance the reader.
    if (attestationVersion <= 0) {
      throw new AttestationVerificationException("Invalid attestation version: " + attestationVersion);
    }

    final AttestationResult.SecurityLevel attestationLevel =
        AttestationResult.SecurityLevel.fromEncoded(reader.readEnumerated());
    final int keymasterVersion = reader.readInteger();
    if (keymasterVersion < 0) {
      throw new AttestationVerificationException("Invalid keymaster version: " + keymasterVersion);
    }
    final AttestationResult.SecurityLevel keymasterLevel =
        AttestationResult.SecurityLevel.fromEncoded(reader.readEnumerated());
    final byte[] challenge = reader.readOctetString();
    final byte[] uniqueId = reader.readOctetString();
    final byte[] softwareEnforced = reader.readSequenceBytes();
    final byte[] teeEnforced = reader.readSequenceBytes();
    byte[] strongBoxEnforced = new byte[0];
    if (reader.hasRemaining()) {
      strongBoxEnforced = reader.readSequenceBytes();
    }
    if (reader.hasRemaining()) {
      throw new AttestationVerificationException("Unexpected trailing data in attestation");
    }

    return new KeyDescription(
        attestationLevel, keymasterLevel, challenge, uniqueId, softwareEnforced.length,
        teeEnforced.length, strongBoxEnforced.length);
  }

  /** Builder used to configure {@link AttestationVerifier} instances. */
  public static final class Builder {
    private final Set<X509Certificate> trustedRoots = new LinkedHashSet<>();
    private boolean requireStrongBox = false;

    private Builder() {}

    /** Adds a trusted root certificate in DER form. */
    public Builder addTrustedRoot(final byte[] certificateDer)
        throws AttestationVerificationException {
      Objects.requireNonNull(certificateDer, "certificateDer");
      try {
        final CertificateFactory factory = CertificateFactory.getInstance("X.509");
        trustedRoots.add(
            (X509Certificate) factory.generateCertificate(new ByteArrayInputStream(certificateDer)));
        return this;
      } catch (final CertificateException ex) {
        throw new AttestationVerificationException("Failed to decode trusted root certificate", ex);
      }
    }

    /** Adds a trusted root certificate. */
    public Builder addTrustedRoot(final X509Certificate certificate) {
      trustedRoots.add(Objects.requireNonNull(certificate, "certificate"));
      return this;
    }

    /** Requires StrongBox-backed attestation when {@code enabled} is {@code true}. */
    public Builder requireStrongBox(final boolean enabled) {
      this.requireStrongBox = enabled;
      return this;
    }

    public AttestationVerifier build() {
      return new AttestationVerifier(this);
    }
  }

  private static final class KeyDescription {
    private final AttestationResult.SecurityLevel attestationSecurityLevel;
    private final AttestationResult.SecurityLevel keymasterSecurityLevel;
    private final byte[] attestationChallenge;
    private final byte[] uniqueId;
    private final int softwareAuthorisationsLength;
    private final int teeAuthorisationsLength;
    private final int strongBoxAuthorisationsLength;

    private KeyDescription(
        final AttestationResult.SecurityLevel attestationSecurityLevel,
        final AttestationResult.SecurityLevel keymasterSecurityLevel,
        final byte[] attestationChallenge,
        final byte[] uniqueId,
        final int softwareAuthorisationsLength,
        final int teeAuthorisationsLength,
        final int strongBoxAuthorisationsLength) {
      this.attestationSecurityLevel = attestationSecurityLevel;
      this.keymasterSecurityLevel = keymasterSecurityLevel;
      this.attestationChallenge = attestationChallenge == null ? new byte[0] : attestationChallenge;
      this.uniqueId = uniqueId == null ? new byte[0] : uniqueId;
      this.softwareAuthorisationsLength = softwareAuthorisationsLength;
      this.teeAuthorisationsLength = teeAuthorisationsLength;
      this.strongBoxAuthorisationsLength = strongBoxAuthorisationsLength;
    }
  }

  private static final class DerReader {
    private static final int TAG_SEQUENCE = 0x30;
    private static final int TAG_INTEGER = 0x02;
    private static final int TAG_ENUMERATED = 0x0A;
    private static final int TAG_OCTET_STRING = 0x04;

    private final byte[] buffer;
    private int offset = 0;

    static DerReader sequence(final byte[] data) throws AttestationVerificationException {
      final DerReader reader = new DerReader(data);
      return new DerReader(reader.readWithExpectedTag(TAG_SEQUENCE));
    }

    DerReader(final byte[] buffer) {
      this.buffer = Objects.requireNonNull(buffer, "buffer");
    }

    boolean hasRemaining() {
      return offset < buffer.length;
    }

    int readInteger() throws AttestationVerificationException {
      return readIntegerWithTag(TAG_INTEGER);
    }

    int readEnumerated() throws AttestationVerificationException {
      return readIntegerWithTag(TAG_ENUMERATED);
    }

    byte[] readOctetString() throws AttestationVerificationException {
      return readWithExpectedTag(TAG_OCTET_STRING);
    }

    byte[] readSequenceBytes() throws AttestationVerificationException {
      return readWithExpectedTag(TAG_SEQUENCE);
    }

    private int readIntegerWithTag(final int expectedTag) throws AttestationVerificationException {
      final byte[] value = readWithExpectedTag(expectedTag);
      try {
        return new BigInteger(value).intValueExact();
      } catch (final ArithmeticException ex) {
        throw new AttestationVerificationException("Integer value out of range", ex);
      }
    }

    private byte[] readWithExpectedTag(final int expectedTag)
        throws AttestationVerificationException {
      final int tag = readTag();
      if (tag != expectedTag) {
        throw new AttestationVerificationException(
            String.format("Unexpected DER tag. expected=0x%02X actual=0x%02X", expectedTag, tag));
      }
      final int length = readLength();
      if (length < 0) {
        throw new AttestationVerificationException("Invalid DER length");
      }
      if (offset + length > buffer.length) {
        throw new AttestationVerificationException("DER value overruns buffer");
      }
      final byte[] value = Arrays.copyOfRange(buffer, offset, offset + length);
      offset += length;
      return value;
    }

    private int readTag() throws AttestationVerificationException {
      if (offset >= buffer.length) {
        throw new AttestationVerificationException("Unexpected end of DER input");
      }
      return buffer[offset++] & 0xFF;
    }

    private int readLength() throws AttestationVerificationException {
      if (offset >= buffer.length) {
        throw new AttestationVerificationException("Unexpected end of DER input");
      }
      final int lengthByte = buffer[offset++] & 0xFF;
      if ((lengthByte & 0x80) == 0) {
        return lengthByte;
      }
      final int lengthOctets = lengthByte & 0x7F;
      if (lengthOctets == 0 || lengthOctets > 4) {
        throw new AttestationVerificationException("Unsupported DER length encoding");
      }
      int length = 0;
      for (int i = 0; i < lengthOctets; i++) {
        if (offset >= buffer.length) {
          throw new AttestationVerificationException("Invalid DER length encoding");
        }
        length = (length << 8) | (buffer[offset++] & 0xFF);
      }
      return length;
    }
  }
}

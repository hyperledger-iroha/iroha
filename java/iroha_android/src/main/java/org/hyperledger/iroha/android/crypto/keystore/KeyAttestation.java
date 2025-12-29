package org.hyperledger.iroha.android.crypto.keystore;

import java.security.cert.CertificateEncodingException;
import java.security.cert.X509Certificate;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Objects;

/**
 * Captures attestation material associated with a generated key.
 *
 * <p>The attestation chain can be surfaced to application code so it can be pinned or transmitted to
 * remote verifiers. Certificates are stored in their DER-encoded form to avoid depending on Android
 * {@code android.security.keystore} types.
 */
public final class KeyAttestation {
  private final String alias;
  private final List<byte[]> certificateChain;

  private KeyAttestation(final Builder builder) {
    this.alias = builder.alias;
    this.certificateChain = List.copyOf(builder.certificateChain);
  }

  public String alias() {
    return alias;
  }

  /**
   * Returns the attestation certificate chain in DER form, starting from the leaf certificate. The
   * list is an immutable copy.
   */
  public List<byte[]> certificateChain() {
    return new ArrayList<>(certificateChain);
  }

  public Builder toBuilder() {
    return new Builder().setAlias(alias).setCertificateChain(certificateChain);
  }

  public static Builder builder() {
    return new Builder();
  }

  public static final class Builder {
    private String alias = "";
    private final List<byte[]> certificateChain = new ArrayList<>();

    public Builder setAlias(final String alias) {
      this.alias = Objects.requireNonNull(alias, "alias");
      return this;
    }

    public Builder addCertificate(final byte[] certificateDer) {
      certificateChain.add(Arrays.copyOf(Objects.requireNonNull(certificateDer, "certificateDer"),
          certificateDer.length));
      return this;
    }

    public Builder addCertificate(final X509Certificate certificate) {
      Objects.requireNonNull(certificate, "certificate");
      try {
        return addCertificate(certificate.getEncoded());
      } catch (final CertificateEncodingException ex) {
        throw new IllegalArgumentException("Failed to encode certificate", ex);
      }
    }

    public Builder setCertificateChain(final List<byte[]> certs) {
      certificateChain.clear();
      if (certs != null) {
        for (byte[] cert : certs) {
          addCertificate(cert);
        }
      }
      return this;
    }

    public KeyAttestation build() {
      return new KeyAttestation(this);
    }
  }
}

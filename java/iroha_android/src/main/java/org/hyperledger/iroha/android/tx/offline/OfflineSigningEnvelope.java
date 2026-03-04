package org.hyperledger.iroha.android.tx.offline;

import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import org.hyperledger.iroha.android.tx.SignedTransaction;

/**
 * Norito-serialisable container that captures a signed transaction alongside
 * metadata required for offline submission or hand-off between devices.
 */
public final class OfflineSigningEnvelope {

  private final byte[] encodedPayload;
  private final byte[] signature;
  private final byte[] publicKey;
  private final String schemaName;
  private final String keyAlias;
  private final long issuedAtMs;
  private final Map<String, String> metadata;
  private final byte[] exportedKeyBundle;

  private OfflineSigningEnvelope(final Builder builder) {
    this.encodedPayload =
        copy(Objects.requireNonNull(builder.encodedPayload, "encodedPayload"));
    this.signature = copy(Objects.requireNonNull(builder.signature, "signature"));
    this.publicKey = copy(Objects.requireNonNull(builder.publicKey, "publicKey"));
    this.schemaName = normalize(builder.schemaName, "schemaName");
    this.keyAlias = normalize(builder.keyAlias, "keyAlias");
    if (builder.issuedAtMs < 0) {
      throw new IllegalArgumentException("issuedAtMs must be non-negative");
    }
    this.issuedAtMs = builder.issuedAtMs;
    this.metadata =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(builder.metadata, "metadata")));
    this.exportedKeyBundle =
        builder.exportedKeyBundle == null ? null : copy(builder.exportedKeyBundle);
  }

  /** Returns the canonical Norito payload bytes. */
  public byte[] encodedPayload() {
    return copy(encodedPayload);
  }

  /** Returns the raw signature bytes. */
  public byte[] signature() {
    return copy(signature);
  }

  /** Returns the public signing key bytes. */
  public byte[] publicKey() {
    return copy(publicKey);
  }

  /** Returns the Norito schema used when encoding the payload. */
  public String schemaName() {
    return schemaName;
  }

  /** Returns the alias used to look up the signing key. */
  public String keyAlias() {
    return keyAlias;
  }

  /** Timestamp (milliseconds since Unix epoch) when the envelope was created. */
  public long issuedAtMs() {
    return issuedAtMs;
  }

  /** Additional metadata supplied by the caller (immutable). */
  public Map<String, String> metadata() {
    return metadata;
  }

  /** Optional deterministic export of the signing key (if provided). */
  public Optional<byte[]> exportedKeyBundle() {
    return exportedKeyBundle == null ? Optional.empty() : Optional.of(copy(exportedKeyBundle));
  }

  /** Recreates a {@link SignedTransaction} instance from the envelope contents. */
  public SignedTransaction toSignedTransaction() {
    return new SignedTransaction(
        encodedPayload(),
        signature(),
        publicKey(),
        schemaName,
        keyAlias,
        exportedKeyBundle);
  }

  /** Creates a builder initialised from the current envelope. */
  public Builder toBuilder() {
    final Builder builder =
        builder()
            .setEncodedPayload(encodedPayload())
            .setSignature(signature())
            .setPublicKey(publicKey())
            .setSchemaName(schemaName)
            .setKeyAlias(keyAlias)
            .setIssuedAtMs(issuedAtMs)
            .setMetadata(metadata);
    exportedKeyBundle().ifPresent(builder::setExportedKeyBundle);
    return builder;
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof OfflineSigningEnvelope)) {
      return false;
    }
    final OfflineSigningEnvelope other = (OfflineSigningEnvelope) obj;
    return Arrays.equals(encodedPayload, other.encodedPayload)
        && Arrays.equals(signature, other.signature)
        && Arrays.equals(publicKey, other.publicKey)
        && schemaName.equals(other.schemaName)
        && keyAlias.equals(other.keyAlias)
        && issuedAtMs == other.issuedAtMs
        && metadata.equals(other.metadata)
        && Arrays.equals(exportedKeyBundle, other.exportedKeyBundle);
  }

  @Override
  public int hashCode() {
    int result =
        Objects.hash(schemaName, keyAlias, issuedAtMs, metadata);
    result = 31 * result + Arrays.hashCode(encodedPayload);
    result = 31 * result + Arrays.hashCode(signature);
    result = 31 * result + Arrays.hashCode(publicKey);
    result = 31 * result + Arrays.hashCode(exportedKeyBundle);
    return result;
  }

  private static byte[] copy(final byte[] source) {
    return Arrays.copyOf(source, source.length);
  }

  private static String normalize(final String value, final String field) {
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException(field + " must not be blank");
    }
    return value;
  }

  public static Builder builder() {
    return new Builder();
  }

  /** Builds {@link OfflineSigningEnvelope} instances. */
  public static final class Builder {
    private byte[] encodedPayload = new byte[0];
    private byte[] signature = new byte[0];
    private byte[] publicKey = new byte[0];
    private String schemaName = "iroha.android.transaction.Payload.v1";
    private String keyAlias = "";
    private long issuedAtMs = System.currentTimeMillis();
    private Map<String, String> metadata = new LinkedHashMap<>();
    private byte[] exportedKeyBundle;

    public Builder setEncodedPayload(final byte[] encodedPayload) {
      this.encodedPayload = copy(Objects.requireNonNull(encodedPayload, "encodedPayload"));
      return this;
    }

    public Builder setSignature(final byte[] signature) {
      this.signature = copy(Objects.requireNonNull(signature, "signature"));
      return this;
    }

    public Builder setPublicKey(final byte[] publicKey) {
      this.publicKey = copy(Objects.requireNonNull(publicKey, "publicKey"));
      return this;
    }

    public Builder setSchemaName(final String schemaName) {
      this.schemaName = normalize(schemaName, "schemaName");
      return this;
    }

    public Builder setKeyAlias(final String keyAlias) {
      this.keyAlias = normalize(keyAlias, "keyAlias");
      return this;
    }

    public Builder setIssuedAtMs(final long issuedAtMs) {
      if (issuedAtMs < 0) {
        throw new IllegalArgumentException("issuedAtMs must be non-negative");
      }
      this.issuedAtMs = issuedAtMs;
      return this;
    }

    public Builder setMetadata(final Map<String, String> metadata) {
      this.metadata = new LinkedHashMap<>();
      if (metadata != null) {
        metadata.forEach(
            (key, value) ->
                this.metadata.put(normalize(key, "metadata key"), Objects.requireNonNull(value, "metadata value")));
      }
      return this;
    }

    public Builder putMetadata(final String key, final String value) {
      this.metadata.put(normalize(key, "metadata key"), Objects.requireNonNull(value, "metadata value"));
      return this;
    }

    public Builder setExportedKeyBundle(final byte[] exportedKeyBundle) {
      this.exportedKeyBundle =
          exportedKeyBundle == null ? null : copy(exportedKeyBundle);
      return this;
    }

    public OfflineSigningEnvelope build() {
      if (keyAlias.isEmpty()) {
        throw new IllegalStateException("keyAlias must be provided");
      }
      if (encodedPayload.length == 0) {
        throw new IllegalStateException("encodedPayload must not be empty");
      }
      if (signature.length == 0) {
        throw new IllegalStateException("signature must not be empty");
      }
      if (publicKey.length == 0) {
        throw new IllegalStateException("publicKey must not be empty");
      }
      return new OfflineSigningEnvelope(this);
    }
  }
}

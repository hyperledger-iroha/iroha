package org.hyperledger.iroha.android.tx.offline;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;

/** Configuration for building {@link OfflineSigningEnvelope} instances. */
public final class OfflineEnvelopeOptions {
  private final long issuedAtMs;
  private final Map<String, String> metadata;
  private final byte[] exportedKeyBundle;

  private OfflineEnvelopeOptions(final Builder builder) {
    this.issuedAtMs = builder.issuedAtMs;
    this.metadata = Collections.unmodifiableMap(new LinkedHashMap<>(builder.metadata));
    this.exportedKeyBundle =
        builder.exportedKeyBundle == null ? null : builder.exportedKeyBundle.clone();
  }

  public long issuedAtMs() {
    return issuedAtMs;
  }

  public Map<String, String> metadata() {
    return metadata;
  }

  public Optional<byte[]> exportedKeyBundle() {
    return exportedKeyBundle == null ? Optional.empty() : Optional.of(exportedKeyBundle.clone());
  }

  public static Builder builder() {
    return new Builder();
  }

  public static Builder builderFromDefaults() {
    return new Builder();
  }

  public static final class Builder {
    private long issuedAtMs = System.currentTimeMillis();
    private final Map<String, String> metadata = new LinkedHashMap<>();
    private byte[] exportedKeyBundle;

    public Builder setIssuedAtMs(final long issuedAtMs) {
      if (issuedAtMs < 0) {
        throw new IllegalArgumentException("issuedAtMs must be non-negative");
      }
      this.issuedAtMs = issuedAtMs;
      return this;
    }

    public Builder setMetadata(final Map<String, String> metadata) {
      this.metadata.clear();
      if (metadata != null) {
        metadata.forEach((key, value) -> this.metadata.put(Objects.requireNonNull(key, "metadata key"), Objects.requireNonNull(value, "metadata value")));
      }
      return this;
    }

    public Builder putMetadata(final String key, final String value) {
      this.metadata.put(Objects.requireNonNull(key, "metadata key"), Objects.requireNonNull(value, "metadata value"));
      return this;
    }

    public Builder setExportedKeyBundle(final byte[] exportedKeyBundle) {
      this.exportedKeyBundle = exportedKeyBundle == null ? null : exportedKeyBundle.clone();
      return this;
    }

    public OfflineEnvelopeOptions build() {
      return new OfflineEnvelopeOptions(this);
    }
  }
}

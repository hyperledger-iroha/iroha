package org.hyperledger.iroha.android.nexus;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

/** Input for the V1 numeric asset transfer flow. */
public final class NexusTransferInput {

  private final String sourceAssetId;
  private final String quantity;
  private final String destinationAccountId;
  private final String authority;
  private final byte[] signingPublicKey;
  private final Long creationTimeMs;
  private final Long ttlMs;
  private final Integer nonce;
  private final Map<String, String> metadata;

  public NexusTransferInput(
      final String sourceAssetId, final String quantity, final String destinationAccountId) {
    this(builder()
        .sourceAssetId(sourceAssetId)
        .quantity(quantity)
        .destinationAccountId(destinationAccountId));
  }

  private NexusTransferInput(final Builder builder) {
    this.sourceAssetId = NexusModelUtils.requireNonBlank(builder.sourceAssetId, "sourceAssetId");
    this.quantity = NexusModelUtils.requireNonBlank(builder.quantity, "quantity");
    this.destinationAccountId =
        NexusModelUtils.requireNonBlank(builder.destinationAccountId, "destinationAccountId");
    this.authority = builder.authority;
    this.signingPublicKey = NexusModelUtils.copy(builder.signingPublicKey);
    this.creationTimeMs = builder.creationTimeMs;
    this.ttlMs = builder.ttlMs;
    this.nonce = builder.nonce;
    this.metadata = NexusModelUtils.copyMap(builder.metadata);
  }

  public Builder toBuilder() {
    return builder()
        .sourceAssetId(sourceAssetId)
        .quantity(quantity)
        .destinationAccountId(destinationAccountId)
        .authority(authority)
        .signingPublicKey(signingPublicKey)
        .creationTimeMs(creationTimeMs)
        .ttlMs(ttlMs)
        .nonce(nonce)
        .metadata(metadata);
  }

  public String sourceAssetId() {
    return sourceAssetId;
  }

  public String quantity() {
    return quantity;
  }

  public String destinationAccountId() {
    return destinationAccountId;
  }

  public String authority() {
    return authority;
  }

  public byte[] signingPublicKey() {
    return NexusModelUtils.copy(signingPublicKey);
  }

  public Long creationTimeMs() {
    return creationTimeMs;
  }

  public Long ttlMs() {
    return ttlMs;
  }

  public Integer nonce() {
    return nonce;
  }

  public Map<String, String> metadata() {
    return metadata;
  }

  public static Builder builder() {
    return new Builder();
  }

  public static final class Builder {
    private String sourceAssetId;
    private String quantity;
    private String destinationAccountId;
    private String authority;
    private byte[] signingPublicKey;
    private Long creationTimeMs;
    private Long ttlMs;
    private Integer nonce;
    private Map<String, String> metadata = Collections.emptyMap();

    private Builder() {}

    public Builder sourceAssetId(final String sourceAssetId) {
      this.sourceAssetId = sourceAssetId;
      return this;
    }

    public Builder quantity(final String quantity) {
      this.quantity = quantity;
      return this;
    }

    public Builder destinationAccountId(final String destinationAccountId) {
      this.destinationAccountId = destinationAccountId;
      return this;
    }

    public Builder authority(final String authority) {
      this.authority = authority;
      return this;
    }

    public Builder signingPublicKey(final byte[] signingPublicKey) {
      this.signingPublicKey = NexusModelUtils.copy(signingPublicKey);
      return this;
    }

    public Builder creationTimeMs(final Long creationTimeMs) {
      this.creationTimeMs = creationTimeMs;
      return this;
    }

    public Builder ttlMs(final Long ttlMs) {
      this.ttlMs = ttlMs;
      return this;
    }

    public Builder nonce(final Integer nonce) {
      this.nonce = nonce;
      return this;
    }

    public Builder metadata(final Map<String, String> metadata) {
      this.metadata = metadata == null ? Collections.emptyMap() : new LinkedHashMap<>(metadata);
      return this;
    }

    public NexusTransferInput build() {
      return new NexusTransferInput(this);
    }
  }
}

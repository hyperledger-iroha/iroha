package org.hyperledger.iroha.android.nexus;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

/** Query parameters for `/v1/space-directory/uaids/{uaid}/manifests`. */
public final class UaidManifestQuery {
  private final Long dataspaceId;
  private final UaidManifestStatusFilter status;
  private final Long limit;
  private final Long offset;

  private UaidManifestQuery(final Builder builder) {
    this.dataspaceId = builder.dataspaceId;
    this.status = builder.status;
    this.limit = builder.limit;
    this.offset = builder.offset;
  }

  public static Builder builder() {
    return new Builder();
  }

  public Long dataspaceId() {
    return dataspaceId;
  }

  public UaidManifestStatusFilter status() {
    return status;
  }

  public Long limit() {
    return limit;
  }

  public Long offset() {
    return offset;
  }

  /** Serialises the query into URL parameters suitable for Torii. */
  public Map<String, String> toQueryParameters() {
    final Map<String, String> params = new LinkedHashMap<>();
    if (dataspaceId != null) {
      params.put("dataspace", String.valueOf(dataspaceId));
    }
    if (status != null) {
      params.put("status", status.parameterValue());
    }
    if (limit != null) {
      params.put("limit", String.valueOf(limit));
    }
    if (offset != null) {
      params.put("offset", String.valueOf(offset));
    }
    return Collections.unmodifiableMap(params);
  }

  /** Builder for {@link UaidManifestQuery}. */
  public static final class Builder {
    private Long dataspaceId;
    private UaidManifestStatusFilter status;
    private Long limit;
    private Long offset;

    private Builder() {}

    public Builder setDataspaceId(final Long dataspaceId) {
      if (dataspaceId != null && dataspaceId < 0) {
        throw new IllegalArgumentException("dataspaceId must be non-negative");
      }
      this.dataspaceId = dataspaceId;
      return this;
    }

    public Builder setStatus(final UaidManifestStatusFilter status) {
      this.status = status;
      return this;
    }

    public Builder setLimit(final Long limit) {
      if (limit != null && limit < 0) {
        throw new IllegalArgumentException("limit must be non-negative");
      }
      this.limit = limit;
      return this;
    }

    public Builder setOffset(final Long offset) {
      if (offset != null && offset < 0) {
        throw new IllegalArgumentException("offset must be non-negative");
      }
      this.offset = offset;
      return this;
    }

    public UaidManifestQuery build() {
      return new UaidManifestQuery(this);
    }
  }

  /** Status filter accepted by Torii manifests endpoint. */
  public enum UaidManifestStatusFilter {
    ACTIVE("active"),
    INACTIVE("inactive"),
    ALL("all");

    private final String parameter;

    UaidManifestStatusFilter(final String parameter) {
      this.parameter = parameter;
    }

    String parameterValue() {
      return parameter;
    }
  }
}

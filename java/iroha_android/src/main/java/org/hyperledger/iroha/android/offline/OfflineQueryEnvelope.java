package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Optional;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;

/** JSON envelope used by the Torii offline query POST handlers. */
public final class OfflineQueryEnvelope {

  private final Object filter;
  private final Object sort;
  private final Long limit;
  private final Long offset;

  private OfflineQueryEnvelope(final Builder builder) {
    this.filter = builder.filter;
    this.sort = builder.sort;
    this.limit = builder.limit;
    this.offset = builder.offset;
  }

  public byte[] toJsonBytes() {
    final Map<String, Object> json = new LinkedHashMap<>();
    if (filter != null) {
      json.put("filter", filter);
    }
    if (sort != null) {
      json.put("sort", sort);
    }
    if (limit != null) {
      json.put("limit", limit);
    }
    if (offset != null) {
      json.put("offset", offset);
    }
    return JsonEncoder.encode(json).getBytes(StandardCharsets.UTF_8);
  }

  public static Builder builder() {
    return new Builder();
  }

  public static OfflineQueryEnvelope fromListParams(final OfflineListParams params) {
    if (params == null) {
      return builder().build();
    }
    final Builder builder = builder();
    params.filter().map(OfflineQueryEnvelope::parseJson).ifPresent(builder::setFilterNode);
    params.sort().map(OfflineQueryEnvelope::parseJson).ifPresent(builder::setSortNode);
    params.limit().ifPresent(builder::setLimit);
    params.offset().ifPresent(builder::setOffset);
    return builder.build();
  }

  private static Object parseJson(final String json) {
    final String trimmed = Optional.ofNullable(json).map(String::trim).orElse("");
    if (trimmed.isEmpty()) {
      return null;
    }
    return JsonParser.parse(trimmed);
  }

  public static final class Builder {
    private Object filter;
    private Object sort;
    private Long limit;
    private Long offset;

    private Builder() {}

    public Builder filterJson(final String json) {
      this.filter = parseJson(json);
      return this;
    }

    public Builder sortJson(final String json) {
      this.sort = parseJson(json);
      return this;
    }

    Builder setFilterNode(final Object filter) {
      this.filter = filter;
      return this;
    }

    Builder setSortNode(final Object sort) {
      this.sort = sort;
      return this;
    }

    public Builder setLimit(final Long limit) {
      if (limit != null && limit < 0) {
        throw new IllegalArgumentException("limit must be positive");
      }
      this.limit = limit;
      return this;
    }

    public Builder setOffset(final Long offset) {
      if (offset != null && offset < 0) {
        throw new IllegalArgumentException("offset must be positive");
      }
      this.offset = offset;
      return this;
    }

    public OfflineQueryEnvelope build() {
      return new OfflineQueryEnvelope(this);
    }
  }
}

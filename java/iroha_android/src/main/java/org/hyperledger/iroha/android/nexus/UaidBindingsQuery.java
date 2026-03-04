package org.hyperledger.iroha.android.nexus;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

/** Query parameters for `/v1/space-directory/uaids/{uaid}` bindings endpoint. */
public final class UaidBindingsQuery {
  private final AddressFormatOption addressFormat;

  private UaidBindingsQuery(final Builder builder) {
    this.addressFormat = builder.addressFormat;
  }

  public static Builder builder() {
    return new Builder();
  }

  public AddressFormatOption addressFormat() {
    return addressFormat;
  }

  public Map<String, String> toQueryParameters() {
    final Map<String, String> params = new LinkedHashMap<>();
    if (addressFormat != null) {
      params.put("address_format", addressFormat.parameterValue());
    }
    return Collections.unmodifiableMap(params);
  }

  /** Builder for {@link UaidBindingsQuery}. */
  public static final class Builder {
    private AddressFormatOption addressFormat;

    private Builder() {}

    public Builder setAddressFormat(final AddressFormatOption addressFormat) {
      this.addressFormat = addressFormat;
      return this;
    }

    public UaidBindingsQuery build() {
      return new UaidBindingsQuery(this);
    }
  }
}

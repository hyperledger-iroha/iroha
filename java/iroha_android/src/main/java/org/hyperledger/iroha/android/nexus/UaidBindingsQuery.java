package org.hyperledger.iroha.android.nexus;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

/** Query parameters for `/v1/space-directory/uaids/{uaid}` bindings endpoint. */
public final class UaidBindingsQuery {
  private UaidBindingsQuery(final Builder builder) {}

  public static Builder builder() {
    return new Builder();
  }

  public Map<String, String> toQueryParameters() {
    return Collections.emptyMap();
  }

  /** Builder for {@link UaidBindingsQuery}. */
  public static final class Builder {
    private Builder() {}

    public UaidBindingsQuery build() {
      return new UaidBindingsQuery(this);
    }
  }
}

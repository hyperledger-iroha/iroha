package org.hyperledger.iroha.android.client.stream;

import java.time.Duration;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Optional request customisations for Torii event streams. */
public final class ToriiEventStreamOptions {

  private final Map<String, String> queryParameters;
  private final Map<String, String> headers;
  private final Duration timeout;

  private ToriiEventStreamOptions(final Builder builder) {
    this.queryParameters = Collections.unmodifiableMap(new LinkedHashMap<>(builder.queryParameters));
    this.headers = Collections.unmodifiableMap(new LinkedHashMap<>(builder.headers));
    this.timeout = builder.timeout;
  }

  public static Builder builder() {
    return new Builder();
  }

  public static ToriiEventStreamOptions defaultOptions() {
    return builder().build();
  }

  public Map<String, String> queryParameters() {
    return queryParameters;
  }

  public Map<String, String> headers() {
    return headers;
  }

  public Duration timeout() {
    return timeout;
  }

  public static final class Builder {
    private final Map<String, String> queryParameters = new LinkedHashMap<>();
    private final Map<String, String> headers = new LinkedHashMap<>();
    private Duration timeout;

    private Builder() {}

    public Builder putQueryParameter(final String key, final String value) {
      Objects.requireNonNull(key, "key");
      Objects.requireNonNull(value, "value");
      queryParameters.put(key, value);
      return this;
    }

    public Builder queryParameters(final Map<String, String> parameters) {
      queryParameters.clear();
      if (parameters != null) {
        parameters.forEach(this::putQueryParameter);
      }
      return this;
    }

    public Builder putHeader(final String name, final String value) {
      Objects.requireNonNull(name, "name");
      Objects.requireNonNull(value, "value");
      headers.put(name, value);
      return this;
    }

    public Builder headers(final Map<String, String> values) {
      headers.clear();
      if (values != null) {
        values.forEach(this::putHeader);
      }
      return this;
    }

    public Builder setTimeout(final Duration timeout) {
      if (timeout == null) {
        this.timeout = null;
      } else {
        this.timeout = timeout.isNegative() ? Duration.ZERO : timeout;
      }
      return this;
    }

    public ToriiEventStreamOptions build() {
      return new ToriiEventStreamOptions(this);
    }
  }
}

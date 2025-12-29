package org.hyperledger.iroha.android.client.websocket;

import java.time.Duration;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;

/** Configuration for Torii WebSocket connections. */
public final class ToriiWebSocketOptions {

  private final Map<String, String> queryParameters;
  private final Map<String, String> headers;
  private final List<String> subprotocols;
  private final Duration connectTimeout;

  private ToriiWebSocketOptions(final Builder builder) {
    this.queryParameters = Collections.unmodifiableMap(new LinkedHashMap<>(builder.queryParameters));
    this.headers = Collections.unmodifiableMap(new LinkedHashMap<>(builder.headers));
    this.subprotocols = Collections.unmodifiableList(new ArrayList<>(builder.subprotocols));
    this.connectTimeout = builder.connectTimeout;
  }

  public static Builder builder() {
    return new Builder();
  }

  public static ToriiWebSocketOptions defaultOptions() {
    return builder().build();
  }

  public Map<String, String> queryParameters() {
    return queryParameters;
  }

  public Map<String, String> headers() {
    return headers;
  }

  public List<String> subprotocols() {
    return subprotocols;
  }

  public Duration connectTimeout() {
    return connectTimeout;
  }

  public static final class Builder {
    private final Map<String, String> queryParameters = new LinkedHashMap<>();
    private final Map<String, String> headers = new LinkedHashMap<>();
    private final List<String> subprotocols = new ArrayList<>();
    private Duration connectTimeout;

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

    public Builder addSubprotocol(final String value) {
      Objects.requireNonNull(value, "value");
      if (!value.isBlank()) {
        subprotocols.add(value);
      }
      return this;
    }

    public Builder subprotocols(final List<String> values) {
      subprotocols.clear();
      if (values != null) {
        values.forEach(this::addSubprotocol);
      }
      return this;
    }

    public Builder setConnectTimeout(final Duration timeout) {
      if (timeout == null) {
        this.connectTimeout = null;
      } else {
        this.connectTimeout = timeout.isNegative() ? Duration.ZERO : timeout;
      }
      return this;
    }

    public ToriiWebSocketOptions build() {
      return new ToriiWebSocketOptions(this);
    }
  }
}

package org.hyperledger.iroha.android.client;

import java.time.Duration;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/**
 * Optional request overrides for {@link NoritoRpcClient#call(String, byte[], NoritoRpcRequestOptions)}.
 */
public final class NoritoRpcRequestOptions {

  private final Duration timeout;
  private final Map<String, String> headers;
  private final Map<String, String> queryParameters;
  private final String method;
  private final String accept;
  private final boolean acceptConfigured;

  private NoritoRpcRequestOptions(final Builder builder) {
    this.timeout = builder.timeout;
    this.headers = Collections.unmodifiableMap(new LinkedHashMap<>(builder.headers));
    this.queryParameters =
        Collections.unmodifiableMap(new LinkedHashMap<>(builder.queryParameters));
    this.method = builder.method;
    this.accept = builder.accept;
    this.acceptConfigured = builder.acceptConfigured;
  }

  static NoritoRpcRequestOptions defaultOptions() {
    return builder().build();
  }

  public Duration timeout() {
    return timeout;
  }

  public Map<String, String> headers() {
    return headers;
  }

  public Map<String, String> queryParameters() {
    return queryParameters;
  }

  public String method() {
    return method;
  }

  boolean acceptConfigured() {
    return acceptConfigured;
  }

  public String accept() {
    return accept;
  }

  public static Builder builder() {
    return new Builder();
  }

  public static final class Builder {
    private static final String DEFAULT_METHOD = "POST";
    private Duration timeout = null;
    private final Map<String, String> headers = new LinkedHashMap<>();
    private final Map<String, String> queryParameters = new LinkedHashMap<>();
    private String method = DEFAULT_METHOD;
    private String accept = NoritoRpcClient.DEFAULT_ACCEPT;
    private boolean acceptConfigured = false;

    private Builder() {}

    public Builder timeout(final Duration timeout) {
      if (timeout == null) {
        this.timeout = null;
      } else {
        this.timeout = timeout.isNegative() ? Duration.ZERO : timeout;
      }
      return this;
    }

    public Builder putHeader(final String name, final String value) {
      headers.put(
          Objects.requireNonNull(name, "name"), Objects.requireNonNull(value, "value"));
      return this;
    }

    public Builder headers(final Map<String, String> headers) {
      this.headers.clear();
      if (headers != null) {
        headers.forEach(this::putHeader);
      }
      return this;
    }

    public Builder putQueryParameter(final String name, final String value) {
      queryParameters.put(
          Objects.requireNonNull(name, "name"), Objects.requireNonNull(value, "value"));
      return this;
    }

    public Builder queryParameters(final Map<String, String> queryParameters) {
      this.queryParameters.clear();
      if (queryParameters != null) {
        queryParameters.forEach(this::putQueryParameter);
      }
      return this;
    }

    public Builder method(final String method) {
      if (method == null || method.isBlank()) {
        this.method = DEFAULT_METHOD;
      } else {
        this.method = method.toUpperCase();
      }
      return this;
    }

    /**
     * Overrides the Accept header for the request. Pass {@code null} to explicitly omit the header.
     */
    public Builder accept(final String accept) {
      this.accept = accept;
      this.acceptConfigured = true;
      return this;
    }

    public NoritoRpcRequestOptions build() {
      return new NoritoRpcRequestOptions(this);
    }
  }
}

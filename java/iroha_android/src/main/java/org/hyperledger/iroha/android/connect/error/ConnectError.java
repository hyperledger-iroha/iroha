package org.hyperledger.iroha.android.connect.error;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;

/** Canonical error payload surfaced by the Connect SDK. */
public final class ConnectError extends RuntimeException implements ConnectErrorConvertible {
  private static final long serialVersionUID = 1L;
  private final ConnectErrorCategory category;
  private final String code;
  private final boolean fatal;
  private final Integer httpStatus;
  private final String underlying;

  private ConnectError(final Builder builder) {
    super(
        builder.message != null ? builder.message : Objects.requireNonNull(builder.code, "code"),
        builder.cause);
    this.category = Objects.requireNonNull(builder.category, "category");
    this.code = Objects.requireNonNull(builder.code, "code");
    this.fatal = builder.fatal;
    this.httpStatus = builder.httpStatus;
    this.underlying = builder.underlying;
  }

  public static Builder builder() {
    return new Builder();
  }

  public Builder toBuilder() {
    return new Builder()
        .category(category)
        .code(code)
        .message(getMessage())
        .fatal(fatal)
        .httpStatus(httpStatus)
        .underlying(underlying)
        .cause(getCause());
  }

  public ConnectErrorCategory category() {
    return category;
  }

  public String code() {
    return code;
  }

  public boolean fatal() {
    return fatal;
  }

  public Optional<Integer> httpStatus() {
    return Optional.ofNullable(httpStatus);
  }

  public Optional<String> underlying() {
    return Optional.ofNullable(underlying);
  }

  public Map<String, String> telemetryAttributes() {
    return telemetryAttributes(ConnectErrorTelemetryOptions.empty());
  }

  public Map<String, String> telemetryAttributes(
      final ConnectErrorTelemetryOptions options) {
    final ConnectErrorTelemetryOptions effective =
        options == null ? ConnectErrorTelemetryOptions.empty() : options;
    final LinkedHashMap<String, String> attributes = new LinkedHashMap<>();
    attributes.put("category", labelFor(category));
    attributes.put("code", code);
    final boolean fatalValue =
        effective.fatal() != null ? effective.fatal() : this.fatal;
    attributes.put("fatal", Boolean.toString(fatalValue));
    final Integer httpValue =
        effective.httpStatus() != null ? effective.httpStatus() : httpStatus;
    if (httpValue != null) {
      attributes.put("http_status", Integer.toString(httpValue));
    }
    final String underlyingValue =
        effective.underlying() != null ? effective.underlying() : underlying;
    if (underlyingValue != null && !underlyingValue.isEmpty()) {
      attributes.put("underlying", underlyingValue);
    }
    return attributes;
  }

  @Override
  public ConnectError toConnectError() {
    return this;
  }

  private static String labelFor(final ConnectErrorCategory category) {
    return switch (category) {
      case TRANSPORT -> "transport";
      case CODEC -> "codec";
      case AUTHORIZATION -> "authorization";
      case TIMEOUT -> "timeout";
      case QUEUE_OVERFLOW -> "queueOverflow";
      case INTERNAL -> "internal";
    };
  }

  public static final class Builder {
    private ConnectErrorCategory category = ConnectErrorCategory.INTERNAL;
    private String code = "unknown_error";
    private String message;
    private boolean fatal;
    private Integer httpStatus;
    private String underlying;
    private Throwable cause;

    public Builder category(final ConnectErrorCategory category) {
      this.category = Objects.requireNonNull(category, "category");
      return this;
    }

    public Builder code(final String code) {
      this.code = code != null ? code : "unknown_error";
      return this;
    }

    public Builder message(final String message) {
      this.message = message;
      return this;
    }

    public Builder fatal(final boolean fatal) {
      this.fatal = fatal;
      return this;
    }

    public Builder httpStatus(final Integer httpStatus) {
      this.httpStatus = httpStatus;
      return this;
    }

    public Builder underlying(final String underlying) {
      this.underlying = underlying;
      return this;
    }

    public Builder cause(final Throwable cause) {
      this.cause = cause;
      return this;
    }

    public ConnectError build() {
      return new ConnectError(this);
    }
  }
}

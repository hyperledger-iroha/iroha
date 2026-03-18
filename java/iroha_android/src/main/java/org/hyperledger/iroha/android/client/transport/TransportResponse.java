package org.hyperledger.iroha.android.client.transport;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.TreeMap;

/** SDK-owned transport response wrapper to decouple callers from {@code java.net.http.HttpResponse}. */
public final class TransportResponse {

  private final int statusCode;
  private final byte[] body;
  private final String message;
  private final Map<String, List<String>> headers;

  public TransportResponse(
      final int statusCode,
      final byte[] body,
      final String message,
      final Map<String, List<String>> headers) {
    this.statusCode = statusCode;
    this.body = body == null ? new byte[0] : body.clone();
    this.message = message == null ? "" : message;
    this.headers = Collections.unmodifiableMap(copyHeaders(headers));
  }

  public int statusCode() {
    return statusCode;
  }

  public byte[] body() {
    return body.clone();
  }

  public String message() {
    return message;
  }

  public Map<String, List<String>> headers() {
    return headers;
  }

  private static Map<String, List<String>> copyHeaders(final Map<String, List<String>> source) {
    if (source == null) {
      return Map.of();
    }
    final Map<String, List<String>> copy = new TreeMap<>(String.CASE_INSENSITIVE_ORDER);
    for (final Map.Entry<String, List<String>> entry : source.entrySet()) {
      final List<String> values =
          entry.getValue() == null ? List.of() : Collections.unmodifiableList(new ArrayList<>(entry.getValue()));
      copy.put(entry.getKey(), values);
    }
    return copy;
  }

  public static Builder builder() {
    return new Builder();
  }

  public static final class Builder {
    private int statusCode = 0;
    private byte[] body = new byte[0];
    private String message = "";
    private final Map<String, List<String>> headers = new java.util.LinkedHashMap<>();

    public Builder setStatusCode(final int statusCode) {
      this.statusCode = statusCode;
      return this;
    }

    public Builder setBody(final byte[] body) {
      this.body = body == null ? new byte[0] : body.clone();
      return this;
    }

    public Builder setMessage(final String message) {
      this.message = message == null ? "" : message;
      return this;
    }

    public Builder addHeader(final String name, final String value) {
      Objects.requireNonNull(name, "name");
      Objects.requireNonNull(value, "value");
      headers.computeIfAbsent(name, ignored -> new ArrayList<>()).add(value);
      return this;
    }

    public Builder setHeaders(final Map<String, List<String>> headers) {
      this.headers.clear();
      if (headers != null) {
        for (final Map.Entry<String, List<String>> entry : headers.entrySet()) {
          final List<String> values =
              entry.getValue() == null ? List.of() : new ArrayList<>(entry.getValue());
          this.headers.put(entry.getKey(), values);
        }
      }
      return this;
    }

    public TransportResponse build() {
      return new TransportResponse(statusCode, body, message, headers);
    }
  }
}

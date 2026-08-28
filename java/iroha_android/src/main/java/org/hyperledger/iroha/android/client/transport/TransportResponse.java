package org.hyperledger.iroha.android.client.transport;

import java.net.URI;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.TreeMap;

/**
 * SDK-owned transport response wrapper to decouple callers from {@code java.net.http.HttpResponse}.
 *
 * <p>{@link #finalUri()} and {@link #redirected()} carry network provenance for response paths that
 * must remain bound to an exact signed URI. The four-argument constructor remains available for
 * response paths that do not require that provenance; exact signed-response consumers fail closed
 * when {@link #finalUri()} is absent.
 */
public final class TransportResponse {

  private final int statusCode;
  private final byte[] body;
  private final String message;
  private final Map<String, List<String>> headers;
  private final URI finalUri;
  private final boolean redirected;

  public TransportResponse(
      final int statusCode,
      final byte[] body,
      final String message,
      final Map<String, List<String>> headers) {
    this(statusCode, body, message, headers, null, false);
  }

  /** Creates a response with explicit final-URI and redirect provenance. */
  public TransportResponse(
      final int statusCode,
      final byte[] body,
      final String message,
      final Map<String, List<String>> headers,
      final URI finalUri,
      final boolean redirected) {
    this.statusCode = statusCode;
    this.body = body == null ? new byte[0] : body.clone();
    this.message = message == null ? "" : message;
    this.headers = Collections.unmodifiableMap(copyHeaders(headers));
    this.finalUri = finalUri;
    this.redirected = redirected;
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

  /** Returns the final network URI, or {@code null} when the executor did not provide provenance. */
  public URI finalUri() {
    return finalUri;
  }

  /** Returns whether the executor followed any redirect before receiving this response. */
  public boolean redirected() {
    return redirected;
  }

  private static Map<String, List<String>> copyHeaders(final Map<String, List<String>> source) {
    if (source == null) {
      return Collections.emptyMap();
    }
    final Map<String, List<String>> merged = new TreeMap<>(String.CASE_INSENSITIVE_ORDER);
    for (final Map.Entry<String, List<String>> entry : source.entrySet()) {
      final List<String> values =
          merged.computeIfAbsent(entry.getKey(), ignored -> new ArrayList<>());
      if (entry.getValue() != null) {
        values.addAll(entry.getValue());
      }
    }
    final Map<String, List<String>> copy = new TreeMap<>(String.CASE_INSENSITIVE_ORDER);
    merged.forEach(
        (name, values) ->
            copy.put(name, Collections.unmodifiableList(new ArrayList<>(values))));
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
    private URI finalUri;
    private boolean redirected;

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
              entry.getValue() == null
                  ? Collections.emptyList()
                  : new ArrayList<>(entry.getValue());
          this.headers.put(entry.getKey(), values);
        }
      }
      return this;
    }

    /** Records the final network URI and whether any redirect was followed. */
    public Builder setNetworkProvenance(final URI finalUri, final boolean redirected) {
      this.finalUri = Objects.requireNonNull(finalUri, "finalUri");
      this.redirected = redirected;
      return this;
    }

    public TransportResponse build() {
      return new TransportResponse(statusCode, body, message, headers, finalUri, redirected);
    }
  }
}

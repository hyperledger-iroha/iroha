package org.hyperledger.iroha.android.client.transport;

import java.io.ByteArrayInputStream;
import java.io.FilterInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.TreeMap;
import java.util.concurrent.atomic.AtomicBoolean;

/**
 * Streaming transport response that exposes the response body without pre-buffering.
 */
public final class TransportStreamResponse implements AutoCloseable {

  private final int statusCode;
  private final InputStream rawBody;
  private final InputStream body;
  private final String message;
  private final Map<String, List<String>> headers;
  private final Runnable onClose;
  private final AtomicBoolean closed = new AtomicBoolean(false);

  public TransportStreamResponse(
      final int statusCode,
      final InputStream body,
      final String message,
      final Map<String, List<String>> headers,
      final Runnable onClose) {
    this.statusCode = statusCode;
    this.rawBody = body == null ? new ByteArrayInputStream(new byte[0]) : body;
    this.message = message == null ? "" : message;
    this.headers = Collections.unmodifiableMap(copyHeaders(headers));
    this.onClose = onClose;
    this.body =
        new FilterInputStream(this.rawBody) {
          @Override
          public void close() throws IOException {
            TransportStreamResponse.this.close();
          }
        };
  }

  public int statusCode() {
    return statusCode;
  }

  public InputStream body() {
    return body;
  }

  public String message() {
    return message;
  }

  public Map<String, List<String>> headers() {
    return headers;
  }

  @Override
  public void close() {
    if (!closed.compareAndSet(false, true)) {
      return;
    }
    try {
      rawBody.close();
    } catch (final IOException ignored) {
      // Close should not throw for callers.
    }
    if (onClose != null) {
      onClose.run();
    }
  }

  private static Map<String, List<String>> copyHeaders(final Map<String, List<String>> source) {
    if (source == null) {
      return Map.of();
    }
    final Map<String, List<String>> copy = new TreeMap<>(String.CASE_INSENSITIVE_ORDER);
    for (final Map.Entry<String, List<String>> entry : source.entrySet()) {
      final List<String> values =
          entry.getValue() == null
              ? List.of()
              : Collections.unmodifiableList(new ArrayList<>(entry.getValue()));
      copy.put(entry.getKey(), values);
    }
    return copy;
  }
}

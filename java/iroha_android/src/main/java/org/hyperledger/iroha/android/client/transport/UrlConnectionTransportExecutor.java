package org.hyperledger.iroha.android.client.transport;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.io.InputStream;
import java.net.HttpURLConnection;
import java.net.URL;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;

/**
 * {@link HttpTransportExecutor} implementation backed by {@link HttpURLConnection}.
 *
 * <p>This executor avoids {@code java.net.http} so it can serve as a portable fallback across JVM
 * and Android targets. Callers should prefer platform-optimised executors (OkHttp on Android, JDK
 * HTTP client on JVM) when available.
 */
public final class UrlConnectionTransportExecutor
    implements HttpTransportExecutor, StreamingTransportExecutor {

  private final Duration connectTimeout;
  private final Duration readTimeout;

  /** Creates an executor with no explicit timeouts (uses JVM/Android defaults). */
  public UrlConnectionTransportExecutor() {
    this(null, null);
  }

  /** Creates an executor that applies the same timeout to connect and read operations. */
  public UrlConnectionTransportExecutor(final Duration timeout) {
    this(timeout, timeout);
  }

  /** Creates an executor with distinct connect/read timeouts (nullable to use defaults). */
  public UrlConnectionTransportExecutor(final Duration connectTimeout, final Duration readTimeout) {
    this.connectTimeout = connectTimeout;
    this.readTimeout = readTimeout;
  }

  @Override
  public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
    Objects.requireNonNull(request, "request");
    return CompletableFuture.supplyAsync(() -> executeSync(request));
  }

  @Override
  public CompletableFuture<TransportStreamResponse> openStream(final TransportRequest request) {
    Objects.requireNonNull(request, "request");
    return CompletableFuture.supplyAsync(() -> openStreamSync(request));
  }

  private TransportResponse executeSync(final TransportRequest request) {
    HttpURLConnection connection = null;
    try {
      connection = openConnection(request);
      writeRequestBody(request, connection);
      final int status = connection.getResponseCode();
      final String message = emptyIfNull(connection.getResponseMessage());
      final byte[] body = readBody(connection, status);
      final Map<String, List<String>> headers = normalizeHeaders(connection.getHeaderFields());
      return new TransportResponse(status, body, message, headers);
    } catch (final IOException ex) {
      throw new RuntimeException("HTTP request failed", ex);
    } finally {
      if (connection != null) {
        connection.disconnect();
      }
    }
  }

  private TransportStreamResponse openStreamSync(final TransportRequest request) {
    HttpURLConnection connection = null;
    try {
      connection = openConnection(request);
      writeRequestBody(request, connection);
      final int status = connection.getResponseCode();
      final String message = emptyIfNull(connection.getResponseMessage());
      final InputStream stream = responseStream(connection, status);
      final Map<String, List<String>> headers = normalizeHeaders(connection.getHeaderFields());
      final HttpURLConnection target = connection;
      return new TransportStreamResponse(
          status,
          stream,
          message,
          headers,
          target::disconnect);
    } catch (final IOException ex) {
      if (connection != null) {
        connection.disconnect();
      }
      throw new RuntimeException("HTTP request failed", ex);
    }
  }

  private HttpURLConnection openConnection(final TransportRequest request) throws IOException {
    final URL url = request.uri().toURL();
    final HttpURLConnection connection = (HttpURLConnection) url.openConnection();
    connection.setRequestMethod(request.method());
    connection.setDoInput(true);
    final Duration timeout = request.timeout();
    final int connectMs =
        toMillis(timeout != null ? timeout : connectTimeout, connection.getConnectTimeout());
    final int readMs = toMillis(timeout != null ? timeout : readTimeout, connection.getReadTimeout());
    connection.setConnectTimeout(connectMs);
    connection.setReadTimeout(readMs);
    for (final Map.Entry<String, List<String>> header : request.headers().entrySet()) {
      for (final String value : header.getValue()) {
        connection.addRequestProperty(header.getKey(), value);
      }
    }
    final boolean hasBody = request.body().length > 0 && !request.method().equalsIgnoreCase("GET");
    connection.setDoOutput(hasBody);
    return connection;
  }

  private static void writeRequestBody(
      final TransportRequest request, final HttpURLConnection connection) throws IOException {
    final boolean hasBody =
        request.body().length > 0 && !request.method().equalsIgnoreCase("GET");
    if (!hasBody) {
      return;
    }
    connection.getOutputStream().write(request.body());
  }

  private static byte[] readBody(final HttpURLConnection connection, final int status)
      throws IOException {
    final InputStream stream = responseStream(connection, status);
    if (stream == null) {
      return new byte[0];
    }
    try (stream; final ByteArrayOutputStream buffer = new ByteArrayOutputStream()) {
      final byte[] chunk = new byte[4096];
      int read;
      while ((read = stream.read(chunk)) != -1) {
        buffer.write(chunk, 0, read);
      }
      return buffer.toByteArray();
    }
  }

  private static InputStream responseStream(final HttpURLConnection connection, final int status)
      throws IOException {
    if (status >= 400) {
      final InputStream error = connection.getErrorStream();
      return error != null ? error : connection.getInputStream();
    }
    return connection.getInputStream();
  }

  private static Map<String, List<String>> normalizeHeaders(final Map<String, List<String>> raw) {
    if (raw == null) {
      return Map.of();
    }
    final Map<String, List<String>> out = new java.util.LinkedHashMap<>();
    for (final Map.Entry<String, List<String>> entry : raw.entrySet()) {
      if (entry.getKey() == null) {
        continue; // status line
      }
      final List<String> values =
          entry.getValue() == null
              ? List.of()
              : Collections.unmodifiableList(new ArrayList<>(entry.getValue()));
      out.put(entry.getKey(), values);
    }
    return Collections.unmodifiableMap(out);
  }

  private static String emptyIfNull(final String value) {
    return value == null ? "" : value;
  }

  private static int toMillis(final Duration timeout, final int fallback) {
    if (timeout == null) {
      return fallback;
    }
    final long millis = timeout.toMillis();
    if (millis > Integer.MAX_VALUE) {
      return Integer.MAX_VALUE;
    }
    return Math.max(0, (int) millis);
  }
}

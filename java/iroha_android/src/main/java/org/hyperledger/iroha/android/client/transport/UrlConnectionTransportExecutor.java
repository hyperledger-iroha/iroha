package org.hyperledger.iroha.android.client.transport;

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
 * <p>This executor avoids {@code java.net.http} so it can serve JVM and Android targets with the
 * same canonical implementation. It deliberately rejects credential-free requests: the JVM and
 * Android URLConnection stacks expose process-wide authentication caches that cannot be disabled
 * or inspected portably on an individual connection.
 */
public final class UrlConnectionTransportExecutor
    implements HttpTransportExecutor, StreamingTransportExecutor {

  private final Duration connectTimeout;
  private final Duration readTimeout;
  private final long maximumResponseBytes;

  /** Creates an executor with no explicit timeouts (uses JVM/Android defaults). */
  public UrlConnectionTransportExecutor() {
    this(null, null, BoundedResponseBodyReader.DEFAULT_MAXIMUM_RESPONSE_BYTES);
  }

  /** Creates an executor that applies the same timeout to connect and read operations. */
  public UrlConnectionTransportExecutor(final Duration timeout) {
    this(timeout, timeout, BoundedResponseBodyReader.DEFAULT_MAXIMUM_RESPONSE_BYTES);
  }

  /** Creates an executor with distinct connect/read timeouts (nullable to use defaults). */
  public UrlConnectionTransportExecutor(final Duration connectTimeout, final Duration readTimeout) {
    this(
        connectTimeout,
        readTimeout,
        BoundedResponseBodyReader.DEFAULT_MAXIMUM_RESPONSE_BYTES);
  }

  /** Creates an executor with default timeouts and a custom buffered-response limit. */
  public UrlConnectionTransportExecutor(final long maximumResponseBytes) {
    this(null, null, maximumResponseBytes);
  }

  /** Creates an executor with distinct timeouts and a custom buffered-response limit. */
  public UrlConnectionTransportExecutor(
      final Duration connectTimeout,
      final Duration readTimeout,
      final long maximumResponseBytes) {
    BoundedResponseBodyReader.validateMaximum(maximumResponseBytes);
    this.connectTimeout = connectTimeout;
    this.readTimeout = readTimeout;
    this.maximumResponseBytes = maximumResponseBytes;
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
    if (!request.allowAmbientCredentials()) {
      throw credentialFreeUnsupported();
    }
    return executeSyncInternal(request);
  }

  private TransportResponse executeSyncInternal(final TransportRequest request) {
    HttpURLConnection connection = null;
    try {
      connection = openConnection(request);
      writeRequestBody(request, connection);
      final int status = connection.getResponseCode();
      final String message = emptyIfNull(connection.getResponseMessage());
      final long responseLimit = responseLimit(request, maximumResponseBytes);
      final byte[] body =
          readBody(connection, status, request.method(), responseLimit);
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
    if (!request.allowAmbientCredentials()) {
      throw credentialFreeUnsupported();
    }
    return openStreamSyncInternal(request);
  }

  private TransportStreamResponse openStreamSyncInternal(final TransportRequest request) {
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
    // Canonical signatures bind the original URI, and onboarding tokens must never be forwarded
    // to a redirect target by the platform HTTP stack.
    connection.setInstanceFollowRedirects(false);
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
    final byte[] body = request.body();
    final boolean hasBody = body.length > 0 && !request.method().equalsIgnoreCase("GET");
    if (request.replayPolicy() == RequestReplayPolicy.ONE_SHOT) {
      // Avoid stale pooled connections, the main source of transparent URLConnection replays, and
      // use fixed-length streaming to disable URLConnection's internal body retry path.
      connection.setUseCaches(false);
      connection.setRequestProperty("Connection", "close");
      if (hasBody) {
        connection.setFixedLengthStreamingMode(body.length);
      }
    }
    connection.setDoOutput(hasBody);
    return connection;
  }

  private static void writeRequestBody(
      final TransportRequest request, final HttpURLConnection connection) throws IOException {
    final byte[] body = request.body();
    final boolean hasBody = body.length > 0 && !request.method().equalsIgnoreCase("GET");
    if (!hasBody) {
      return;
    }
    connection.getOutputStream().write(body);
  }

  private static byte[] readBody(
      final HttpURLConnection connection,
      final int status,
      final String requestMethod,
      final long maximumResponseBytes)
      throws IOException {
    final Map<String, List<String>> headers = normalizeHeaders(connection.getHeaderFields());
    final boolean bodyExpected = responseMayHaveBody(requestMethod, status);
    if (!bodyExpected) {
      return BoundedResponseBodyReader.read(null, headers, maximumResponseBytes, false);
    }
    final InputStream stream = responseStream(connection, status);
    return BoundedResponseBodyReader.read(stream, headers, maximumResponseBytes, true);
  }

  private static boolean responseMayHaveBody(final String requestMethod, final int status) {
    return !"HEAD".equalsIgnoreCase(requestMethod)
        && (status < 100 || status >= 200)
        && status != HttpURLConnection.HTTP_NO_CONTENT
        && status != HttpURLConnection.HTTP_NOT_MODIFIED;
  }

  private static long responseLimit(
      final TransportRequest request, final long executorMaximumResponseBytes) {
    final Long requestMaximum = request.maximumResponseBytes();
    return requestMaximum == null
        ? executorMaximumResponseBytes
        : Math.min(executorMaximumResponseBytes, requestMaximum.longValue());
  }

  private static InputStream responseStream(final HttpURLConnection connection, final int status)
      throws IOException {
    if (status >= 400) {
      final InputStream error = connection.getErrorStream();
      if (error != null) {
        return error;
      }
      try {
        return connection.getInputStream();
      } catch (final IOException ignored) {
        return null;
      }
    }
    return connection.getInputStream();
  }

  private static Map<String, List<String>> normalizeHeaders(final Map<String, List<String>> raw) {
    if (raw == null) {
      return Collections.emptyMap();
    }
    final Map<String, List<String>> out = new java.util.LinkedHashMap<>();
    for (final Map.Entry<String, List<String>> entry : raw.entrySet()) {
      if (entry.getKey() == null) {
        continue; // status line
      }
      final List<String> values =
          entry.getValue() == null
              ? Collections.emptyList()
              : Collections.unmodifiableList(new ArrayList<>(entry.getValue()));
      out.put(entry.getKey(), values);
    }
    return Collections.unmodifiableMap(out);
  }

  private static String emptyIfNull(final String value) {
    return value == null ? "" : value;
  }

  private static int toMillis(final Duration timeout, final int defaultValue) {
    if (timeout == null) {
      return defaultValue;
    }
    final long millis = timeout.toMillis();
    if (millis > Integer.MAX_VALUE) {
      return Integer.MAX_VALUE;
    }
    return Math.max(0, (int) millis);
  }

  private static IllegalStateException credentialFreeUnsupported() {
    return new IllegalStateException(
        "credential-free requests are unsupported by URLConnection because process-wide "
            + "authentication caches cannot be isolated");
  }
}

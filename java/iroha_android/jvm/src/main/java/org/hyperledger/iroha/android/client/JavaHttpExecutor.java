package org.hyperledger.iroha.android.client;

import java.io.ByteArrayInputStream;
import java.io.IOException;
import java.io.InputStream;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.android.client.transport.BoundedResponseBodyReader;
import org.hyperledger.iroha.android.client.transport.StreamingTransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.client.transport.TransportStreamResponse;

/** Default executor that delegates to {@link HttpClient}. */
final class JavaHttpExecutor implements HttpTransportExecutor, StreamingTransportExecutor {

  private final HttpClient httpClient;
  private final long maximumResponseBytes;
  private static final Set<String> RESTRICTED_HEADERS =
      Set.of("connection", "content-length", "expect", "host", "upgrade");

  JavaHttpExecutor(final HttpClient httpClient) {
    this(httpClient, BoundedResponseBodyReader.DEFAULT_MAXIMUM_RESPONSE_BYTES);
  }

  JavaHttpExecutor(final HttpClient httpClient, final long maximumResponseBytes) {
    this.httpClient = Objects.requireNonNull(httpClient, "httpClient");
    BoundedResponseBodyReader.validateMaximum(maximumResponseBytes);
    this.maximumResponseBytes = maximumResponseBytes;
  }

  @Override
  public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
    final HttpRequest httpRequest = buildRequest(request);
    final long responseLimit = responseLimit(request, maximumResponseBytes);
    return httpClient
        .sendAsync(httpRequest, HttpResponse.BodyHandlers.ofInputStream())
        .thenApply(
            response -> {
              try {
                final byte[] body =
                    BoundedResponseBodyReader.read(
                        response.body(),
                        response.headers().map(),
                        responseLimit,
                        responseMayHaveBody(httpRequest.method(), response.statusCode()));
                return new TransportResponse(
                    response.statusCode(),
                    body,
                    response.statusCode() >= 400 ? httpRequest.uri().toString() : "",
                    response.headers().map());
              } catch (final IOException ex) {
                throw new CompletionException("HTTP response body rejected", ex);
              }
            });
  }

  @Override
  public CompletableFuture<TransportStreamResponse> openStream(final TransportRequest request) {
    final HttpRequest httpRequest = buildRequest(request);
    return httpClient
        .sendAsync(httpRequest, HttpResponse.BodyHandlers.ofInputStream())
        .thenApply(
            response -> {
              final InputStream body =
                  response.body() == null ? new ByteArrayInputStream(new byte[0]) : response.body();
              return new TransportStreamResponse(
                  response.statusCode(),
                  body,
                  response.statusCode() >= 400 ? httpRequest.uri().toString() : "",
                  response.headers().map(),
                  () -> {});
            });
  }

  @Override
  public boolean supportsClientUnwrap() {
    return true;
  }

  public HttpClient unwrapHttpClient() {
    return httpClient;
  }

  private static boolean isRestrictedHeader(final String name) {
    if (name == null) {
      return true;
    }
    return RESTRICTED_HEADERS.contains(name.toLowerCase(Locale.ROOT));
  }

  private static boolean responseMayHaveBody(final String requestMethod, final int status) {
    return !"HEAD".equalsIgnoreCase(requestMethod)
        && (status < 100 || status >= 200)
        && status != 204
        && status != 304;
  }

  private static long responseLimit(
      final TransportRequest request, final long executorMaximumResponseBytes) {
    final Long requestMaximum = request.maximumResponseBytes();
    return requestMaximum == null
        ? executorMaximumResponseBytes
        : Math.min(executorMaximumResponseBytes, requestMaximum.longValue());
  }

  private static HttpRequest buildRequest(final TransportRequest request) {
    final HttpRequest.BodyPublisher publisher =
        request.body().length == 0
            ? HttpRequest.BodyPublishers.noBody()
            : HttpRequest.BodyPublishers.ofByteArray(request.body());
    final HttpRequest.Builder builder =
        HttpRequest.newBuilder(request.uri()).method(request.method(), publisher);
    request
        .headers()
        .forEach(
            (name, values) -> {
              if (isRestrictedHeader(name)) {
                return;
              }
              for (final String value : values) {
                builder.header(name, value);
              }
            });
    if (request.timeout() != null && !request.timeout().isZero()) {
      builder.timeout(request.timeout());
    }
    return builder.build();
  }
}

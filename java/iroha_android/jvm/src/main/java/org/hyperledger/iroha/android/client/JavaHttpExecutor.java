package org.hyperledger.iroha.android.client;

import java.io.ByteArrayInputStream;
import java.io.InputStream;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.client.transport.TransportStreamResponse;
import org.hyperledger.iroha.android.client.transport.StreamingTransportExecutor;

/** Default executor that delegates to {@link HttpClient}. */
final class JavaHttpExecutor implements HttpTransportExecutor, StreamingTransportExecutor {

  private final HttpClient httpClient;
  private static final Set<String> RESTRICTED_HEADERS =
      Set.of("connection", "content-length", "expect", "host", "upgrade");

  JavaHttpExecutor(final HttpClient httpClient) {
    this.httpClient = Objects.requireNonNull(httpClient, "httpClient");
  }

  @Override
  public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
    final HttpRequest httpRequest = buildRequest(request);
    return httpClient
        .sendAsync(httpRequest, HttpResponse.BodyHandlers.ofByteArray())
        .thenApply(
            response ->
                new TransportResponse(
                    response.statusCode(),
                    response.body(),
                    response.statusCode() >= 400 ? httpRequest.uri().toString() : "",
                    response.headers().map()));
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

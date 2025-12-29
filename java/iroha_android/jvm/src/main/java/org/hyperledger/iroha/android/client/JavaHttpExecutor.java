package org.hyperledger.iroha.android.client;

import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

/** Default executor that delegates to {@link HttpClient}. */
final class JavaHttpExecutor implements HttpTransportExecutor {

  private final HttpClient httpClient;

  JavaHttpExecutor(final HttpClient httpClient) {
    this.httpClient = Objects.requireNonNull(httpClient, "httpClient");
  }

  @Override
  public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
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
              for (final String value : values) {
                builder.header(name, value);
              }
            });
    if (request.timeout() != null && !request.timeout().isZero()) {
      builder.timeout(request.timeout());
    }
    final HttpRequest httpRequest = builder.build();
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
  public boolean supportsClientUnwrap() {
    return true;
  }

  public HttpClient unwrapHttpClient() {
    return httpClient;
  }
}

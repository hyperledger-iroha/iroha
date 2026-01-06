package org.hyperledger.iroha.android.client.okhttp;

import java.io.IOException;
import java.time.Duration;
import java.io.ByteArrayInputStream;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;
import okhttp3.Call;
import okhttp3.Callback;
import okhttp3.Headers;
import okhttp3.MediaType;
import okhttp3.OkHttpClient;
import okhttp3.Request;
import okhttp3.RequestBody;
import okhttp3.Response;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.client.transport.TransportStreamResponse;
import org.hyperledger.iroha.android.client.transport.StreamingTransportExecutor;

/** OkHttp-backed {@link HttpTransportExecutor} for Android runtimes. */
public final class OkHttpTransportExecutor
    implements HttpTransportExecutor, StreamingTransportExecutor {

  private final OkHttpClient client;

  public OkHttpTransportExecutor(final OkHttpClient client) {
    this.client = Objects.requireNonNull(client, "client");
  }

  @Override
  public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
    Objects.requireNonNull(request, "request");
    final Request okRequest = buildRequest(request);
    final Call call = client.newCall(okRequest);
    final Duration timeout = request.timeout();
    if (timeout != null && !timeout.isZero() && !timeout.isNegative()) {
      call.timeout().timeout(Math.max(0L, timeout.toMillis()), TimeUnit.MILLISECONDS);
    }

    final CompletableFuture<TransportResponse> future = new CompletableFuture<>();
    call.enqueue(
        new Callback() {
          @Override
          public void onFailure(final Call call, final IOException e) {
            future.completeExceptionally(e);
          }

          @Override
          public void onResponse(final Call call, final Response response) {
            try (response) {
              final byte[] bodyBytes =
                  response.body() == null ? new byte[0] : response.body().bytes();
              final TransportResponse transportResponse =
                  new TransportResponse(
                      response.code(), bodyBytes, response.message(), response.headers().toMultimap());
              future.complete(transportResponse);
            } catch (final IOException e) {
              future.completeExceptionally(e);
            }
          }
        });
    future.whenComplete(
        (ignored, throwable) -> {
          if (future.isCancelled()) {
            call.cancel();
          }
        });
    return future;
  }

  @Override
  public CompletableFuture<TransportStreamResponse> openStream(final TransportRequest request) {
    Objects.requireNonNull(request, "request");
    final Request okRequest = buildRequest(request);
    final Call call = client.newCall(okRequest);
    final java.time.Duration timeout = request.timeout();
    if (timeout != null && !timeout.isNegative()) {
      call.timeout().timeout(Math.max(0L, timeout.toMillis()), TimeUnit.MILLISECONDS);
    }

    final CompletableFuture<TransportStreamResponse> future = new CompletableFuture<>();
    call.enqueue(
        new Callback() {
          @Override
          public void onFailure(final Call call, final IOException e) {
            future.completeExceptionally(e);
          }

          @Override
          public void onResponse(final Call call, final Response response) {
            final java.io.InputStream bodyStream =
                response.body() == null
                    ? new ByteArrayInputStream(new byte[0])
                    : response.body().byteStream();
            final TransportStreamResponse transportResponse =
                new TransportStreamResponse(
                    response.code(),
                    bodyStream,
                    response.message(),
                    response.headers().toMultimap(),
                    () -> {
                      response.close();
                      call.cancel();
                    });
            future.complete(transportResponse);
          }
        });
    future.whenComplete(
        (ignored, throwable) -> {
          if (future.isCancelled()) {
            call.cancel();
          }
        });
    return future;
  }

  @Override
  public boolean supportsClientUnwrap() {
    return true;
  }

  /** Exposes the underlying OkHttp client for callers that need connection pooling reuse. */
  public OkHttpClient unwrapClient() {
    return client;
  }

  private Request buildRequest(final TransportRequest request) {
    final Request.Builder builder = new Request.Builder().url(request.uri().toString());
    applyHeaders(builder, request.headers());
    final RequestBody body = buildRequestBody(request);
    builder.method(request.method(), body);
    return builder.build();
  }

  private static void applyHeaders(
      final Request.Builder builder, final Map<String, List<String>> headers) {
    if (headers == null || headers.isEmpty()) {
      return;
    }
    final Headers.Builder headersBuilder = new Headers.Builder();
    headers.forEach(
        (name, values) -> {
          for (final String value : values) {
            headersBuilder.add(name, value);
          }
        });
    builder.headers(headersBuilder.build());
  }

  private static RequestBody buildRequestBody(final TransportRequest request) {
    final byte[] payload = request.body();
    final boolean hasBody = payload != null && payload.length > 0;
    final boolean permitsEmptyBody = permitsEmptyBody(request.method());
    if (!hasBody && permitsEmptyBody) {
      return null;
    }
    final byte[] bytes = hasBody ? payload : new byte[0];
    return RequestBody.create(bytes, (MediaType) null);
  }

  private static boolean permitsEmptyBody(final String method) {
    final String normalized = method == null ? "" : method.trim().toUpperCase(Locale.ROOT);
    return "GET".equals(normalized) || "HEAD".equals(normalized);
  }
}

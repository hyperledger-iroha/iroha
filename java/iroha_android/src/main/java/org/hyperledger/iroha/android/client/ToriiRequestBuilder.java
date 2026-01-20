package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.time.Duration;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.norito.SignedTransactionEncoder;
import org.hyperledger.iroha.android.tx.SignedTransaction;

/** Builds Torii HTTP requests for submitting signed transactions. */
final class ToriiRequestBuilder {
  private static final String SUBMIT_PATH = "/v1/pipeline/transactions";
  private static final String STATUS_PATH = "/v1/pipeline/transactions/status";

  private ToriiRequestBuilder() {}

  static TransportRequest buildSubmitRequest(
      final URI baseUri,
      final SignedTransaction transaction,
      final Duration timeout,
      final Map<String, String> extraHeaders) {
    Objects.requireNonNull(baseUri, "baseUri");
    Objects.requireNonNull(transaction, "transaction");
    final URI target = resolve(baseUri, SUBMIT_PATH);
    final byte[] norito;
    try {
      norito = SignedTransactionEncoder.encodeVersioned(transaction);
    } catch (final NoritoException ex) {
      throw new IllegalStateException("Failed to encode signed transaction", ex);
    }
    final TransportRequest.Builder builder =
        TransportRequest.builder()
            .setUri(target)
            .setMethod("POST")
            .addHeader("Content-Type", "application/x-norito")
            .addHeader("Accept", "application/x-norito, application/json")
            .setBody(norito);
    applyHeaders(builder, extraHeaders);
    applyTimeout(builder, timeout);
    return builder.build();
  }

  static TransportRequest buildStatusRequest(
      final URI baseUri,
      final String hashHex,
      final Duration timeout,
      final Map<String, String> extraHeaders) {
    Objects.requireNonNull(baseUri, "baseUri");
    final String normalizedHash =
        Objects.requireNonNull(hashHex, "hashHex").trim();
    if (normalizedHash.isEmpty()) {
      throw new IllegalArgumentException("hashHex must not be blank");
    }
    final URI target = resolve(baseUri, STATUS_PATH + "?hash=" + normalizedHash);
    final TransportRequest.Builder builder =
        TransportRequest.builder().setUri(target).setMethod("GET").addHeader("Accept", "application/json");
    applyHeaders(builder, extraHeaders);
    applyTimeout(builder, timeout);
    return builder.build();
  }

  private static URI resolve(final URI baseUri, final String path) {
    final String base = baseUri.toString();
    final String normalizedPath = path.startsWith("/") ? path.substring(1) : path;
    final String joined = base.endsWith("/")
        ? base + normalizedPath
        : base + "/" + normalizedPath;
    return URI.create(joined);
  }

  private static void applyHeaders(
      final TransportRequest.Builder builder, final Map<String, String> headers) {
    if (headers == null || headers.isEmpty()) {
      return;
    }
    for (Map.Entry<String, String> entry : headers.entrySet()) {
      if (entry.getKey() != null && entry.getValue() != null) {
        builder.addHeader(entry.getKey(), entry.getValue());
      }
    }
  }

  private static void applyTimeout(
      final TransportRequest.Builder builder, final Duration timeout) {
    if (timeout == null || timeout.isZero() || timeout.isNegative()) {
      return;
    }
    builder.setTimeout(timeout);
  }
}

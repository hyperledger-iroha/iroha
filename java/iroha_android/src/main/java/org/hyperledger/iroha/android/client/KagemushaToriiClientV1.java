// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.time.Duration;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.android.client.KagemushaToriiModelsV1.Readiness;
import org.hyperledger.iroha.android.client.KagemushaToriiModelsV1.UnverifiedOperationStatus;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.norito.SignedTransactionEncoder;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.sdk.offline.KagemushaRedemptionRequestV1;

/** Mirrored Java client for the sole canonical first-release KAGEMUSHA reserve API. */
public final class KagemushaToriiClientV1 {
  public static final String READINESS_PATH = "/v1/kagemusha/readiness";
  public static final String TOP_UP_PATH = "/v1/kagemusha/top-up";
  public static final String REDEEM_PATH = "/v1/kagemusha/redeem";
  public static final String OPERATION_PATH_PREFIX = "/v1/kagemusha/operations/";

  private final org.hyperledger.iroha.sdk.client.KagemushaToriiClientV1 delegate;

  private KagemushaToriiClientV1(final Builder builder) {
    final org.hyperledger.iroha.sdk.client.KagemushaToriiClientV1.Builder core =
        org.hyperledger.iroha.sdk.client.KagemushaToriiClientV1.builder()
            .baseUri(Objects.requireNonNull(builder.baseUri, "baseUri"))
            .timeout(builder.timeout);
    if (builder.executor != null) {
      core.executor(
          request ->
              builder
                  .executor
                  .execute(toJavaRequest(request))
                  .thenApply(KagemushaToriiClientV1::toKotlinResponse));
    }
    builder.defaultHeaders.forEach(core::addHeader);
    delegate = core.build();
  }

  public static Builder builder() {
    return new Builder();
  }

  public CompletableFuture<Readiness> getReadiness() {
    return delegate.getReadiness().thenApply(value -> new Readiness(
        value.kagemushaHandoffCapability,
        value.wireVersion,
        value.deviceLifecycleVersion,
        value.ready));
  }

  /**
   * Submits one canonical payer-signed KAGEMUSHA top-up transaction with its explicit nonzero
   * 32-byte operation identifier.
   */
  public CompletableFuture<UnverifiedOperationStatus> submitTopUp(
      final SignedTransaction transaction, final byte[] operationId) {
    final byte[] expected = requireNonzero32(operationId, "operationId");
    final byte[] versionedTransaction;
    final org.hyperledger.iroha.sdk.tx.SignedTransaction coreTransaction;
    try {
      versionedTransaction =
          SignedTransactionEncoder.encodeVersioned(
              Objects.requireNonNull(transaction, "transaction"));
      coreTransaction =
          org.hyperledger.iroha.sdk.tx.norito.SignedTransactionEncoder.decodeVersioned(
              versionedTransaction);
    } catch (final Exception error) {
      throw new IllegalArgumentException(
          "transaction must be one canonical version-1 signed transaction", error);
    }
    return delegate.submitTopUp(coreTransaction, expected)
        .thenApply(UnverifiedOperationStatus::new);
  }

  public CompletableFuture<UnverifiedOperationStatus> submitRedemption(
      final KagemushaRedemptionRequestV1 request) {
    return delegate.submitRedemption(Objects.requireNonNull(request, "request"))
        .thenApply(UnverifiedOperationStatus::new);
  }

  public CompletableFuture<UnverifiedOperationStatus> getOperation(final byte[] operationId) {
    return delegate.getOperation(Objects.requireNonNull(operationId, "operationId").clone())
        .thenApply(UnverifiedOperationStatus::new);
  }

  private static byte[] requireNonzero32(final byte[] value, final String fieldName) {
    Objects.requireNonNull(value, fieldName);
    if (value.length != 32) {
      throw new IllegalArgumentException(fieldName + " must contain exactly 32 bytes");
    }
    int aggregate = 0;
    for (final byte item : value) {
      aggregate |= item & 0xff;
    }
    if (aggregate == 0) {
      throw new IllegalArgumentException(fieldName + " must not be all zeroes");
    }
    return value.clone();
  }

  private static TransportRequest toJavaRequest(
      final org.hyperledger.iroha.sdk.client.transport.TransportRequest request) {
    final TransportRequest.Builder result = TransportRequest.builder()
        .setMethod(request.method)
        .setUri(request.uri)
        .setHeaders(request.getHeaders())
        .setBody(request.getBody())
        .setTimeout(request.timeout)
        .setMaximumResponseBytes(request.maximumResponseBytes);
    return result.build();
  }

  private static org.hyperledger.iroha.sdk.client.transport.TransportResponse toKotlinResponse(
      final TransportResponse response) {
    final org.hyperledger.iroha.sdk.client.transport.TransportResponse.Builder result =
        org.hyperledger.iroha.sdk.client.transport.TransportResponse.builder()
            .setStatusCode(response.statusCode())
            .setBody(response.body())
            .setMessage(response.message())
            .setHeaders(response.headers());
    if (response.finalUri() != null) {
      result.setNetworkProvenance(response.finalUri(), response.redirected());
    }
    return result.build();
  }

  /** Builder for {@link KagemushaToriiClientV1}. */
  public static final class Builder {
    private HttpTransportExecutor executor;
    private URI baseUri = URI.create("http://localhost:8080");
    private Duration timeout = Duration.ofSeconds(15);
    private final java.util.LinkedHashMap<String, String> defaultHeaders =
        new java.util.LinkedHashMap<>();

    public Builder executor(final HttpTransportExecutor executor) {
      this.executor = Objects.requireNonNull(executor, "executor");
      return this;
    }

    public Builder baseUri(final URI baseUri) {
      this.baseUri = Objects.requireNonNull(baseUri, "baseUri");
      return this;
    }

    public Builder timeout(final Duration timeout) {
      this.timeout = timeout;
      return this;
    }

    public Builder addHeader(final String name, final String value) {
      defaultHeaders.put(name, value);
      return this;
    }

    public KagemushaToriiClientV1 build() {
      return new KagemushaToriiClientV1(this);
    }
  }
}

package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import java.util.function.Function;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

/** Typed HTTP client for DA proof-policy, commitment, and pin-intent routes. */
public final class DaToriiClient {
  private static final String PROOF_POLICIES_PATH = "/v1/da/proof-policies";
  private static final String PROOF_POLICY_SNAPSHOT_PATH =
      "/v1/da/proof-policies/snapshot";
  private static final String COMMITMENTS_PATH = "/v1/da/commitments";
  private static final String COMMITMENTS_PROVE_PATH = "/v1/da/commitments/prove";
  private static final String COMMITMENTS_VERIFY_PATH = "/v1/da/commitments/verify";
  private static final String PIN_INTENTS_PATH = "/v1/da/pin-intents";
  private static final String PIN_INTENTS_PROVE_PATH = "/v1/da/pin-intents/prove";
  private static final String PIN_INTENTS_VERIFY_PATH = "/v1/da/pin-intents/verify";
  private static final int REQUEST_MAX_BYTES = 64 * 1024;
  private static final int RESPONSE_MAX_BYTES = 8 * 1024 * 1024;

  private final HttpTransportExecutor executor;
  private final URI baseUri;
  private final Duration timeout;
  private final Map<String, String> defaultHeaders;
  private final List<ClientObserver> observers;

  private DaToriiClient(final Builder builder) {
    this.executor =
        builder.executor == null
            ? PlatformHttpTransportExecutor.createDefault()
            : builder.executor;
    this.baseUri = Objects.requireNonNull(builder.baseUri, "baseUri");
    this.timeout = builder.timeout;
    this.defaultHeaders =
        Collections.unmodifiableMap(new LinkedHashMap<>(builder.defaultHeaders));
    this.observers =
        Collections.unmodifiableList(new ArrayList<>(builder.observers));
  }

  /** Creates a client builder. */
  public static Builder builder() {
    return new Builder();
  }

  /** Fetches the active proof-policy bundle. */
  public CompletableFuture<DaModels.ProofPolicyBundle> getProofPolicies() {
    return executeGet(
        PROOF_POLICIES_PATH,
        bytes ->
            DaJson.parsePolicyBundle(
                DaJson.parse(bytes, "DA proof-policy response"), "response"));
  }

  /** Fetches the deterministic proof-policy snapshot. */
  public CompletableFuture<DaModels.ProofPolicyBundle> getProofPolicySnapshot() {
    return executeGet(
        PROOF_POLICY_SNAPSHOT_PATH,
        bytes ->
            DaJson.parsePolicyBundle(
                DaJson.parse(bytes, "DA proof-policy response"), "response"));
  }

  /** Lists DA commitments matching {@code query}. */
  public CompletableFuture<DaModels.CommitmentListResponse> listCommitments(
      final DaModels.CommitmentQuery query) {
    final DaModels.CommitmentQuery actual =
        query == null ? new DaModels.CommitmentQuery() : query;
    return executePost(
        COMMITMENTS_PATH,
        actual.toJsonBytes(),
        bytes ->
            DaJson.parseCommitmentList(
                DaJson.parse(bytes, "DA commitment response"), "response"));
  }

  /** Lists all DA commitments. */
  public CompletableFuture<DaModels.CommitmentListResponse> listCommitments() {
    return listCommitments(null);
  }

  /** Produces the first DA commitment proof matching {@code query}, if any. */
  public CompletableFuture<DaModels.CommitmentProofResponse> proveCommitment(
      final DaModels.CommitmentQuery query) {
    final DaModels.CommitmentQuery actual =
        query == null ? new DaModels.CommitmentQuery() : query;
    return executePost(
        COMMITMENTS_PROVE_PATH,
        actual.toJsonBytes(),
        bytes ->
            DaJson.parseCommitmentProofResponse(
                DaJson.parse(bytes, "DA commitment proof response"), "response"));
  }

  /** Produces a proof for the first DA commitment, if any. */
  public CompletableFuture<DaModels.CommitmentProofResponse> proveCommitment() {
    return proveCommitment(null);
  }

  /** Verifies a DA commitment proof. */
  public CompletableFuture<DaModels.VerifyResponse> verifyCommitment(
      final DaModels.CommitmentProof proof) {
    return executePost(
        COMMITMENTS_VERIFY_PATH,
        Objects.requireNonNull(proof, "proof").toJsonBytes(),
        bytes ->
            DaJson.parseVerifyResponse(
                DaJson.parse(bytes, "DA commitment verification response"), "response"));
  }

  /** Lists DA pin intents matching {@code query}. */
  public CompletableFuture<List<DaModels.PinIntentWithLocation>> listPinIntents(
      final DaModels.PinIntentQuery query) {
    final DaModels.PinIntentQuery actual =
        query == null ? new DaModels.PinIntentQuery() : query;
    return executePost(
        PIN_INTENTS_PATH,
        actual.toJsonBytes(),
        bytes ->
            DaJson.parsePinIntentList(
                DaJson.parse(bytes, "DA pin-intent response"), "response"));
  }

  /** Lists all DA pin intents. */
  public CompletableFuture<List<DaModels.PinIntentWithLocation>> listPinIntents() {
    return listPinIntents(null);
  }

  /** Produces the first DA pin-intent proof matching {@code query}, if any. */
  public CompletableFuture<DaModels.PinIntentProof> provePinIntent(
      final DaModels.PinIntentQuery query) {
    final DaModels.PinIntentQuery actual =
        query == null ? new DaModels.PinIntentQuery() : query;
    return executePost(
        PIN_INTENTS_PROVE_PATH,
        actual.toJsonBytes(),
        bytes ->
            DaJson.parsePinIntentProof(
                DaJson.parse(bytes, "DA pin-intent proof response"), "response"));
  }

  /** Produces a proof for the first DA pin intent, if any. */
  public CompletableFuture<DaModels.PinIntentProof> provePinIntent() {
    return provePinIntent(null);
  }

  /** Verifies a DA pin-intent proof. */
  public CompletableFuture<DaModels.VerifyResponse> verifyPinIntent(
      final DaModels.PinIntentProof proof) {
    return executePost(
        PIN_INTENTS_VERIFY_PATH,
        Objects.requireNonNull(proof, "proof").toJsonBytes(),
        bytes ->
            DaJson.parseVerifyResponse(
                DaJson.parse(bytes, "DA pin-intent verification response"), "response"));
  }

  /** Returns the injected HTTP executor. */
  public HttpTransportExecutor executor() {
    return executor;
  }

  private <T> CompletableFuture<T> executeGet(
      final String path, final Function<byte[], T> parser) {
    return execute(buildRequest("GET", path, null), parser);
  }

  private <T> CompletableFuture<T> executePost(
      final String path, final byte[] body, final Function<byte[], T> parser) {
    if (body.length > REQUEST_MAX_BYTES) {
      throw new IllegalArgumentException(
          "DA request exceeds the " + REQUEST_MAX_BYTES + "-byte route limit");
    }
    return execute(buildRequest("POST", path, body), parser);
  }

  private TransportRequest buildRequest(
      final String method, final String path, final byte[] body) {
    final URI target = resolvePath(path);
    final Map<String, String> headers = new LinkedHashMap<>(defaultHeaders);
    ensureHeader(headers, "Accept", "application/json");
    if (body != null) {
      ensureHeader(headers, "Content-Type", "application/json");
    }
    TransportSecurity.requireHttpRequestAllowed(
        "DaToriiClient", baseUri, target, headers, body);
    final TransportRequest.Builder builder =
        TransportRequest.builder()
            .setUri(target)
            .setMethod(method)
            .setTimeout(timeout)
            .setMaximumResponseBytes((long) RESPONSE_MAX_BYTES);
    if (body != null) {
      builder.setBody(body);
    }
    for (final Map.Entry<String, String> header : headers.entrySet()) {
      builder.addHeader(header.getKey(), header.getValue());
    }
    return builder.build();
  }

  private <T> CompletableFuture<T> execute(
      final TransportRequest request, final Function<byte[], T> parser) {
    notifyRequest(request);
    return executor
        .execute(request)
        .handle(
            (response, throwable) -> {
              if (throwable != null) {
                final Throwable unwrapped =
                    throwable instanceof CompletionException
                        ? throwable.getCause()
                        : throwable;
                final DaToriiException error =
                    new DaToriiException(
                        "DA request failed", unwrapped == null ? throwable : unwrapped);
                notifyFailure(request, error);
                throw new CompletionException(error);
              }
              try {
                return parseResponse(request, response, parser);
              } catch (final RuntimeException error) {
                final DaToriiException wrapped =
                    error instanceof DaToriiException
                        ? (DaToriiException) error
                        : new DaToriiException("Failed to parse DA response", error);
                notifyFailure(request, wrapped);
                throw new CompletionException(wrapped);
              }
            });
  }

  private <T> T parseResponse(
      final TransportRequest request,
      final TransportResponse response,
      final Function<byte[], T> parser) {
    final byte[] body = response.body();
    final ClientResponse clientResponse =
        new ClientResponse(
            response.statusCode(),
            body,
            response.message(),
            null,
            HttpErrorMessageExtractor.extractRejectCode(
                response.headers(), "x-iroha-reject-code", body));
    if (response.statusCode() < 200 || response.statusCode() >= 300) {
      final String extracted = HttpErrorMessageExtractor.extractMessage(body);
      final String detail = extracted == null ? response.message() : extracted;
      final DaToriiException error =
          new DaToriiException(
              "DA request failed with HTTP " + response.statusCode() + ": " + detail);
      throw error;
    }
    if (body.length > RESPONSE_MAX_BYTES) {
      throw new DaToriiException(
          "DA response exceeds the " + RESPONSE_MAX_BYTES + "-byte client limit");
    }
    if (!hasJsonContentType(response.headers())) {
      throw new DaToriiException("DA response must use application/json");
    }
    final T parsed = parser.apply(body);
    notifyResponse(request, clientResponse);
    return parsed;
  }

  private URI resolvePath(final String path) {
    final String normalized = path.startsWith("/") ? path.substring(1) : path;
    final String base = baseUri.toString();
    return URI.create(base.endsWith("/") ? base + normalized : base + "/" + normalized);
  }

  private void notifyRequest(final TransportRequest request) {
    for (final ClientObserver observer : observers) {
      observer.onRequest(request);
    }
  }

  private void notifyResponse(
      final TransportRequest request, final ClientResponse response) {
    for (final ClientObserver observer : observers) {
      observer.onResponse(request, response);
    }
  }

  private void notifyFailure(
      final TransportRequest request, final Throwable error) {
    for (final ClientObserver observer : observers) {
      observer.onFailure(request, error);
    }
  }

  private static void ensureHeader(
      final Map<String, String> headers, final String name, final String value) {
    String existing = null;
    for (final String candidate : headers.keySet()) {
      if (candidate.equalsIgnoreCase(name)) {
        existing = candidate;
        break;
      }
    }
    headers.put(existing == null ? name : existing, value);
  }

  private static boolean hasJsonContentType(
      final Map<String, List<String>> headers) {
    for (final Map.Entry<String, List<String>> entry : headers.entrySet()) {
      if (!entry.getKey().equalsIgnoreCase("Content-Type")) {
        continue;
      }
      for (final String value : entry.getValue()) {
        final String mediaType = value.split(";", 2)[0].trim();
        if ("application/json".equalsIgnoreCase(mediaType)) {
          return true;
        }
      }
    }
    return false;
  }

  /** Builder for {@link DaToriiClient}. */
  public static final class Builder {
    private HttpTransportExecutor executor;
    private URI baseUri = URI.create("http://localhost:8080");
    private Duration timeout = Duration.ofSeconds(15);
    private final Map<String, String> defaultHeaders = new LinkedHashMap<>();
    private final List<ClientObserver> observers = new ArrayList<>();

    private Builder() {}

    public Builder setExecutor(final HttpTransportExecutor executor) {
      this.executor = Objects.requireNonNull(executor, "executor");
      return this;
    }

    public Builder setBaseUri(final URI baseUri) {
      this.baseUri = Objects.requireNonNull(baseUri, "baseUri");
      return this;
    }

    public Builder setTimeout(final Duration timeout) {
      if (timeout != null && timeout.isNegative()) {
        throw new IllegalArgumentException("timeout must be non-negative");
      }
      this.timeout = timeout;
      return this;
    }

    public Builder addHeader(final String name, final String value) {
      defaultHeaders.put(
          Objects.requireNonNull(name, "name"),
          Objects.requireNonNull(value, "value"));
      return this;
    }

    public Builder setDefaultHeaders(final Map<String, String> headers) {
      defaultHeaders.clear();
      if (headers != null) {
        defaultHeaders.putAll(headers);
      }
      return this;
    }

    public Builder addObserver(final ClientObserver observer) {
      if (observer != null) {
        observers.add(observer);
      }
      return this;
    }

    public Builder setObservers(final List<ClientObserver> observers) {
      this.observers.clear();
      if (observers != null) {
        for (final ClientObserver observer : observers) {
          addObserver(observer);
        }
      }
      return this;
    }

    public DaToriiClient build() {
      return new DaToriiClient(this);
    }
  }
}

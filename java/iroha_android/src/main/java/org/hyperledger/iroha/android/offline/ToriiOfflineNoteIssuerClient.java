package org.hyperledger.iroha.android.offline;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import java.util.function.BiConsumer;
import java.util.function.Function;
import java.util.function.LongSupplier;
import org.hyperledger.iroha.android.client.CanonicalRequestSigner;
import org.hyperledger.iroha.android.client.ClientObserver;
import org.hyperledger.iroha.android.client.ClientResponse;
import org.hyperledger.iroha.android.client.HttpTransportExecutor;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.client.PlatformHttpTransportExecutor;
import org.hyperledger.iroha.android.client.ToriiCanonicalRequestAuth;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

/** Torii-backed issuer client for Offline Note wallet loads. */
public final class ToriiOfflineNoteIssuerClient
    implements OfflineNoteIssuerClient, KagemushaTopUpClient {
  private static final String KEYS_REFILL_PATH = "/v1/offline/v2/keys/refill";
  private static final String KAGEMUSHA_TOPUP_PATH = "/v1/offline/v2/kagemusha/topup";
  private static final String HEADER_WITNESS = "X-Iroha-Witness";
  private static final String DEVICE_PROOF_PLATFORM_IOS = "ios";
  private static final String DEVICE_PROOF_PLATFORM_ANDROID = "android";
  public static final String RETIRED_OFFLINE_NOTE_ISSUE_MESSAGE =
      "Classic Offline Note issue transactions are retired; use Kagemusha online-to-offline top-up flows.";

  private final ToriiCanonicalRequestAuth canonicalAuth;
  private final OfflineNoteIssuerDeviceBindingProvider deviceBindingProvider;
  private final OfflineNoteIssuerDeviceProofProvider deviceProofProvider;
  private final HttpTransportExecutor executor;
  private final URI baseUri;
  private final Duration timeout;
  private final Map<String, String> defaultHeaders;
  private final List<ClientObserver> observers;
  private final LongSupplier clock;
  private final OfflineNoteIdGenerator nonceGenerator;
  private final boolean idempotencyKeysEnabled;
  private BiConsumer<String, Map<String, Object>> responseListener = (path, response) -> {};
  private final Map<String, PendingLoad> pendingLoads = new LinkedHashMap<>();
  private final Map<String, StoredLineageState> lineageStates = new LinkedHashMap<>();

  public ToriiOfflineNoteIssuerClient(
      final ToriiCanonicalRequestAuth canonicalAuth,
      final OfflineNoteIssuerDeviceBindingProvider deviceBindingProvider) {
    this(
        canonicalAuth,
        deviceBindingProvider,
        PlatformHttpTransportExecutor.createDefault(),
        URI.create("http://localhost:8080"),
        Duration.ofSeconds(15),
        Collections.emptyMap(),
        Collections.emptyList(),
        System::currentTimeMillis,
        new UuidOfflineNoteIdGenerator());
  }

  public ToriiOfflineNoteIssuerClient(
      final ToriiCanonicalRequestAuth canonicalAuth,
      final OfflineNoteIssuerDeviceBindingProvider deviceBindingProvider,
      final HttpTransportExecutor executor,
      final URI baseUri,
      final Duration timeout,
      final Map<String, String> defaultHeaders,
      final List<ClientObserver> observers,
      final LongSupplier clock,
      final OfflineNoteIdGenerator nonceGenerator) {
    this(
        canonicalAuth,
        deviceBindingProvider,
        null,
        executor,
        baseUri,
        timeout,
        defaultHeaders,
        observers,
        clock,
        nonceGenerator,
        false);
  }

  public ToriiOfflineNoteIssuerClient(
      final ToriiCanonicalRequestAuth canonicalAuth,
      final OfflineNoteIssuerDeviceBindingProvider deviceBindingProvider,
      final OfflineNoteIssuerDeviceProofProvider deviceProofProvider,
      final HttpTransportExecutor executor,
      final URI baseUri,
      final Duration timeout,
      final Map<String, String> defaultHeaders,
      final List<ClientObserver> observers,
      final LongSupplier clock,
      final OfflineNoteIdGenerator nonceGenerator) {
    this(
        canonicalAuth,
        deviceBindingProvider,
        deviceProofProvider,
        executor,
        baseUri,
        timeout,
        defaultHeaders,
        observers,
        clock,
        nonceGenerator,
        true);
  }

  private ToriiOfflineNoteIssuerClient(
      final ToriiCanonicalRequestAuth canonicalAuth,
      final OfflineNoteIssuerDeviceBindingProvider deviceBindingProvider,
      final OfflineNoteIssuerDeviceProofProvider deviceProofProvider,
      final HttpTransportExecutor executor,
      final URI baseUri,
      final Duration timeout,
      final Map<String, String> defaultHeaders,
      final List<ClientObserver> observers,
      final LongSupplier clock,
      final OfflineNoteIdGenerator nonceGenerator,
      final boolean idempotencyKeysEnabled) {
    this.canonicalAuth = Objects.requireNonNull(canonicalAuth, "canonicalAuth");
    this.deviceBindingProvider =
        Objects.requireNonNull(deviceBindingProvider, "deviceBindingProvider");
    this.deviceProofProvider = deviceProofProvider;
    this.executor = Objects.requireNonNull(executor, "executor");
    this.baseUri = Objects.requireNonNull(baseUri, "baseUri");
    this.timeout = timeout;
    this.defaultHeaders =
        Collections.unmodifiableMap(stripRetiredCanonicalBodyAuthHeaders(
            defaultHeaders == null ? Collections.emptyMap() : defaultHeaders));
    this.observers =
        Collections.unmodifiableList(
            new ArrayList<>(observers == null ? Collections.emptyList() : observers));
    this.clock = Objects.requireNonNull(clock, "clock");
    this.nonceGenerator = Objects.requireNonNull(nonceGenerator, "nonceGenerator");
    this.idempotencyKeysEnabled = idempotencyKeysEnabled;
  }

  public void rememberLineageState(
      final String chainId,
      final String accountId,
      final String assetDefinitionId,
      final OfflineNoteIssuerDeviceBinding binding,
      final Map<String, Object> lineageState,
      final OfflineNote.KeyCertificate keyCertificate) {
    Objects.requireNonNull(chainId, "chainId");
    final StoredLineageState stored = storedLineageState(lineageState, keyCertificate);
    synchronized (this) {
      lineageStates.put(lineageKey(accountId, assetDefinitionId, binding), stored);
    }
  }

  public void setResponseListener(final BiConsumer<String, Map<String, Object>> responseListener) {
    this.responseListener = responseListener == null ? (path, response) -> {} : responseListener;
  }

  @Override
  public CompletableFuture<OfflineNoteLoadContext> prepareLoad(
      final String chainId,
      final String accountId,
      final String assetDefinitionId,
      final String amount) {
    if (!canonicalAuth.accountId().equals(accountId)) {
      return failedFuture(
          new IllegalArgumentException("canonical auth accountId must match wallet accountId"));
    }
    final OfflineNoteIssuerDeviceBinding binding =
        deviceBindingProvider.currentDeviceBinding(chainId, accountId, assetDefinitionId);
    final String lineageKey = lineageKey(accountId, assetDefinitionId, binding);
    final StoredLineageState cached;
    synchronized (this) {
      final StoredLineageState candidate = lineageStates.get(lineageKey);
      cached = candidate == null || candidate.isExpired(clock.getAsLong()) ? null : candidate;
    }
    if (cached != null) {
      final String operationId = nonceGenerator.nextId("offline-load");
      final PendingLoad pending =
          new PendingLoad(
              operationId,
              lineageKey,
              cached.lineageId,
              cached.revision,
              cached.balance,
              cached.keyCertificate,
              cached.lineageState,
              binding);
      synchronized (this) {
        pendingLoads.put(operationId, pending);
      }
      return CompletableFuture.completedFuture(pending.context());
    }
    return refillKeys(chainId, accountId, assetDefinitionId, binding, lineageKey);
  }

  @Override
  public CompletableFuture<OfflineNoteIssueResponse> issueNote(
      final OfflineNoteIssueRequest request) {
    final PendingLoad pending;
    synchronized (this) {
      pending = pendingLoads.remove(request.loadContext().operationId());
    }
    if (pending == null) {
      return failedFuture(
          new OfflineToriiException(
              "Missing Offline Note load context for operation "
                  + request.loadContext().operationId()
                  + "."));
    }
    return failedFuture(new IllegalStateException(RETIRED_OFFLINE_NOTE_ISSUE_MESSAGE));
  }

  @Override
  public CompletableFuture<KagemushaTopUpResponse> submitKagemushaTopUp(
      final String chainId,
      final String accountId,
      final String assetDefinitionId,
      final byte[] topUpRequestArchive) {
    if (!canonicalAuth.accountId().equals(accountId)) {
      throw new IllegalArgumentException("canonical auth accountId must match top-up accountId");
    }
    requireExactNonEmptyText(chainId, "chainId");
    requireExactNonEmptyText(accountId, "accountId");
    requireExactNonEmptyText(assetDefinitionId, "assetDefinitionId");
    Objects.requireNonNull(topUpRequestArchive, "topUpRequestArchive");
    if (topUpRequestArchive.length == 0) {
      throw new IllegalArgumentException("topUpRequestArchive must not be empty");
    }
    final OfflineNoteIssuerDeviceBinding binding =
        deviceBindingProvider.currentDeviceBinding(chainId, accountId, assetDefinitionId);
    final String operationId = nonceGenerator.nextId("offline-kagemusha-topup");
    final Map<String, Object> body = new LinkedHashMap<>();
    body.put("account_id", accountId);
    body.put("operation_id", operationId);
    body.put("device_id", binding.deviceId());
    body.put("offline_public_key", binding.offlinePublicKey());
    body.put("asset_definition_id", assetDefinitionId);
    body.put("device_binding", binding.deviceBinding());
    body.put(
        "topup_request_norito_base64",
        Base64.getEncoder()
            .encodeToString(Arrays.copyOf(topUpRequestArchive, topUpRequestArchive.length)));
    return executePost(
        KAGEMUSHA_TOPUP_PATH,
        body,
        payload -> {
          final Map<String, Object> response =
              expectObject(parseJson(payload), "kagemusha top-up response");
          notifyIssuerResponse(KAGEMUSHA_TOPUP_PATH, response);
          return new KagemushaTopUpResponse(
              requiredString(response, "operation_id"),
              requiredString(response, "chain_tx_hash"),
              requiredString(response, "asset_definition_id"),
              requiredString(response, "amount"),
              requiredStringList(response, "topup_anchor_nullifiers"),
              requiredStringList(response, "output_commitments"),
              requiredString(response, "root_hint"));
        });
  }

  private CompletableFuture<OfflineNoteLoadContext> refillKeys(
      final String chainId,
      final String accountId,
      final String assetDefinitionId,
      final OfflineNoteIssuerDeviceBinding binding,
      final String lineageKey) {
    final String operationId = nonceGenerator.nextId("offline-key-refill");
    final StoredLineageState existing;
    synchronized (this) {
      existing = lineageStates.get(lineageKey);
    }
    final Map<String, Object> body = new LinkedHashMap<>();
    body.put("account_id", accountId);
    body.put("operation_id", operationId);
    body.put("device_id", binding.deviceId());
    body.put("offline_public_key", binding.offlinePublicKey());
    body.put("attestation_key_id", binding.attestationKeyId());
    body.put("asset_definition_id", assetDefinitionId);
    body.put("local_revision", existing == null ? 0L : existing.revision);
    final String localStateHash =
        existing == null ? "" : optionalString(existing.lineageState.get("server_state_hash"));
    body.put("local_state_hash", localStateHash == null ? "" : localStateHash);
    body.put("device_binding", binding.deviceBinding());
    addDeviceProof(body, chainId, accountId, assetDefinitionId, "setup", "", null);
    if (existing != null) {
      body.put("existing_lineage_id", existing.lineageId);
      body.put("lineage_state", OfflineNoteIssuerDeviceBinding.deepCopyObject(existing.lineageState));
    }
    return executePost(
        KEYS_REFILL_PATH,
        body,
        payload -> {
          final Map<String, Object> response = expectObject(parseJson(payload), "keys refill response");
          final Map<String, Object> lineageState =
              expectObject(requiredValue(response, "lineage_state"), "lineage_state");
          validateLineageState(lineageState);
          final OfflineNote.KeyCertificate keyCertificate =
              parseKeyCertificate(expectObject(requiredValue(response, "key_certificate"), "key_certificate"));
          notifyIssuerResponse(KEYS_REFILL_PATH, response);
          final PendingLoad pending =
              new PendingLoad(
                  requiredString(response, "operation_id"),
                  lineageKey,
                  requiredString(lineageState, "lineage_id"),
                  requiredLong(lineageState, "server_revision"),
                  requiredString(lineageState, "balance"),
                  keyCertificate,
                  lineageState,
                  binding);
          synchronized (this) {
            pendingLoads.put(pending.operationId, pending);
          }
          return pending.context();
        });
  }

  private <T> CompletableFuture<T> executePost(
      final String path, final Map<String, Object> bodyFields, final Function<byte[], T> parser) {
    final URI target = resolvePath(path);
    final byte[] signedBody = signedBody("POST", target, bodyFields);
    final TransportRequest request = buildPostRequest(path, target, signedBody);
    notifyRequest(request);
    final CompletableFuture<T> future = new CompletableFuture<>();
    executor.execute(request)
        .whenComplete(
            (response, throwable) -> {
              if (throwable != null) {
                final Throwable cause =
                    throwable instanceof CompletionException ? throwable.getCause() : throwable;
                final OfflineToriiException error =
                    new OfflineToriiException(
                        "Offline issuer request failed: " + summarizeCauseMessage(cause), cause);
                notifyFailure(request, error);
                future.completeExceptionally(error);
                return;
              }
              final String bodyPreview = responseBodyPreview(response.body());
              final ClientResponse clientResponse =
                  new ClientResponse(response.statusCode(), response.body(), response.message(), null, null);
              if (response.statusCode() < 200 || response.statusCode() >= 300) {
                final OfflineToriiException error =
                    new OfflineToriiException(
                        "Offline issuer request failed with HTTP "
                            + response.statusCode()
                            + " on "
                            + request.uri().getPath()
                            + (bodyPreview == null || bodyPreview.isEmpty()
                                ? ""
                                : ". body=" + bodyPreview),
                        response.statusCode(),
                        null,
                        bodyPreview);
                notifyFailure(request, error);
                future.completeExceptionally(error);
                return;
              }
              try {
                final T parsed = parser.apply(response.body());
                notifyResponse(request, clientResponse);
                future.complete(parsed);
              } catch (RuntimeException ex) {
                final OfflineToriiException error =
                    new OfflineToriiException(
                        "Failed to parse Offline Note issuer response for "
                            + request.uri().getPath()
                            + ".",
                        ex,
                        response.statusCode(),
                        null,
                        bodyPreview);
                notifyFailure(request, error);
                future.completeExceptionally(error);
              }
            });
    return future;
  }

  private byte[] signedBody(
      final String method, final URI target, final Map<String, Object> bodyFields) {
    final long timestampMs =
        canonicalAuth.timestampMs() == null ? clock.getAsLong() : canonicalAuth.timestampMs();
    final String nonce =
        canonicalAuth.nonce() == null ? nonceGenerator.nextId("offline-auth") : canonicalAuth.nonce();
    final Map<String, Object> signed =
        CanonicalRequestSigner.withBodySignature(
            method,
            target,
            bodyFields,
            canonicalAuth,
            timestampMs,
            nonce);
    return JsonEncoder.encode(signed).getBytes(StandardCharsets.UTF_8);
  }

  private TransportRequest buildPostRequest(final String path, final URI target, final byte[] body) {
    final Map<String, String> headers = new LinkedHashMap<>(defaultHeaders);
    ensureHeader(headers, "Content-Type", "application/json");
    ensureHeader(headers, "Accept", "application/json");
    if (idempotencyKeysEnabled && !containsHeader(headers, "Idempotency-Key")) {
      ensureHeader(headers, "Idempotency-Key", idempotencyKey(path, body));
    }
    final TransportRequest.Builder builder =
        TransportRequest.builder().setUri(target).setMethod("POST").setBody(body).setTimeout(timeout);
    headers.forEach(builder::addHeader);
    return builder.build();
  }

  private void addDeviceProof(
      final Map<String, Object> body,
      final String chainId,
      final String accountId,
      final String assetDefinitionId,
      final String operation,
      final String lineageId,
      final String amount) {
    if (deviceProofProvider == null) {
      return;
    }
    final Map<String, Object> proof =
        deviceProofProvider.currentDeviceProof(
            chainId, accountId, assetDefinitionId, operation, lineageId, amount);
    validateDeviceProof(proof);
    body.put("device_proof", OfflineNoteIssuerDeviceBinding.deepCopyObject(proof));
  }

  private static void validateDeviceProof(final Map<String, Object> proof) {
    if (proof == null) {
      throw new IllegalStateException("device_proof must be a JSON object");
    }
    rejectUnexpectedDeviceProofFields(proof);
    final String platform = requiredString(proof, "platform");
    if (!isSupportedDeviceProofPlatform(platform)) {
      throw new IllegalStateException("platform must be a supported first-release value");
    }
    requiredExactNonEmptyString(proof, "attestation_key_id");
    requireLowerHex32(requiredString(proof, "challenge_hash_hex"), "challenge_hash_hex");
    decodeExactBase64(requiredString(proof, "assertion_base64"), "assertion_base64");
    if (proof.containsKey("counter")) {
      final long counter = requiredDeviceProofCounter(proof.get("counter"));
      if (counter < 0L) {
        throw new IllegalStateException("counter must be non-negative");
      }
    }
  }

  private static void rejectUnexpectedDeviceProofFields(final Map<String, Object> proof) {
    for (final String field : proof.keySet()) {
      if (!isDeviceProofField(field)) {
        throw new IllegalStateException("device_proof." + field + " is not supported");
      }
    }
  }

  private static boolean isDeviceProofField(final String field) {
    return "platform".equals(field)
        || "attestation_key_id".equals(field)
        || "challenge_hash_hex".equals(field)
        || "assertion_base64".equals(field)
        || "counter".equals(field);
  }

  private static boolean isSupportedDeviceProofPlatform(final String platform) {
    return DEVICE_PROOF_PLATFORM_IOS.equals(platform) || DEVICE_PROOF_PLATFORM_ANDROID.equals(platform);
  }

  private URI resolvePath(final String path) {
    if (path.startsWith("http://") || path.startsWith("https://")) {
      return URI.create(path);
    }
    final String normalized = path.startsWith("/") ? path.substring(1) : path;
    final String base = baseUri.toString();
    return URI.create(base.endsWith("/") ? base + normalized : base + "/" + normalized);
  }

  private void notifyRequest(final TransportRequest request) {
    for (final ClientObserver observer : observers) {
      observer.onRequest(request);
    }
  }

  private void notifyResponse(final TransportRequest request, final ClientResponse response) {
    for (final ClientObserver observer : observers) {
      observer.onResponse(request, response);
    }
  }

  private void notifyFailure(final TransportRequest request, final Throwable error) {
    for (final ClientObserver observer : observers) {
      observer.onFailure(request, error);
    }
  }

  private void notifyIssuerResponse(final String path, final Map<String, Object> response) {
    responseListener.accept(path, OfflineNoteIssuerDeviceBinding.deepCopyObject(response));
  }

  private static OfflineNote.KeyCertificate parseKeyCertificate(final Map<String, Object> value) {
    return new OfflineNote.KeyCertificate(
        requiredKeyCertificateVersion(value),
        requiredString(value, "platform"),
        requiredString(value, "key_id"),
        requiredString(value, "device_id"),
        requiredString(value, "account_id"),
        decodeExactBase64(requiredString(value, "public_key"), "public_key"),
        requiredAssertionScheme(value),
        requiredAssertionKeyAlgorithm(value),
        decodeExactBase64(requiredString(value, "assertion_public_key"), "assertion_public_key"),
        optionalAssertionUsageCountLimit(value.get("assertion_usage_count_limit")),
        requiredBoolean(value, "one_use"),
        decodeExactBase64(requiredString(value, "issuer_signature_base64"), "issuer_signature_base64"));
  }

  private static StoredLineageState storedLineageState(
      final Map<String, Object> lineageState,
      final OfflineNote.KeyCertificate keyCertificate) {
    return storedLineageState(lineageState, keyCertificate, null);
  }

  private static StoredLineageState storedLineageState(
      final Map<String, Object> lineageState,
      final OfflineNote.KeyCertificate keyCertificate,
      final Long keyCertificateExpiresAtMs) {
    validateLineageState(lineageState);
    final Map<String, Object> authorization = optionalObject(lineageState.get("authorization"));
    return new StoredLineageState(
        requiredString(lineageState, "lineage_id"),
        requiredLong(lineageState, "server_revision"),
        requiredString(lineageState, "balance"),
        authorization == null ? null : optionalLong(authorization.get("expires_at_ms")),
        keyCertificateExpiresAtMs,
        keyCertificate,
        lineageState);
  }

  private static void validateLineageState(final Map<String, Object> lineageState) {
    requiredExactNonEmptyString(lineageState, "lineage_id");
    requiredExactNonEmptyString(lineageState, "account_id");
    requiredExactNonEmptyString(lineageState, "device_id");
    requiredExactNonEmptyString(lineageState, "offline_public_key");
    requiredExactNonEmptyString(lineageState, "asset_definition_id");
    requireNonNegativeAmountString(lineageState, "balance");
    requireNonNegativeAmountString(lineageState, "locked_balance");
    requireNonNegativeLong(requiredLong(lineageState, "server_revision"), "server_revision");
    requireLowerHex32(requiredString(lineageState, "server_state_hash"), "server_state_hash");
    requireNonNegativeLong(
        requiredLong(lineageState, "pending_local_revision"), "pending_local_revision");
    validateSpendAuthorization(
        expectObject(requiredValue(lineageState, "authorization"), "authorization"));
    requireCanonicalSignatureBase64(
        requiredString(lineageState, "issuer_signature_base64"), "issuer_signature_base64");
  }

  private static void validateSpendAuthorization(final Map<String, Object> authorization) {
    requiredExactNonEmptyString(authorization, "authorization_id");
    requiredExactNonEmptyString(authorization, "lineage_id");
    requiredExactNonEmptyString(authorization, "account_id");
    optionalExactNonEmptyString(authorization, "device_id");
    optionalExactNonEmptyString(authorization, "offline_public_key");
    requiredExactNonEmptyString(authorization, "verdict_id");
    requireNonNegativeAmountString(authorization, "max_balance");
    requireNonNegativeAmountString(authorization, "max_tx_value");
    final long issuedAtMs = requiredLong(authorization, "issued_at_ms");
    final long refreshAtMs = requiredLong(authorization, "refresh_at_ms");
    final long expiresAtMs = requiredLong(authorization, "expires_at_ms");
    requireNonNegativeLong(issuedAtMs, "issued_at_ms");
    if (refreshAtMs < issuedAtMs) {
      throw new IllegalStateException("refresh_at_ms must be at or after issued_at_ms");
    }
    if (expiresAtMs <= issuedAtMs) {
      throw new IllegalStateException("expires_at_ms must be after issued_at_ms");
    }
    if (authorization.containsKey("device_binding")) {
      validateLineageDeviceBinding(
          expectObject(requiredValue(authorization, "device_binding"), "authorization.device_binding"));
    }
    requireCanonicalSignatureBase64(
        requiredString(authorization, "issuer_signature_base64"), "issuer_signature_base64");
  }

  private static void validateLineageDeviceBinding(final Map<String, Object> deviceBinding) {
    if (deviceBinding.containsKey("platform")) {
      final String platform = requiredString(deviceBinding, "platform");
      if (!isSupportedDeviceProofPlatform(platform)) {
        throw new IllegalStateException("device_binding.platform must be a supported first-release value");
      }
    }
    requiredExactNonEmptyString(deviceBinding, "attestation_key_id");
    requiredExactNonEmptyString(deviceBinding, "device_id");
    requiredExactNonEmptyString(deviceBinding, "offline_public_key");
    optionalExactNonEmptyString(deviceBinding, "assertion_public_key");
    optionalExactNonEmptyString(deviceBinding, "ios_team_id");
    optionalExactNonEmptyString(deviceBinding, "ios_bundle_id");
    if (deviceBinding.containsKey("ios_environment")) {
      final String environment = requiredString(deviceBinding, "ios_environment");
      if (!"production".equals(environment) && !"development".equals(environment)) {
        throw new IllegalStateException("device_binding.ios_environment must be production or development");
      }
    }
  }

  private static Object parseJson(final byte[] payload) {
    return JsonParser.parse(new String(payload, StandardCharsets.UTF_8));
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> expectObject(final Object value, final String path) {
    if (!(value instanceof Map<?, ?> map)) {
      throw new IllegalStateException(path + " must be a JSON object");
    }
    final Map<String, Object> result = new LinkedHashMap<>();
    for (final Map.Entry<?, ?> entry : map.entrySet()) {
      if (!(entry.getKey() instanceof String key)) {
        throw new IllegalStateException(path + " keys must be strings");
      }
      result.put(key, normalizeJsonValue(entry.getValue()));
    }
    return result;
  }

  private static Map<String, Object> optionalObject(final Object value) {
    return value == null ? null : expectObject(value, "object");
  }

  private static Object requiredValue(final Map<String, Object> value, final String field) {
    if (!value.containsKey(field)) {
      throw new IllegalStateException(field + " is required");
    }
    return value.get(field);
  }

  private static String requiredString(final Map<String, Object> value, final String field) {
    final String string = optionalString(requiredValue(value, field));
    if (string == null) {
      throw new IllegalStateException(field + " must be a string");
    }
    return string;
  }

  private static List<String> requiredStringList(
      final Map<String, Object> value, final String field) {
    final Object item = requiredValue(value, field);
    if (!(item instanceof List<?> list)) {
      throw new IllegalStateException(field + " must be an array");
    }
    final List<String> result = new ArrayList<>(list.size());
    for (int index = 0; index < list.size(); index++) {
      final Object valueAtIndex = list.get(index);
      if (!(valueAtIndex instanceof String string)) {
        throw new IllegalStateException(field + "[" + index + "] must be a string");
      }
      if (!isExactNonEmptyText(string)) {
        throw new IllegalStateException(field + "[" + index + "] must be exact non-empty text");
      }
      result.add(string);
    }
    return result;
  }

  private static String requireExactNonEmptyText(final String value, final String field) {
    if (!isExactNonEmptyText(value)) {
      throw new IllegalArgumentException(field + " must be exact non-empty text");
    }
    return value;
  }

  private static boolean isExactNonEmptyText(final String value) {
    return value != null && !value.isEmpty() && value.equals(value.trim());
  }

  private static String requiredExactNonEmptyString(
      final Map<String, Object> value, final String field) {
    final String string = requiredString(value, field);
    if (string.isEmpty() || !string.equals(string.trim())) {
      throw new IllegalStateException(field + " must be an exact non-empty string");
    }
    return string;
  }

  private static void optionalExactNonEmptyString(
      final Map<String, Object> value, final String field) {
    if (value.containsKey(field)) {
      requiredExactNonEmptyString(value, field);
    }
  }

  private static void requireNonNegativeLong(final long value, final String field) {
    if (value < 0L) {
      throw new IllegalStateException(field + " must be non-negative");
    }
  }

  private static void requireNonNegativeAmountString(
      final Map<String, Object> value, final String field) {
    final String amount = requiredString(value, field);
    if (amount.isEmpty() || !amount.equals(amount.trim())) {
      throw new IllegalStateException(field + " must be an exact non-negative amount");
    }
    boolean seenDigit = false;
    boolean seenDot = false;
    for (int index = 0; index < amount.length(); index++) {
      final char ch = amount.charAt(index);
      if (index == 0 && ch == '+') {
        continue;
      }
      if (ch >= '0' && ch <= '9') {
        seenDigit = true;
        continue;
      }
      if (ch == '.' && !seenDot) {
        seenDot = true;
        continue;
      }
      throw new IllegalStateException(field + " must be a non-negative amount");
    }
    if (!seenDigit) {
      throw new IllegalStateException(field + " must be a non-negative amount");
    }
  }

  private static String optionalString(final Object value) {
    return value instanceof String string ? string : null;
  }

  private static boolean requiredBoolean(final Map<String, Object> value, final String field) {
    final Object item = requiredValue(value, field);
    if (!(item instanceof Boolean bool)) {
      throw new IllegalStateException(field + " must be a boolean");
    }
    return bool;
  }

  private static long requiredLong(final Map<String, Object> value, final String field) {
    final Long result = optionalLong(requiredValue(value, field));
    if (result == null) {
      throw new IllegalStateException(field + " must be an integer");
    }
    return result;
  }

  private static int requiredKeyCertificateVersion(final Map<String, Object> value) {
    final long version = requiredLong(value, "version");
    if (version != OfflineNote.KEY_CERTIFICATE_VERSION) {
      throw new IllegalStateException("version must be " + OfflineNote.KEY_CERTIFICATE_VERSION);
    }
    return OfflineNote.KEY_CERTIFICATE_VERSION;
  }

  private static Integer optionalAssertionUsageCountLimit(final Object value) {
    final Long limit = optionalLong(value);
    if (limit == null) {
      return null;
    }
    if (limit.longValue() != 1L) {
      throw new IllegalStateException("assertion_usage_count_limit must be exactly 1");
    }
    return Integer.valueOf(1);
  }

  private static String requiredAssertionScheme(final Map<String, Object> value) {
    final String scheme = requiredString(value, "assertion_scheme");
    final String expected = expectedAssertionScheme(requiredString(value, "platform"));
    if (!scheme.equals(expected)) {
      throw new IllegalStateException("assertion_scheme must be " + expected);
    }
    return scheme;
  }

  private static String requiredAssertionKeyAlgorithm(final Map<String, Object> value) {
    final String algorithm = requiredString(value, "assertion_key_algorithm");
    final String expected = expectedAssertionKeyAlgorithm(requiredString(value, "platform"));
    if (!algorithm.equals(expected)) {
      throw new IllegalStateException("assertion_key_algorithm must be " + expected);
    }
    return algorithm;
  }

  private static String expectedAssertionScheme(final String platform) {
    if (OfflineNoteV2.ANDROID_KEYMINT_PLATFORM.equals(platform)) {
      return OfflineNoteV2.ANDROID_KEYMINT_ASSERTION_SCHEME;
    }
    if (OfflineNoteV2.IOS_APP_ATTEST_PLATFORM.equals(platform)) {
      return OfflineNoteV2.IOS_APP_ATTEST_ASSERTION_SCHEME;
    }
    throw new IllegalStateException("platform must be a supported first-release value");
  }

  private static String expectedAssertionKeyAlgorithm(final String platform) {
    if (OfflineNoteV2.ANDROID_KEYMINT_PLATFORM.equals(platform)) {
      return OfflineNoteV2.ANDROID_KEYMINT_ASSERTION_KEY_ALGORITHM;
    }
    if (OfflineNoteV2.IOS_APP_ATTEST_PLATFORM.equals(platform)) {
      return OfflineNoteV2.IOS_APP_ATTEST_ASSERTION_KEY_ALGORITHM;
    }
    throw new IllegalStateException("platform must be a supported first-release value");
  }

  private static Long optionalLong(final Object value) {
    if (value == null) {
      return null;
    }
    if (value instanceof Long longValue) {
      return longValue;
    }
    if (value instanceof Integer intValue) {
      return intValue.longValue();
    }
    if (value instanceof Short shortValue) {
      return shortValue.longValue();
    }
    if (value instanceof Byte byteValue) {
      return byteValue.longValue();
    }
    if (value instanceof java.math.BigInteger bigInteger) {
      return bigInteger.longValueExact();
    }
    if (value instanceof Double doubleValue) {
      if (!Double.isFinite(doubleValue) || doubleValue % 1.0 != 0.0) {
        throw new IllegalStateException("value must be an integer");
      }
      return doubleValue.longValue();
    }
    if (value instanceof Float floatValue) {
      return optionalLong(floatValue.doubleValue());
    }
    throw new IllegalStateException("value must be an integer");
  }

  private static long requiredDeviceProofCounter(final Object value) {
    if (value instanceof Long longValue) {
      return longValue.longValue();
    }
    if (value instanceof Integer intValue) {
      return intValue.longValue();
    }
    if (value instanceof Short shortValue) {
      return shortValue.longValue();
    }
    if (value instanceof Byte byteValue) {
      return byteValue.longValue();
    }
    if (value instanceof java.math.BigInteger bigInteger) {
      try {
        return bigInteger.longValueExact();
      } catch (ArithmeticException ex) {
        throw new IllegalStateException("counter must be an integer", ex);
      }
    }
    throw new IllegalStateException("counter must be an integer");
  }

  private static byte[] decodeExactBase64(final String value, final String field) {
    try {
      if (value.isEmpty() || !value.equals(value.trim())) {
        throw new IllegalArgumentException(field + " must be canonical base64");
      }
      final byte[] decoded = Base64.getDecoder().decode(value);
      if (decoded.length == 0 || !Base64.getEncoder().encodeToString(decoded).equals(value)) {
        throw new IllegalArgumentException(field + " must be canonical base64");
      }
      return decoded;
    } catch (IllegalArgumentException ex) {
      throw new IllegalStateException(field + " must be canonical base64", ex);
    }
  }

  private static void requireCanonicalSignatureBase64(final String value, final String field) {
    final byte[] signature = decodeExactBase64(value, field);
    if (signature.length != 64) {
      throw new IllegalStateException(field + " must be 64 bytes");
    }
  }

  private static void requireLowerHex32(final String value, final String field) {
    if (value.length() != 64) {
      throw new IllegalStateException(field + " must be 32-byte lowercase hex");
    }
    for (int index = 0; index < value.length(); index++) {
      final char ch = value.charAt(index);
      if (!((ch >= '0' && ch <= '9') || (ch >= 'a' && ch <= 'f'))) {
        throw new IllegalStateException(field + " must be 32-byte lowercase hex");
      }
    }
  }

  private static byte[] hexToBytes(final String value, final String field) {
    if (value.length() != 64) {
      throw new IllegalStateException(field + " must be 64 hex characters");
    }
    final byte[] out = new byte[32];
    for (int i = 0; i < out.length; i++) {
      final int hi = Character.digit(value.charAt(i * 2), 16);
      final int lo = Character.digit(value.charAt(i * 2 + 1), 16);
      if (hi < 0 || lo < 0) {
        throw new IllegalStateException(field + " must be hex");
      }
      out[i] = (byte) ((hi << 4) | lo);
    }
    return out;
  }

  private static void ensureHeader(
      final Map<String, String> headers, final String name, final String value) {
    String existing = null;
    for (final String key : headers.keySet()) {
      if (key.equalsIgnoreCase(name)) {
        existing = key;
        break;
      }
    }
    headers.put(existing == null ? name : existing, value);
  }

  private static boolean containsHeader(final Map<String, String> headers, final String name) {
    for (final String key : headers.keySet()) {
      if (key.equalsIgnoreCase(name)) {
        return true;
      }
    }
    return false;
  }

  private static LinkedHashMap<String, String> stripRetiredCanonicalBodyAuthHeaders(
      final Map<String, String> headers) {
    final LinkedHashMap<String, String> filtered = new LinkedHashMap<>();
    for (final Map.Entry<String, String> entry : headers.entrySet()) {
      if (!isRetiredCanonicalBodyAuthHeader(entry.getKey())) {
        filtered.put(entry.getKey(), entry.getValue());
      }
    }
    return filtered;
  }

  private static boolean isRetiredCanonicalBodyAuthHeader(final String name) {
    return CanonicalRequestSigner.HEADER_ACCOUNT.equalsIgnoreCase(name)
        || CanonicalRequestSigner.HEADER_SIGNATURE.equalsIgnoreCase(name)
        || CanonicalRequestSigner.HEADER_TIMESTAMP_MS.equalsIgnoreCase(name)
        || CanonicalRequestSigner.HEADER_NONCE.equalsIgnoreCase(name)
        || HEADER_WITNESS.equalsIgnoreCase(name);
  }

  private static String idempotencyKey(final String path, final byte[] body) {
    final String action =
        path.contains("/keys/refill")
            ? "keys.refill"
            : path.contains("/kagemusha/topup")
                ? "kagemusha.topup"
                : path.contains("/notes/issue") ? "notes.issue" : "mutation";
    return "offline-cash." + action + "." + sha256Hex(body);
  }

  private static String sha256Hex(final byte[] bytes) {
    try {
      final byte[] digest = MessageDigest.getInstance("SHA-256").digest(bytes);
      final StringBuilder out = new StringBuilder(digest.length * 2);
      for (final byte item : digest) {
        out.append(String.format("%02x", item & 0xff));
      }
      return out.toString();
    } catch (Exception ex) {
      throw new IllegalStateException("sha256 unavailable", ex);
    }
  }

  private static String lineageKey(
      final String accountId,
      final String assetDefinitionId,
      final OfflineNoteIssuerDeviceBinding binding) {
    return accountId + "\n" + assetDefinitionId + "\n" + binding.deviceId() + "\n"
        + binding.offlinePublicKey();
  }

  private static String summarizeCauseMessage(final Throwable cause) {
    if (cause == null) {
      return "unknown transport error";
    }
    final String detail = cause.getMessage();
    return detail == null || detail.trim().isEmpty() ? cause.getClass().getSimpleName() : detail;
  }

  private static String responseBodyPreview(final byte[] body) {
    if (body == null || body.length == 0) {
      return null;
    }
    final String text = new String(body, StandardCharsets.UTF_8).trim();
    if (text.isEmpty()) {
      return null;
    }
    return text.length() > 512 ? text.substring(0, 512) : text;
  }

  private static <T> CompletableFuture<T> failedFuture(final Throwable error) {
    final CompletableFuture<T> future = new CompletableFuture<>();
    future.completeExceptionally(error);
    return future;
  }

  @SuppressWarnings("unchecked")
  private static Object normalizeJsonValue(final Object value) {
    if (value == null
        || value instanceof String
        || value instanceof Number
        || value instanceof Boolean) {
      return value;
    }
    if (value instanceof Map<?, ?> map) {
      final Map<String, Object> object = new LinkedHashMap<>();
      for (final Map.Entry<?, ?> entry : map.entrySet()) {
        if (!(entry.getKey() instanceof String key)) {
          throw new IllegalStateException("JSON object keys must be strings");
        }
        object.put(key, normalizeJsonValue(entry.getValue()));
      }
      return object;
    }
    if (value instanceof List<?> list) {
      final List<Object> copy = new ArrayList<>(list.size());
      for (final Object item : list) {
        copy.add(normalizeJsonValue(item));
      }
      return copy;
    }
    throw new IllegalStateException("Unsupported JSON value: " + value.getClass());
  }

  private static final class PendingLoad {
    private final String operationId;
    private final String lineageKey;
    private final String lineageId;
    private final long preIssueRevision;
    private final String localBalance;
    private final OfflineNote.KeyCertificate keyCertificate;
    private final Map<String, Object> lineageState;
    private final OfflineNoteIssuerDeviceBinding deviceBinding;

    private PendingLoad(
        final String operationId,
        final String lineageKey,
        final String lineageId,
        final long preIssueRevision,
        final String localBalance,
        final OfflineNote.KeyCertificate keyCertificate,
        final Map<String, Object> lineageState,
        final OfflineNoteIssuerDeviceBinding deviceBinding) {
      this.operationId = operationId;
      this.lineageKey = lineageKey;
      this.lineageId = lineageId;
      this.preIssueRevision = preIssueRevision;
      this.localBalance = localBalance;
      this.keyCertificate = keyCertificate;
      this.lineageState = OfflineNoteIssuerDeviceBinding.deepCopyObject(lineageState);
      this.deviceBinding = deviceBinding;
    }

    private OfflineNoteLoadContext context() {
      return new OfflineNoteLoadContext(
          operationId, lineageId, preIssueRevision + 1, keyCertificate);
    }
  }

  private static final class StoredLineageState {
    private final String lineageId;
    private final long revision;
    private final String balance;
    private final Long authorizationExpiresAtMs;
    private final Long keyCertificateExpiresAtMs;
    private final OfflineNote.KeyCertificate keyCertificate;
    private final Map<String, Object> lineageState;

    private StoredLineageState(
        final String lineageId,
        final long revision,
        final String balance,
        final Long authorizationExpiresAtMs,
        final Long keyCertificateExpiresAtMs,
        final OfflineNote.KeyCertificate keyCertificate,
        final Map<String, Object> lineageState) {
      this.lineageId = lineageId;
      this.revision = revision;
      this.balance = balance;
      this.authorizationExpiresAtMs = authorizationExpiresAtMs;
      this.keyCertificateExpiresAtMs = keyCertificateExpiresAtMs;
      this.keyCertificate = keyCertificate;
      this.lineageState = OfflineNoteIssuerDeviceBinding.deepCopyObject(lineageState);
    }

    private boolean isExpired(final long nowMs) {
      return authorizationExpiresAtMs != null && authorizationExpiresAtMs <= nowMs
          || keyCertificateExpiresAtMs != null && keyCertificateExpiresAtMs <= nowMs;
    }
  }
}

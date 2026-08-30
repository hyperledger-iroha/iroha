package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.ByteBuffer;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import java.util.function.BiFunction;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

/** Exact-route V1 client for prepared-leg, audit, coordination, and redacted query workflows. */
public final class AtomicPrivateSettlementToriiClientV1 {
  private static final String JSON_MEDIA_TYPE = "application/json";
  private static final int RESPONSE_SMALL_MAX_BYTES = 1024 * 1024;
  private static final int RESPONSE_PUBLIC_BUNDLE_MAX_BYTES = 8 * 1024 * 1024;
  private static final int RESPONSE_RESTRICTED_MAX_BYTES = 32 * 1024 * 1024;

  private static final Set<String> FORBIDDEN_DEFAULT_HEADERS =
      Set.of(
          "authorization",
          "x-api-token",
          "x-iroha-witness",
          "x-iroha-account",
          "x-iroha-signature",
          "x-iroha-timestamp-ms",
          "x-iroha-nonce",
          "x-iroha-operator-public-key",
          "x-iroha-operator-timestamp-ms",
          "x-iroha-operator-nonce",
          "x-iroha-operator-signature",
          "accept",
          "accept-encoding",
          "content-type",
          "content-length",
          "cache-control",
          "pragma",
          "host");

  private final HttpTransportExecutor executor;
  private final URI baseUri;
  private final LocalSigningContext localSigningContext;
  private final Duration timeout;
  private final Map<String, String> defaultHeaders;

  private AtomicPrivateSettlementToriiClientV1(final Builder builder) {
    executor =
        builder.executor == null ? PlatformHttpTransportExecutor.createDefault() : builder.executor;
    baseUri = requireBaseUri(builder.baseUri);
    if (builder.localSigningContext == null) {
      throw new IllegalStateException(
          "localSigningContext must be configured before building a settlement client");
    }
    localSigningContext = builder.localSigningContext;
    timeout = builder.timeout;
    defaultHeaders =
        Collections.unmodifiableMap(new LinkedHashMap<>(builder.defaultHeaders));
    requireSafeDefaultHeaders(defaultHeaders);
  }

  /** Create a settlement client builder. */
  public static Builder builder() {
    return new Builder();
  }

  /** Request one durable availability share with exact sponsor authentication. */
  public CompletableFuture<AtomicPrivateSettlementJsonResponseV1> requestAvailabilityShare(
      final AtomicPrivateSettlementPreparedRequestV1 request,
      final ToriiCanonicalRequestAuth sponsorAuth) {
    return executeSponsorMutation(
        request, AtomicPrivateSettlementOperationV1.AVAILABILITY_SHARE, sponsorAuth);
  }

  /** Request one independently verified Prepare vote. */
  public CompletableFuture<AtomicPrivateSettlementJsonResponseV1> requestPrepareVote(
      final AtomicPrivateSettlementPreparedRequestV1 request,
      final ToriiCanonicalRequestAuth sponsorAuth) {
    return executeSponsorMutation(
        request, AtomicPrivateSettlementOperationV1.PREPARE_VOTE, sponsorAuth);
  }

  /** Request one complete-barrier Commit vote. */
  public CompletableFuture<AtomicPrivateSettlementJsonResponseV1> requestCommitVote(
      final AtomicPrivateSettlementPreparedRequestV1 request,
      final ToriiCanonicalRequestAuth sponsorAuth) {
    return executeSponsorMutation(
        request, AtomicPrivateSettlementOperationV1.COMMIT_VOTE, sponsorAuth);
  }

  /** Persist one aggregate Prepare or Commit certificate. */
  public CompletableFuture<AtomicPrivateSettlementJsonResponseV1> persistPhaseCertificate(
      final AtomicPrivateSettlementPreparedRequestV1 request,
      final ToriiCanonicalRequestAuth sponsorAuth) {
    return executeSponsorMutation(
        request, AtomicPrivateSettlementOperationV1.PHASE_CERTIFICATE, sponsorAuth);
  }

  /** Promote one availability-certified encrypted leg. */
  public CompletableFuture<AtomicPrivateSettlementJsonResponseV1> uploadLeg(
      final AtomicPrivateSettlementPreparedRequestV1 request,
      final ToriiCanonicalRequestAuth sponsorAuth) {
    return executeSponsorMutation(
        request, AtomicPrivateSettlementOperationV1.LEG_UPLOAD, sponsorAuth);
  }

  /** Submit one sponsor-signed exact global carrier. */
  public CompletableFuture<AtomicPrivateSettlementJsonResponseV1> submitBundle(
      final AtomicPrivateSettlementPreparedRequestV1 request,
      final ToriiCanonicalRequestAuth sponsorAuth) {
    return executeSponsorMutation(
        request, AtomicPrivateSettlementOperationV1.BUNDLE_SUBMIT, sponsorAuth);
  }

  /** Submit one purpose-separated approval as the exact governed auditor identity. */
  public CompletableFuture<AtomicPrivateSettlementJsonResponseV1> submitAuditApproval(
      final AtomicPrivateSettlementIdentifierV1 payloadDigest,
      final AtomicPrivateSettlementPreparedRequestV1 request,
      final OperatorSigningContext auditorSigningContext) {
    Objects.requireNonNull(payloadDigest, "payloadDigest");
    Objects.requireNonNull(auditorSigningContext, "auditorSigningContext");
    requireOperation(request, AtomicPrivateSettlementOperationV1.AUDIT_APPROVAL);
    final String path =
        request
            .operation()
            .path()
            .replace("{payload_digest}", payloadDigest.pathComponent());
    final byte[] body = request.bytes();
    return executeMutation(
        path,
        body,
        (target, signedBody) ->
            OperatorRequestSigner.buildHeaders(
                auditorSigningContext, "POST", target, signedBody),
        payloadDigest,
        "payload_digest");
  }

  /** Read one sponsor-authenticated redacted local leg lifecycle. */
  public CompletableFuture<AtomicPrivateSettlementJsonResponseV1> getLegStatus(
      final AtomicPrivateSettlementIdentifierV1 payloadDigest,
      final ToriiCanonicalRequestAuth sponsorAuth) {
    Objects.requireNonNull(payloadDigest, "payloadDigest");
    return executeGet(
        "/v1/nexus/private-settlements/legs/"
            + payloadDigest.pathComponent()
            + "/status",
        RESPONSE_SMALL_MAX_BYTES,
        (target, body) -> sponsorHeaders("GET", target, body, sponsorAuth),
        payloadDigest,
        "payload_digest");
  }

  /** Recover the persisted Prepare and Commit certificates for one local leg. */
  public CompletableFuture<AtomicPrivateSettlementJsonResponseV1> getPhaseCertificates(
      final AtomicPrivateSettlementIdentifierV1 payloadDigest,
      final ToriiCanonicalRequestAuth sponsorAuth) {
    Objects.requireNonNull(payloadDigest, "payloadDigest");
    return executeGet(
        "/v1/nexus/private-settlements/legs/"
            + payloadDigest.pathComponent()
            + "/phase-certificates",
        RESPONSE_SMALL_MAX_BYTES,
        (target, body) -> sponsorHeaders("GET", target, body, sponsorAuth),
        payloadDigest,
        "payload_digest");
  }

  /** Fetch proof and opaque delta material as one exact participant validator. */
  public CompletableFuture<AtomicPrivateSettlementJsonResponseV1> getCommitteeProof(
      final AtomicPrivateSettlementIdentifierV1 payloadDigest,
      final OperatorSigningContext validatorSigningContext) {
    Objects.requireNonNull(payloadDigest, "payloadDigest");
    Objects.requireNonNull(validatorSigningContext, "validatorSigningContext");
    return executeGet(
        "/v1/nexus/private-settlements/legs/"
            + payloadDigest.pathComponent()
            + "/committee-proof",
        RESPONSE_RESTRICTED_MAX_BYTES,
        (target, body) ->
            OperatorRequestSigner.buildHeaders(
                validatorSigningContext, "GET", target, body),
        null,
        null);
  }

  /** Fetch one padded encrypted capsule as one exact governed local auditor. */
  public CompletableFuture<AtomicPrivateSettlementJsonResponseV1> getAuditorCapsule(
      final AtomicPrivateSettlementIdentifierV1 payloadDigest,
      final OperatorSigningContext auditorSigningContext) {
    Objects.requireNonNull(payloadDigest, "payloadDigest");
    Objects.requireNonNull(auditorSigningContext, "auditorSigningContext");
    return executeGet(
        "/v1/nexus/private-settlements/legs/"
            + payloadDigest.pathComponent()
            + "/audit-capsule",
        RESPONSE_RESTRICTED_MAX_BYTES,
        (target, body) ->
            OperatorRequestSigner.buildHeaders(
                auditorSigningContext, "GET", target, body),
        null,
        null);
  }

  /** Read the public allowlisted lifecycle for one bundle. */
  public CompletableFuture<AtomicPrivateSettlementJsonResponseV1> getBundleStatus(
      final AtomicPrivateSettlementIdentifierV1 bundleId) {
    Objects.requireNonNull(bundleId, "bundleId");
    return executeGet(
        "/v1/nexus/private-settlements/bundles/" + bundleId.pathComponent(),
        RESPONSE_PUBLIC_BUNDLE_MAX_BYTES,
        null,
        bundleId,
        null);
  }

  /** Read the public terminal receipt or pending marker for one bundle. */
  public CompletableFuture<AtomicPrivateSettlementJsonResponseV1> getBundleReceipt(
      final AtomicPrivateSettlementIdentifierV1 bundleId) {
    Objects.requireNonNull(bundleId, "bundleId");
    return executeGet(
        "/v1/nexus/private-settlements/bundles/"
            + bundleId.pathComponent()
            + "/receipt",
        RESPONSE_RESTRICTED_MAX_BYTES,
        null,
        bundleId,
        null);
  }

  private CompletableFuture<AtomicPrivateSettlementJsonResponseV1> executeSponsorMutation(
      final AtomicPrivateSettlementPreparedRequestV1 request,
      final AtomicPrivateSettlementOperationV1 expectedOperation,
      final ToriiCanonicalRequestAuth sponsorAuth) {
    requireOperation(request, expectedOperation);
    Objects.requireNonNull(sponsorAuth, "sponsorAuth");
    final byte[] body = request.bytes();
    return executeMutation(
        expectedOperation.path(),
        body,
        (target, signedBody) -> sponsorHeaders("POST", target, signedBody, sponsorAuth),
        null,
        null);
  }

  private CompletableFuture<AtomicPrivateSettlementJsonResponseV1> executeMutation(
      final String path,
      final byte[] body,
      final BiFunction<URI, byte[], Map<String, String>> identityHeaders,
      final AtomicPrivateSettlementIdentifierV1 expectedIdentifier,
      final String expectedIdentifierField) {
    final URI target = resolvePath(path);
    final Map<String, String> headers = requestHeaders(true);
    headers.putAll(identityHeaders.apply(target, body));
    final TransportRequest request =
        buildRequest("POST", target, body, headers, RESPONSE_RESTRICTED_MAX_BYTES);
    return execute(
        request,
        path,
        RESPONSE_RESTRICTED_MAX_BYTES,
        expectedIdentifier,
        expectedIdentifierField);
  }

  private CompletableFuture<AtomicPrivateSettlementJsonResponseV1> executeGet(
      final String path,
      final int maximumResponseBytes,
      final BiFunction<URI, byte[], Map<String, String>> identityHeaders,
      final AtomicPrivateSettlementIdentifierV1 expectedIdentifier,
      final String expectedIdentifierField) {
    final URI target = resolvePath(path);
    final byte[] body = new byte[0];
    final Map<String, String> headers = requestHeaders(false);
    if (identityHeaders != null) {
      headers.putAll(identityHeaders.apply(target, body));
    }
    final TransportRequest request =
        buildRequest("GET", target, body, headers, maximumResponseBytes);
    return execute(
        request,
        path,
        maximumResponseBytes,
        expectedIdentifier,
        expectedIdentifierField);
  }

  private CompletableFuture<AtomicPrivateSettlementJsonResponseV1> execute(
      final TransportRequest request,
      final String route,
      final int maximumResponseBytes,
      final AtomicPrivateSettlementIdentifierV1 expectedIdentifier,
      final String expectedIdentifierField) {
    final CompletableFuture<TransportResponse> execution;
    try {
      execution = executor.execute(request);
    } catch (final RuntimeException error) {
      return CompletableFuture.failedFuture(
          new AtomicPrivateSettlementToriiExceptionV1(
              "atomic private settlement request failed", error));
    }
    return execution.handle(
        (response, throwable) -> {
          if (throwable != null) {
            final Throwable cause =
                throwable instanceof CompletionException && throwable.getCause() != null
                    ? throwable.getCause()
                    : throwable;
            throw new CompletionException(
                new AtomicPrivateSettlementToriiExceptionV1(
                    "atomic private settlement request failed", cause));
          }
          try {
            return validateResponse(
                request,
                Objects.requireNonNull(response, "response"),
                route,
                maximumResponseBytes,
                expectedIdentifier,
                expectedIdentifierField);
          } catch (final RuntimeException error) {
            final AtomicPrivateSettlementToriiExceptionV1 wrapped =
                error instanceof AtomicPrivateSettlementToriiExceptionV1 exact
                    ? exact
                    : new AtomicPrivateSettlementToriiExceptionV1(
                        "atomic private settlement response is invalid", error);
            throw new CompletionException(wrapped);
          }
        });
  }

  private AtomicPrivateSettlementJsonResponseV1 validateResponse(
      final TransportRequest request,
      final TransportResponse response,
      final String route,
      final int maximumResponseBytes,
      final AtomicPrivateSettlementIdentifierV1 expectedIdentifier,
      final String expectedIdentifierField) {
    final URI finalUri = response.finalUri();
    if (response.redirected()
        || finalUri == null
        || !request.uri().toASCIIString().equals(finalUri.toASCIIString())) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "atomic private settlement response must come from the exact request URL without redirects");
    }
    if (response.statusCode() < 200 || response.statusCode() >= 300) {
      final String rejectCode =
          HttpErrorMessageExtractor.extractRejectCode(
              response.headers(), "x-iroha-reject-code");
      final String suffix =
          rejectCode == null || rejectCode.isBlank() ? "" : "; reject_code=" + rejectCode;
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "atomic private settlement request failed with HTTP "
              + response.statusCode()
              + suffix);
    }
    final byte[] body = response.body();
    if (body.length == 0 || body.length > maximumResponseBytes) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "atomic private settlement response is empty or exceeds its route limit");
    }
    requireExactJsonContentType(response.headers());
    final Map<String, Object> parsed = parseExactJsonObject(body);
    validateRouteShape(route, parsed, expectedIdentifier, expectedIdentifierField);
    final byte[] canonical = JsonEncoder.encode(parsed).getBytes(StandardCharsets.UTF_8);
    return new AtomicPrivateSettlementJsonResponseV1(route, canonical);
  }

  private static void validateRouteShape(
      final String route,
      final Map<String, Object> parsed,
      final AtomicPrivateSettlementIdentifierV1 expectedIdentifier,
      final String expectedIdentifierField) {
    if (!parsed.keySet().equals(responseFields(route))) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "atomic private settlement response has unexpected public fields");
    }
    if (expectedIdentifier != null
        && expectedIdentifierField != null
        && !expectedIdentifier.jsonLiteral().equals(parsed.get(expectedIdentifierField))) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "atomic private settlement response identifier is substituted");
    }
    if (route.endsWith("/phase-certificates")) {
      final Object prepare = parsed.get("prepare_certificate");
      final Object commit = parsed.get("commit_certificate");
      if ((prepare != null && !(prepare instanceof Map<?, ?>))
          || (commit != null && !(commit instanceof Map<?, ?>))) {
        throw new AtomicPrivateSettlementToriiExceptionV1(
            "settlement phase certificates must be null or opaque objects");
      }
    }
    if (route.endsWith("/receipt")) {
      validateReceiptIdentity(parsed, Objects.requireNonNull(expectedIdentifier));
    } else if (route.contains("/bundles/") && !route.endsWith("/receipt")) {
      validateBundleStatusIdentity(parsed, Objects.requireNonNull(expectedIdentifier));
    }
  }

  @SuppressWarnings("unchecked")
  private static void validateBundleStatusIdentity(
      final Map<String, Object> parsed,
      final AtomicPrivateSettlementIdentifierV1 expectedIdentifier) {
    final Object manifest = parsed.get("manifest");
    if (manifest == null) {
      return;
    }
    if (!(manifest instanceof Map<?, ?> manifestFields)
        || !expectedIdentifier.jsonLiteral().equals(manifestFields.get("bundle_id"))) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "atomic private settlement bundle status is substituted");
    }
  }

  private static void validateReceiptIdentity(
      final Map<String, Object> parsed,
      final AtomicPrivateSettlementIdentifierV1 expectedIdentifier) {
    final Object status = parsed.get("status");
    if (!(status instanceof String statusText)
        || !Set.of("pending", "finalized", "aborted").contains(statusText)
        || !(parsed.get("value") instanceof Map<?, ?> value)
        || !expectedIdentifier.jsonLiteral().equals(value.get("bundle_id"))) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "atomic private settlement receipt is substituted");
    }
  }

  private Map<String, String> sponsorHeaders(
      final String method,
      final URI target,
      final byte[] body,
      final ToriiCanonicalRequestAuth sponsorAuth) {
    Objects.requireNonNull(sponsorAuth, "sponsorAuth");
    final Long timestampMs = sponsorAuth.timestampMs();
    final String nonce = sponsorAuth.nonce();
    if ((timestampMs == null) != (nonce == null)) {
      throw new IllegalArgumentException("timestampMs and nonce must be provided together");
    }
    if (timestampMs == null) {
      return CanonicalRequestSigner.buildHeaders(
          localSigningContext.networkId(), method, target, body, sponsorAuth);
    }
    return CanonicalRequestSigner.buildHeaders(
        localSigningContext.networkId(),
        method,
        target,
        body,
        sponsorAuth,
        timestampMs.longValue(),
        nonce);
  }

  private TransportRequest buildRequest(
      final String method,
      final URI target,
      final byte[] body,
      final Map<String, String> headers,
      final int maximumResponseBytes) {
    TransportSecurity.requireHttpRequestAllowed(
        "AtomicPrivateSettlementToriiClientV1",
        baseUri,
        target,
        headers,
        body.length == 0 ? null : body);
    final TransportRequest.Builder builder =
        TransportRequest.builder()
            .setUri(target)
            .setMethod(method)
            .setTimeout(timeout)
            .setMaximumResponseBytes(Long.valueOf(maximumResponseBytes));
    if (body.length != 0) {
      builder.setBody(body);
    }
    for (final Map.Entry<String, String> header : headers.entrySet()) {
      builder.addHeader(header.getKey(), header.getValue());
    }
    return builder.build();
  }

  private Map<String, String> requestHeaders(final boolean includeContentType) {
    final Map<String, String> headers = new LinkedHashMap<>(defaultHeaders);
    headers.put("Accept", JSON_MEDIA_TYPE);
    headers.put("Accept-Encoding", "identity");
    headers.put("Cache-Control", "no-store");
    headers.put("Pragma", "no-cache");
    if (includeContentType) {
      headers.put("Content-Type", JSON_MEDIA_TYPE);
    }
    return headers;
  }

  private URI resolvePath(final String path) {
    final String normalized = path.startsWith("/") ? path.substring(1) : path;
    final String base = baseUri.toString();
    return URI.create(base.endsWith("/") ? base + normalized : base + "/" + normalized);
  }

  private static URI requireBaseUri(final URI uri) {
    Objects.requireNonNull(uri, "baseUri");
    final String scheme = uri.getScheme();
    if (!("http".equalsIgnoreCase(scheme) || "https".equalsIgnoreCase(scheme))
        || uri.getRawQuery() != null
        || uri.getRawFragment() != null) {
      throw new IllegalArgumentException(
          "settlement Torii base URI must use HTTP(S) without query or fragment");
    }
    return uri;
  }

  private static void requireSafeDefaultHeaders(final Map<String, String> headers) {
    for (final String name : headers.keySet()) {
      if (FORBIDDEN_DEFAULT_HEADERS.contains(name.toLowerCase(Locale.ROOT))) {
        throw new IllegalArgumentException(
            "settlement header " + name + " is generated by the exact-route client");
      }
    }
  }

  private static void requireOperation(
      final AtomicPrivateSettlementPreparedRequestV1 request,
      final AtomicPrivateSettlementOperationV1 expected) {
    Objects.requireNonNull(request, "request");
    if (request.operation() != expected) {
      throw new IllegalArgumentException(
          "prepared " + request.operation() + " request cannot be sent to " + expected);
    }
  }

  private static void requireExactJsonContentType(
      final Map<String, List<String>> headers) {
    final List<String> values = new ArrayList<>();
    for (final Map.Entry<String, List<String>> header : headers.entrySet()) {
      if (header.getKey().equalsIgnoreCase("Content-Type")) {
        values.addAll(header.getValue());
      }
    }
    if (!values.equals(List.of(JSON_MEDIA_TYPE))) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "atomic private settlement response must have one exact application/json content type");
    }
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> parseExactJsonObject(final byte[] body) {
    final String text;
    try {
      text =
          StandardCharsets.UTF_8
              .newDecoder()
              .onMalformedInput(CodingErrorAction.REPORT)
              .onUnmappableCharacter(CodingErrorAction.REPORT)
              .decode(ByteBuffer.wrap(body))
              .toString();
    } catch (final Exception error) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "atomic private settlement response must be exact UTF-8", error);
    }
    final Object parsed;
    try {
      parsed = JsonParser.parse(text);
    } catch (final RuntimeException error) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "atomic private settlement response must be one strict JSON object", error);
    }
    if (!(parsed instanceof Map<?, ?>)) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "atomic private settlement response must be one strict JSON object");
    }
    return (Map<String, Object>) parsed;
  }

  private static Set<String> responseFields(final String route) {
    if (route.endsWith("/availability-shares")) {
      return Set.of("bundle_id", "payload_digest", "leg_ordinal", "disposition", "share");
    }
    if (route.endsWith("/prepare-votes") || route.endsWith("/commit-votes")) {
      return Set.of("bundle_id", "payload_digest", "leg_ordinal", "vote");
    }
    if (route.endsWith("/phase-certificates")) {
      return Set.of(
          "bundle_id",
          "payload_digest",
          "leg_ordinal",
          "lifecycle",
          "prepare_certificate",
          "commit_certificate");
    }
    if (route.endsWith("/certificates")) {
      return Set.of("bundle_id", "payload_digest", "leg_ordinal", "phase", "lifecycle");
    }
    if (route.endsWith("/legs")) {
      return Set.of("bundle_id", "payload_digest", "leg_ordinal", "disposition", "lifecycle");
    }
    if (route.endsWith("/audit-approvals")) {
      return Set.of(
          "bundle_id",
          "payload_digest",
          "leg_ordinal",
          "collected",
          "required",
          "newly_recorded",
          "lifecycle");
    }
    if (route.endsWith("/bundles")) {
      return Set.of("bundle_id", "accepted_at_height", "carrier_id", "lifecycle");
    }
    if (route.endsWith("/status")) {
      return Set.of(
          "bundle_id",
          "payload_digest",
          "leg_ordinal",
          "route",
          "stored_at_height",
          "lifecycle_height",
          "expiry_height",
          "lifecycle");
    }
    if (route.endsWith("/committee-proof")) {
      return Set.of(
          "manifest",
          "audit_policy",
          "committee_authority",
          "statement",
          "proof",
          "delta",
          "audit_approvals",
          "audit_capsule_digest",
          "availability",
          "lifecycle");
    }
    if (route.endsWith("/audit-capsule")) {
      return Set.of(
          "manifest",
          "audit_policy",
          "committee_authority",
          "statement",
          "delta",
          "audit_capsule",
          "availability",
          "lifecycle");
    }
    if (route.endsWith("/receipt")) {
      return Set.of("status", "value");
    }
    if (route.contains("/bundles/")) {
      return Set.of("manifest", "lifecycle", "finalized_height");
    }
    throw new IllegalArgumentException("unknown atomic private settlement route");
  }

  /** Builder with SDK transport injection for Android/JVM parity. */
  public static final class Builder {
    private HttpTransportExecutor executor;
    private URI baseUri = URI.create("http://localhost:8080");
    private LocalSigningContext localSigningContext;
    private Duration timeout = Duration.ofSeconds(30);
    private final Map<String, String> defaultHeaders = new LinkedHashMap<>();

    /** Override the HTTP executor. */
    public Builder executor(final HttpTransportExecutor value) {
      executor = Objects.requireNonNull(value, "executor");
      return this;
    }

    /** Set the exact Torii base, including any deployment path prefix. */
    public Builder baseUri(final URI value) {
      baseUri = Objects.requireNonNull(value, "baseUri");
      return this;
    }

    /** Bind sponsor signatures to one exact genesis-derived network identity. */
    public Builder localSigningContext(final LocalSigningContext value) {
      localSigningContext = Objects.requireNonNull(value, "localSigningContext");
      return this;
    }

    /** Set the one-shot request timeout; {@code null} delegates to the executor. */
    public Builder timeout(final Duration value) {
      timeout = value;
      return this;
    }

    /** Add one non-authentication deployment header. */
    public Builder addHeader(final String name, final String value) {
      defaultHeaders.put(
          Objects.requireNonNull(name, "name"), Objects.requireNonNull(value, "value"));
      return this;
    }

    /** Replace non-authentication deployment headers. */
    public Builder defaultHeaders(final Map<String, String> value) {
      defaultHeaders.clear();
      if (value != null) {
        defaultHeaders.putAll(value);
      }
      return this;
    }

    /** Build the exact-route client. */
    public AtomicPrivateSettlementToriiClientV1 build() {
      return new AtomicPrivateSettlementToriiClientV1(this);
    }
  }
}

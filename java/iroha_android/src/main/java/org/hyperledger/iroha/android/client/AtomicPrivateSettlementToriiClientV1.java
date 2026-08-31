package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.net.URI;
import java.nio.ByteBuffer;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Base64;
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
import java.util.regex.Pattern;
import org.hyperledger.iroha.android.consensus.NativeAmxV2Models;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;

/** Exact-route V1 client for prepared-leg, audit, coordination, and redacted query workflows. */
public final class AtomicPrivateSettlementToriiClientV1 {
  private static final class AuditApprovalRequestContext {
    private final String networkId;
    private final String bundleId;
    private final BigInteger legOrdinal;
    private final BigInteger dataspaceId;
    private final BigInteger expiryHeight;

    private AuditApprovalRequestContext(
        final String networkId,
        final String bundleId,
        final BigInteger legOrdinal,
        final BigInteger dataspaceId,
        final BigInteger expiryHeight) {
      this.networkId = networkId;
      this.bundleId = bundleId;
      this.legOrdinal = legOrdinal;
      this.dataspaceId = dataspaceId;
      this.expiryHeight = expiryHeight;
    }
  }

  private enum RestrictedResponseKind {
    COMMITTEE_PROOF,
    AUDITOR_CAPSULE,
    AUDIT_APPROVAL
  }

  private static final class RestrictedResponseVerificationContext {
    private final RestrictedResponseKind kind;
    private final byte[] requestJson;
    private final String auditorPublicKey;

    private RestrictedResponseVerificationContext(
        final RestrictedResponseKind kind,
        final byte[] requestJson,
        final String auditorPublicKey) {
      this.kind = Objects.requireNonNull(kind, "kind");
      this.requestJson = requestJson == null ? null : requestJson.clone();
      this.auditorPublicKey = auditorPublicKey;
    }

    private static RestrictedResponseVerificationContext committeeProof() {
      return new RestrictedResponseVerificationContext(
          RestrictedResponseKind.COMMITTEE_PROOF, null, null);
    }

    private static RestrictedResponseVerificationContext auditorCapsule(
        final String auditorPublicKey) {
      return new RestrictedResponseVerificationContext(
          RestrictedResponseKind.AUDITOR_CAPSULE,
          null,
          Objects.requireNonNull(auditorPublicKey, "auditorPublicKey"));
    }

    private static RestrictedResponseVerificationContext auditApproval(
        final byte[] requestJson, final String auditorPublicKey) {
      return new RestrictedResponseVerificationContext(
          RestrictedResponseKind.AUDIT_APPROVAL,
          Objects.requireNonNull(requestJson, "requestJson"),
          Objects.requireNonNull(auditorPublicKey, "auditorPublicKey"));
    }
  }

  private static final String JSON_MEDIA_TYPE = "application/json";
  private static final int RESPONSE_SMALL_MAX_BYTES = 1024 * 1024;
  private static final int RESPONSE_PUBLIC_BUNDLE_MAX_BYTES = 8 * 1024 * 1024;
  private static final int RESPONSE_RESTRICTED_MAX_BYTES = 32 * 1024 * 1024;
  private static final String INVALID_RESPONSE_MESSAGE =
      "atomic private settlement response is invalid";
  private static final BigInteger U64_MAX =
      BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE);
  private static final Pattern REJECT_CODE =
      Pattern.compile("^[A-Za-z0-9_.:-]{1,128}$");
  private static final BigInteger U8_MAX = BigInteger.valueOf(255L);
  private static final int BLS_SIGNATURE_BYTES = 96;

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
  private final AtomicPrivateSettlementResponseVerifierV1 responseVerifier;
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
    responseVerifier = Objects.requireNonNull(builder.responseVerifier, "responseVerifier");
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

  /** Submit one sponsor-signed exact global finalization or abort carrier. */
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
    requireRoleNetwork(auditorSigningContext, "auditor");
    requireOperation(request, AtomicPrivateSettlementOperationV1.AUDIT_APPROVAL);
    final String path =
        request
            .operation()
            .path()
            .replace("{payload_digest}", payloadDigest.pathComponent());
    final byte[] body = request.bytes();
    final AuditApprovalRequestContext approvalContext =
        auditApprovalRequestContext(body);
    if (!localSigningContext.networkId().literal().equals(approvalContext.networkId)) {
      throw new IllegalArgumentException(
          "prepared settlement approval must use the settlement client's exact network");
    }
    requireRestrictedResponseVerifierAvailable();
    return executeMutation(
        path,
        body,
        (target, signedBody) ->
            OperatorRequestSigner.buildHeaders(
                auditorSigningContext, "POST", target, signedBody),
        payloadDigest,
        "payload_digest",
        approvalContext,
        RestrictedResponseVerificationContext.auditApproval(
            body, auditorSigningContext.publicKey()));
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
    requireRoleNetwork(validatorSigningContext, "validator");
    requireRestrictedResponseVerifierAvailable();
    return executeGet(
        "/v1/nexus/private-settlements/legs/"
            + payloadDigest.pathComponent()
            + "/committee-proof",
        RESPONSE_RESTRICTED_MAX_BYTES,
        (target, body) ->
            OperatorRequestSigner.buildHeaders(
                validatorSigningContext, "GET", target, body),
        payloadDigest,
        null,
        RestrictedResponseVerificationContext.committeeProof());
  }

  /** Fetch one padded encrypted capsule as one exact governed local auditor. */
  public CompletableFuture<AtomicPrivateSettlementJsonResponseV1> getAuditorCapsule(
      final AtomicPrivateSettlementIdentifierV1 payloadDigest,
      final OperatorSigningContext auditorSigningContext) {
    Objects.requireNonNull(payloadDigest, "payloadDigest");
    Objects.requireNonNull(auditorSigningContext, "auditorSigningContext");
    requireRoleNetwork(auditorSigningContext, "auditor");
    requireRestrictedResponseVerifierAvailable();
    return executeGet(
        "/v1/nexus/private-settlements/legs/"
            + payloadDigest.pathComponent()
            + "/audit-capsule",
        RESPONSE_RESTRICTED_MAX_BYTES,
        (target, body) ->
            OperatorRequestSigner.buildHeaders(
                auditorSigningContext, "GET", target, body),
        payloadDigest,
        null,
        RestrictedResponseVerificationContext.auditorCapsule(
            auditorSigningContext.publicKey()));
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
        null,
        null,
        null);
  }

  private CompletableFuture<AtomicPrivateSettlementJsonResponseV1> executeMutation(
      final String path,
      final byte[] body,
      final BiFunction<URI, byte[], Map<String, String>> identityHeaders,
      final AtomicPrivateSettlementIdentifierV1 expectedIdentifier,
      final String expectedIdentifierField,
      final AuditApprovalRequestContext approvalContext,
      final RestrictedResponseVerificationContext verificationContext) {
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
        expectedIdentifierField,
        approvalContext,
        verificationContext);
  }

  private CompletableFuture<AtomicPrivateSettlementJsonResponseV1> executeGet(
      final String path,
      final int maximumResponseBytes,
      final BiFunction<URI, byte[], Map<String, String>> identityHeaders,
      final AtomicPrivateSettlementIdentifierV1 expectedIdentifier,
      final String expectedIdentifierField) {
    return executeGet(
        path,
        maximumResponseBytes,
        identityHeaders,
        expectedIdentifier,
        expectedIdentifierField,
        null);
  }

  private CompletableFuture<AtomicPrivateSettlementJsonResponseV1> executeGet(
      final String path,
      final int maximumResponseBytes,
      final BiFunction<URI, byte[], Map<String, String>> identityHeaders,
      final AtomicPrivateSettlementIdentifierV1 expectedIdentifier,
      final String expectedIdentifierField,
      final RestrictedResponseVerificationContext verificationContext) {
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
        expectedIdentifierField,
        null,
        verificationContext);
  }

  private CompletableFuture<AtomicPrivateSettlementJsonResponseV1> execute(
      final TransportRequest request,
      final String route,
      final int maximumResponseBytes,
      final AtomicPrivateSettlementIdentifierV1 expectedIdentifier,
      final String expectedIdentifierField,
      final AuditApprovalRequestContext approvalContext,
      final RestrictedResponseVerificationContext verificationContext) {
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
                expectedIdentifierField,
                approvalContext,
                verificationContext);
          } catch (final RuntimeException error) {
            final AtomicPrivateSettlementToriiExceptionV1 wrapped =
                error instanceof AtomicPrivateSettlementToriiExceptionV1 exact
                        && exact.getCause() == null
                    ? exact
                    : new AtomicPrivateSettlementToriiExceptionV1(INVALID_RESPONSE_MESSAGE);
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
      final String expectedIdentifierField,
      final AuditApprovalRequestContext approvalContext,
      final RestrictedResponseVerificationContext verificationContext) {
    final URI finalUri = response.finalUri();
    if (response.redirected()
        || finalUri == null
        || !request.uri().toASCIIString().equals(finalUri.toASCIIString())) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "atomic private settlement response must come from the exact request URL without redirects");
    }
    if (response.statusCode() < 200 || response.statusCode() >= 300) {
      final String rejectCode =
          sanitizedRejectCode(
              HttpErrorMessageExtractor.extractRejectCode(
                  response.headers(), "x-iroha-reject-code"));
      final String suffix =
          rejectCode == null || rejectCode.isBlank() ? "" : "; reject_code=" + rejectCode;
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "atomic private settlement request failed with HTTP "
              + response.statusCode()
              + suffix);
    }
    final int expectedStatus = route.endsWith("/bundles") ? 202 : 200;
    if (response.statusCode() != expectedStatus) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "atomic private settlement response status is invalid");
    }
    try {
      final byte[] body = response.body();
      if (body.length == 0 || body.length > maximumResponseBytes) {
        throw new AtomicPrivateSettlementToriiExceptionV1(
            "atomic private settlement response is empty or exceeds its route limit");
      }
      requireExactJsonContentType(response.headers());
      requireAbsentOrIdentityContentEncoding(response.headers());
      verifyRestrictedResponse(body, expectedIdentifier, verificationContext);
      final Map<String, Object> parsed = parseExactJsonObject(body);
      validateRouteShape(
          route,
          parsed,
          expectedIdentifier,
          expectedIdentifierField,
          localSigningContext.networkId().literal(),
          approvalContext);
      final byte[] canonical = JsonEncoder.encode(parsed).getBytes(StandardCharsets.UTF_8);
      return new AtomicPrivateSettlementJsonResponseV1(route, canonical);
    } catch (final RuntimeException ignored) {
      throw new AtomicPrivateSettlementToriiExceptionV1(INVALID_RESPONSE_MESSAGE);
    }
  }

  private void requireRestrictedResponseVerifierAvailable() {
    try {
      responseVerifier.requireAvailable();
    } catch (final RuntimeException | LinkageError ignored) {
      throw new IllegalStateException(
          "native private settlement response verifier is unavailable");
    }
  }

  private void verifyRestrictedResponse(
      final byte[] responseJson,
      final AtomicPrivateSettlementIdentifierV1 expectedIdentifier,
      final RestrictedResponseVerificationContext verificationContext) {
    if (verificationContext == null) {
      return;
    }
    final byte[] networkId = localSigningContext.networkId().bytes();
    final byte[] payloadDigest =
        Objects.requireNonNull(expectedIdentifier, "expectedIdentifier").bytes();
    switch (verificationContext.kind) {
      case COMMITTEE_PROOF:
        responseVerifier.verifyCommitteeProofResponse(
            responseJson.clone(), networkId, payloadDigest);
        return;
      case AUDITOR_CAPSULE:
        responseVerifier.verifyAuditorCapsuleResponse(
            responseJson.clone(),
            networkId,
            payloadDigest,
            verificationContext.auditorPublicKey);
        return;
      case AUDIT_APPROVAL:
        responseVerifier.verifyAuditApprovalResponse(
            responseJson.clone(),
            Objects.requireNonNull(verificationContext.requestJson, "requestJson").clone(),
            networkId,
            payloadDigest,
            verificationContext.auditorPublicKey);
        return;
      default:
        throw new IllegalStateException("unknown private settlement verification kind");
    }
  }

  private static String sanitizedRejectCode(final String value) {
    return value != null && REJECT_CODE.matcher(value).matches() ? value : null;
  }

  private static void validateRouteShape(
      final String route,
      final Map<String, Object> parsed,
      final AtomicPrivateSettlementIdentifierV1 expectedIdentifier,
      final String expectedIdentifierField,
      final String expectedNetworkId,
      final AuditApprovalRequestContext approvalContext) {
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
    if (route.endsWith("/bundles")) {
      validateBundleAdmission(parsed);
    } else if (route.endsWith("/audit-capsule")) {
      validateAuditorCapsuleHeight(parsed);
      validateAuditorCapsuleAttestation(
          parsed, Objects.requireNonNull(expectedIdentifier), expectedNetworkId);
    } else if (route.endsWith("/audit-approvals")) {
      validateAuditApprovalAcknowledgementAttestation(
          parsed,
          Objects.requireNonNull(expectedIdentifier),
          expectedNetworkId,
          Objects.requireNonNull(approvalContext));
    } else if (route.endsWith("/receipt")) {
      validateReceiptIdentity(parsed, Objects.requireNonNull(expectedIdentifier));
    } else if (route.contains("/bundles/")) {
      validateBundleStatusIdentity(parsed, Objects.requireNonNull(expectedIdentifier));
    }
  }

  private static void validateBundleAdmission(final Map<String, Object> parsed) {
    requireCanonicalHashLiteral(
        parsed.get("bundle_id"), "settlement bundle admission.bundle_id");
    requireCanonicalHashLiteral(
        parsed.get("carrier_id"), "settlement bundle admission.carrier_id");
    final Object rawHeight = parsed.get("accepted_at_height");
    final BigInteger acceptedAtHeight;
    if (rawHeight instanceof BigInteger integer) {
      acceptedAtHeight = integer;
    } else if (rawHeight instanceof Byte
        || rawHeight instanceof Short
        || rawHeight instanceof Integer
        || rawHeight instanceof Long) {
      acceptedAtHeight = BigInteger.valueOf(((Number) rawHeight).longValue());
    } else {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "settlement bundle admission.accepted_at_height must be an integer");
    }
    if (acceptedAtHeight.signum() < 0 || acceptedAtHeight.compareTo(U64_MAX) > 0) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "settlement bundle admission.accepted_at_height must fit in unsigned 64-bit range");
    }
  }

  private static void validateAuditorCapsuleHeight(final Map<String, Object> parsed) {
    final Object rawHeight = parsed.get("authoritative_height");
    final BigInteger authoritativeHeight;
    if (rawHeight instanceof BigInteger integer) {
      authoritativeHeight = integer;
    } else if (rawHeight instanceof Byte
        || rawHeight instanceof Short
        || rawHeight instanceof Integer
        || rawHeight instanceof Long) {
      authoritativeHeight = BigInteger.valueOf(((Number) rawHeight).longValue());
    } else {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "settlement auditor capsule.authoritative_height must be an integer");
    }
    if (authoritativeHeight.signum() <= 0 || authoritativeHeight.compareTo(U64_MAX) > 0) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "settlement auditor capsule.authoritative_height must be a nonzero unsigned 64-bit integer");
    }
  }

  @SuppressWarnings("unchecked")
  private static void validateAuditorCapsuleAttestation(
      final Map<String, Object> parsed,
      final AtomicPrivateSettlementIdentifierV1 expectedPayloadDigest,
      final String expectedNetworkId) {
    final Object rawAttestation = parsed.get("responder_attestation");
    if (!(rawAttestation instanceof Map<?, ?> attestation)
        || !attestation.keySet().equals(Set.of("body", "signature"))) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "settlement auditor capsule responder attestation is invalid");
    }
    final Object rawBody = attestation.get("body");
    final Set<String> bodyFields =
        Set.of(
            "version",
            "network_id",
            "payload_digest",
            "view_digest",
            "authority_digest",
            "lifecycle_code",
            "authoritative_height",
            "responder");
    if (!(rawBody instanceof Map<?, ?> body) || !body.keySet().equals(bodyFields)) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "settlement auditor capsule attestation body is invalid");
    }
    final Map<String, Object> typedBody = (Map<String, Object>) body;
    final Object rawLifecycle = parsed.get("lifecycle");
    final Object lifecycleStatus =
        rawLifecycle instanceof Map<?, ?> lifecycle ? lifecycle.get("status") : null;
    final int lifecycleCode;
    if ("collecting".equals(lifecycleStatus)) {
      lifecycleCode = 0;
    } else if ("audited".equals(lifecycleStatus)) {
      lifecycleCode = 1;
    } else if ("prepared".equals(lifecycleStatus)) {
      lifecycleCode = 2;
    } else if ("commit_certified".equals(lifecycleStatus)) {
      lifecycleCode = 3;
    } else if ("finalized".equals(lifecycleStatus)) {
      lifecycleCode = 4;
    } else if ("aborted".equals(lifecycleStatus)) {
      lifecycleCode = 5;
    } else if ("expired".equals(lifecycleStatus)) {
      lifecycleCode = 6;
    } else {
      lifecycleCode = -1;
    }
    final BigInteger version = attestationInteger(typedBody.get("version"));
    final BigInteger height = attestationInteger(typedBody.get("authoritative_height"));
    final BigInteger responseHeight = attestationInteger(parsed.get("authoritative_height"));
    final BigInteger code = attestationInteger(typedBody.get("lifecycle_code"));
    final Object signature = attestation.get("signature");
    if (!BigInteger.ONE.equals(version)
        || !height.equals(responseHeight)
        || !code.equals(BigInteger.valueOf(lifecycleCode))
        || !expectedNetworkId.equals(typedBody.get("network_id"))
        || !expectedPayloadDigest.jsonLiteral().equals(typedBody.get("payload_digest"))
        || !(typedBody.get("responder") instanceof String responder)
        || !NativeAmxV2Models.isCanonicalBlsNormalPeerId(responder)) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "settlement auditor capsule responder attestation is invalid");
    }
    requireCanonicalBlsSignature(
        signature, "settlement auditor capsule responder attestation.signature");
    for (final String field :
        List.of("network_id", "payload_digest", "view_digest", "authority_digest")) {
      requireCanonicalHashLiteral(
          typedBody.get(field), "settlement auditor capsule attestation." + field);
    }
  }

  private static BigInteger attestationInteger(final Object value) {
    if (value instanceof BigInteger integer) {
      return integer;
    }
    if (value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long) {
      return BigInteger.valueOf(((Number) value).longValue());
    }
    throw new AtomicPrivateSettlementToriiExceptionV1(
        "settlement auditor capsule attestation integer is invalid");
  }

  @SuppressWarnings("unchecked")
  private static void validateAuditApprovalAcknowledgementAttestation(
      final Map<String, Object> parsed,
      final AtomicPrivateSettlementIdentifierV1 expectedPayloadDigest,
      final String expectedNetworkId,
      final AuditApprovalRequestContext approvalContext) {
    final Object rawAttestation = parsed.get("responder_attestation");
    if (!(rawAttestation instanceof Map<?, ?> attestation)
        || !attestation.keySet().equals(Set.of("body", "signature"))) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "settlement approval acknowledgement responder attestation is invalid");
    }
    final Object rawBody = attestation.get("body");
    final Set<String> bodyFields =
        Set.of(
            "version",
            "network_id",
            "payload_digest",
            "approval_digest",
            "acknowledgement_digest",
            "authority_digest",
            "lifecycle_code",
            "authoritative_height",
            "responder");
    if (!(rawBody instanceof Map<?, ?> body) || !body.keySet().equals(bodyFields)) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "settlement approval acknowledgement attestation body is invalid");
    }
    final Map<String, Object> typedBody = (Map<String, Object>) body;
    final Object rawLifecycle = parsed.get("lifecycle");
    final Object lifecycleStatus =
        rawLifecycle instanceof Map<?, ?> lifecycle ? lifecycle.get("status") : null;
    final int lifecycleCode;
    if ("collecting".equals(lifecycleStatus)) {
      lifecycleCode = 0;
    } else if ("audited".equals(lifecycleStatus)) {
      lifecycleCode = 1;
    } else {
      lifecycleCode = -1;
    }
    final BigInteger version = attestationInteger(typedBody.get("version"));
    final BigInteger height = attestationInteger(typedBody.get("authoritative_height"));
    final BigInteger responseHeight = attestationInteger(parsed.get("authoritative_height"));
    final BigInteger code = attestationInteger(typedBody.get("lifecycle_code"));
    final BigInteger collected = attestationInteger(parsed.get("collected"));
    final BigInteger required = attestationInteger(parsed.get("required"));
    final BigInteger legOrdinal = attestationInteger(parsed.get("leg_ordinal"));
    final boolean lifecycleIsExact =
        collected.compareTo(required) < 0 ? lifecycleCode == 0 : lifecycleCode == 1;
    final Object signature = attestation.get("signature");
    if (!BigInteger.ONE.equals(version)
        || responseHeight.signum() <= 0
        || responseHeight.compareTo(U64_MAX) > 0
        || responseHeight.compareTo(approvalContext.expiryHeight) > 0
        || collected.signum() <= 0
        || required.signum() <= 0
        || collected.compareTo(required) > 0
        || required.compareTo(U8_MAX) > 0
        || legOrdinal.signum() < 0
        || legOrdinal.compareTo(U8_MAX) >= 0
        || !legOrdinal.equals(approvalContext.legOrdinal)
        || !(parsed.get("bundle_id") instanceof String)
        || !(parsed.get("committee_authority") instanceof Map<?, ?>)
        || !(parsed.get("newly_recorded") instanceof Boolean)
        || !lifecycleIsExact
        || !height.equals(responseHeight)
        || !expectedNetworkId.equals(typedBody.get("network_id"))
        || !approvalContext.networkId.equals(typedBody.get("network_id"))
        || !expectedPayloadDigest.jsonLiteral().equals(typedBody.get("payload_digest"))
        || !expectedPayloadDigest.jsonLiteral().equals(parsed.get("payload_digest"))
        || !code.equals(BigInteger.valueOf(lifecycleCode))
        || !(typedBody.get("responder") instanceof String responder)
        || !NativeAmxV2Models.isCanonicalBlsNormalPeerId(responder)) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "settlement approval acknowledgement responder attestation is invalid");
    }
    requireCanonicalHashLiteral(
        parsed.get("bundle_id"), "settlement approval acknowledgement.bundle_id");
    if (!approvalContext.bundleId.equals(parsed.get("bundle_id"))) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "settlement approval acknowledgement bundle is substituted");
    }
    final Map<?, ?> authority = (Map<?, ?>) parsed.get("committee_authority");
    final Object rawRoute = authority.get("route");
    if (!(rawRoute instanceof Map<?, ?> route)
        || !attestationInteger(route.get("dataspace_id"))
            .equals(approvalContext.dataspaceId)) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "settlement approval acknowledgement authority is substituted");
    }
    requireCanonicalBlsSignature(
        signature, "settlement approval acknowledgement responder attestation.signature");
    for (final String field :
        List.of(
            "network_id",
            "payload_digest",
            "approval_digest",
            "acknowledgement_digest",
            "authority_digest")) {
      requireCanonicalHashLiteral(
          typedBody.get(field),
          "settlement approval acknowledgement attestation." + field);
    }
  }

  @SuppressWarnings("unchecked")
  private static AuditApprovalRequestContext auditApprovalRequestContext(
      final byte[] bodyBytes) {
    final Map<String, Object> request = parseExactJsonObject(bodyBytes);
    if (!request.keySet().equals(Set.of("approval"))) {
      throw new IllegalArgumentException(
          "prepared settlement audit approval has invalid fields");
    }
    final Object rawApproval = request.get("approval");
    if (!(rawApproval instanceof Map<?, ?> approval)
        || !approval.keySet().equals(Set.of("body", "signature"))
        || approval.get("signature") == null) {
      throw new IllegalArgumentException("prepared settlement audit approval is invalid");
    }
    final Set<String> expectedFields =
        Set.of(
            "version",
            "network_id",
            "bundle_id",
            "leg_ordinal",
            "dataspace_id",
            "auditor_id",
            "audit_policy_digest",
            "audit_key_epoch",
            "proof_digest",
            "capsule_digest",
            "delta_digest",
            "old_root",
            "new_root",
            "expiry_height");
    final Object rawBody = approval.get("body");
    if (!(rawBody instanceof Map<?, ?> body) || !body.keySet().equals(expectedFields)) {
      throw new IllegalArgumentException(
          "prepared settlement audit approval body is invalid");
    }
    final Map<String, Object> typedBody = (Map<String, Object>) body;
    final String networkId = canonicalPreparedHash(typedBody.get("network_id"), "network_id");
    final String bundleId = canonicalPreparedHash(typedBody.get("bundle_id"), "bundle_id");
    final BigInteger version = preparedInteger(typedBody.get("version"));
    final BigInteger legOrdinal = preparedInteger(typedBody.get("leg_ordinal"));
    final BigInteger dataspaceId = preparedInteger(typedBody.get("dataspace_id"));
    final BigInteger expiryHeight = preparedInteger(typedBody.get("expiry_height"));
    if (!BigInteger.ONE.equals(version)
        || legOrdinal.signum() < 0
        || legOrdinal.compareTo(U8_MAX) >= 0
        || dataspaceId.signum() < 0
        || dataspaceId.compareTo(U64_MAX) > 0
        || expiryHeight.signum() <= 0
        || expiryHeight.compareTo(U64_MAX) > 0) {
      throw new IllegalArgumentException(
          "prepared settlement audit approval binding is invalid");
    }
    for (final String field :
        List.of(
            "audit_policy_digest",
            "proof_digest",
            "capsule_digest",
            "delta_digest")) {
      canonicalPreparedHash(typedBody.get(field), field);
    }
    return new AuditApprovalRequestContext(
        networkId, bundleId, legOrdinal, dataspaceId, expiryHeight);
  }

  private static String canonicalPreparedHash(final Object value, final String field) {
    if (!(value instanceof String literal)) {
      throw new IllegalArgumentException(
          "prepared settlement approval " + field + " is invalid");
    }
    final AtomicPrivateSettlementIdentifierV1 parsed =
        AtomicPrivateSettlementIdentifierV1.parse(literal);
    if (!parsed.jsonLiteral().equals(literal)) {
      throw new IllegalArgumentException(
          "prepared settlement approval " + field + " is noncanonical");
    }
    return literal;
  }

  private static BigInteger preparedInteger(final Object value) {
    if (value instanceof BigInteger integer) {
      return integer;
    }
    if (value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long) {
      return BigInteger.valueOf(((Number) value).longValue());
    }
    throw new IllegalArgumentException("prepared settlement approval integer is invalid");
  }

  private static void requireCanonicalHashLiteral(final Object value, final String field) {
    if (!(value instanceof String literal)) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          field + " must be a canonical Iroha hash literal");
    }
    final AtomicPrivateSettlementIdentifierV1 parsed;
    try {
      parsed = AtomicPrivateSettlementIdentifierV1.parse(literal);
    } catch (final IllegalArgumentException error) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          field + " must be a canonical Iroha hash literal", error);
    }
    if (!parsed.jsonLiteral().equals(literal)) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          field + " must be a canonical Iroha hash literal");
    }
  }

  private static void requireCanonicalBlsSignature(final Object value, final String field) {
    if (!(value instanceof String encoded)
        || encoded.isEmpty()
        || !encoded.equals(encoded.trim())) {
      throw new AtomicPrivateSettlementToriiExceptionV1(field + " must be canonical base64");
    }
    final byte[] decoded;
    try {
      decoded = Base64.getDecoder().decode(encoded);
    } catch (final IllegalArgumentException error) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          field + " must be canonical base64", error);
    }
    if (decoded.length != BLS_SIGNATURE_BYTES
        || !Base64.getEncoder().encodeToString(decoded).equals(encoded)) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          field + " must encode one exact BLS-normal signature");
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

  private void requireRoleNetwork(
      final OperatorSigningContext signingContext, final String role) {
    if (!localSigningContext.networkId().equals(signingContext.networkId())) {
      throw new IllegalArgumentException(
          role + " signing context must use the settlement client's exact network");
    }
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

  private static void requireAbsentOrIdentityContentEncoding(
      final Map<String, List<String>> headers) {
    final List<String> values = new ArrayList<>();
    for (final Map.Entry<String, List<String>> header : headers.entrySet()) {
      if (header.getKey().equalsIgnoreCase("Content-Encoding")) {
        values.addAll(header.getValue());
      }
    }
    if (!values.isEmpty()
        && (values.size() != 1 || !"identity".equalsIgnoreCase(values.get(0).trim()))) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "atomic private settlement response Content-Encoding must be absent or identity");
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
    requireJsonIntegersAreBounded(parsed);
    return (Map<String, Object>) parsed;
  }

  private static void requireJsonIntegersAreBounded(final Object value) {
    if (value instanceof Map<?, ?> map) {
      for (final Object child : map.values()) {
        requireJsonIntegersAreBounded(child);
      }
      return;
    }
    if (value instanceof List<?> list) {
      for (final Object child : list) {
        requireJsonIntegersAreBounded(child);
      }
      return;
    }
    if (value instanceof BigInteger integer && integer.bitLength() > 256) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "atomic private settlement JSON contains an oversized integer");
    }
    if (value instanceof java.math.BigDecimal
        || value instanceof Double
        || value instanceof Float) {
      throw new AtomicPrivateSettlementToriiExceptionV1(
          "atomic private settlement JSON must use the integer-only Norito profile");
    }
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
          "authoritative_height",
          "bundle_id",
          "payload_digest",
          "leg_ordinal",
          "committee_authority",
          "collected",
          "required",
          "newly_recorded",
          "lifecycle",
          "responder_attestation");
    }
    if (route.endsWith("/bundles")) {
      return Set.of("bundle_id", "accepted_at_height", "carrier_id");
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
          "authoritative_height",
          "manifest",
          "audit_policy",
          "committee_authority",
          "statement",
          "delta",
          "audit_capsule",
          "availability",
          "lifecycle",
          "responder_attestation");
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
    private AtomicPrivateSettlementResponseVerifierV1 responseVerifier =
        AtomicPrivateSettlementNativeResponseVerifierV1.instance();
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

    /** Override the fail-closed verifier, primarily for deterministic SDK tests. */
    public Builder responseVerifier(final AtomicPrivateSettlementResponseVerifierV1 value) {
      responseVerifier = Objects.requireNonNull(value, "responseVerifier");
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

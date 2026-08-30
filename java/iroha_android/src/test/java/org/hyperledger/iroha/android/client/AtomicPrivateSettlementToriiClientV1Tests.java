package org.hyperledger.iroha.android.client;

import java.io.PrintWriter;
import java.io.StringWriter;
import java.math.BigDecimal;
import java.math.BigInteger;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Set;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.android.client.transport.RequestReplayPolicy;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.testing.TestNetworkIds;

/** Shared-fixture tests for the exact atomic-private-settlement Torii contract. */
public final class AtomicPrivateSettlementToriiClientV1Tests {
  private static final String FIXTURE_PATH =
      "fixtures/norito_rpc/atomic_private_settlement_sdk_v1.json";
  private static final Map<String, Object> FIXTURE = loadFixture();
  private static final Map<String, Object> IDENTIFIERS = objectField(FIXTURE, "identifiers");
  private static final AtomicPrivateSettlementIdentifierV1 BUNDLE =
      AtomicPrivateSettlementIdentifierV1.parse(stringField(IDENTIFIERS, "bundle_hex"));
  private static final AtomicPrivateSettlementIdentifierV1 PAYLOAD =
      AtomicPrivateSettlementIdentifierV1.parse(stringField(IDENTIFIERS, "payload_hex"));

  private AtomicPrivateSettlementToriiClientV1Tests() {}

  /** Runs the settlement transport contract checks from the Gradle main-test harness. */
  public static void main(final String[] args) {
    sharedFixturePinsEveryPreparedRouteAndShape();
    sponsorLegStatusIsNetworkBoundAndIdentityChecked();
    sponsorPhaseCertificateRecoveryIsBoundAndStrictlyAllowlisted();
    bundleAdmissionUsesTheSharedExactNonterminalResponse();
    bundleAdmissionRejectsNoncanonicalHashesInvalidHeightsAndFieldDrift();
    auditorApprovalUsesPurposeSeparatedRoleHeaders();
    publicBundleQueriesAreUnsignedAndBindReceiptIdentity();
    rejectCodesAreAllowlistedBeforeEnteringPublicStatusErrors();
    malformedOrSubstitutedMaterialFailsWithoutLeakingErrorBodies();
    System.out.println("[IrohaAndroid] Atomic private settlement Torii tests passed.");
  }

  private static void sharedFixturePinsEveryPreparedRouteAndShape() {
    final List<Object> routes = listField(FIXTURE, "request_routes");
    assert routes.size() == AtomicPrivateSettlementOperationV1.values().length
        : "shared fixture must cover every closed V1 operation";
    for (final Object rawEntry : routes) {
      final Map<String, Object> entry = asObject(rawEntry, "request route");
      final AtomicPrivateSettlementOperationV1 operation =
          AtomicPrivateSettlementOperationV1.valueOf(stringField(entry, "operation"));
      assert operation.path().equals(stringField(entry, "path")) : "route path drift";
      assert operation.auth().name().equals(stringField(entry, "auth")) : "auth class drift";
      final Set<String> fields = new LinkedHashSet<>();
      for (final Object rawField : listField(entry, "top_level_fields")) {
        assert rawField instanceof String : "top-level field must be a string";
        fields.add((String) rawField);
      }
      final Map<String, Object> body = new LinkedHashMap<>();
      for (final String field : fields) {
        body.put(field, Map.of());
      }
      final AtomicPrivateSettlementPreparedRequestV1 prepared =
          AtomicPrivateSettlementPreparedRequestV1.fromNativePreparedJson(
              operation, JsonEncoder.encode(body).getBytes(StandardCharsets.UTF_8));
      assert prepared.operation() == operation : "prepared request operation drift";
      assert prepared.toString().contains("[REDACTED]") : "prepared body rendered in logs";
      prepared.close();
      assertThrows(IllegalStateException.class, prepared::bytes);
    }
  }

  private static void sponsorLegStatusIsNetworkBoundAndIdentityChecked() {
    final Map<String, Object> response =
        objectField(objectField(FIXTURE, "responses"), "leg_status");
    final CapturingExecutor executor = new CapturingExecutor(jsonResponse(response));
    final AtomicPrivateSettlementToriiClientV1 client = client(executor);
    final ToriiCanonicalRequestAuth auth =
        new ToriiCanonicalRequestAuth(
            "alice@universal",
            AtomicPrivateSettlementToriiClientV1Tests::nonZeroSignature,
            1_700_000_000_000L,
            "settlement-leg-status-1");

    final AtomicPrivateSettlementJsonResponseV1 received =
        client.getLegStatus(PAYLOAD, auth).join();

    assert executor.request.uri().getPath().equals(
            "/api/v1/nexus/private-settlements/legs/"
                + PAYLOAD.pathComponent()
                + "/status")
        : "leg status must retain the deployment prefix";
    assert "GET".equals(executor.request.method()) : "leg status must use GET";
    assert executor.request.replayPolicy() == RequestReplayPolicy.ONE_SHOT
        : "signed status request must not be replayed";
    assert executor.request.headers().containsKey(CanonicalRequestSigner.HEADER_SIGNATURE)
        : "sponsor signature missing";
    assert !executor.request.headers().containsKey(OperatorRequestSigner.HEADER_SIGNATURE)
        : "sponsor request must not carry role identity";
    assert received.toString().contains("[REDACTED]") : "response rendered in logs";
    assert !received.toString().contains(stringField(IDENTIFIERS, "payload_json"))
        : "response identifier rendered in logs";
  }

  private static void sponsorPhaseCertificateRecoveryIsBoundAndStrictlyAllowlisted() {
    final Map<String, Object> response =
        objectField(objectField(FIXTURE, "responses"), "phase_certificates");
    final CapturingExecutor executor = new CapturingExecutor(jsonResponse(response));
    final ToriiCanonicalRequestAuth auth =
        new ToriiCanonicalRequestAuth(
            "alice@universal",
            AtomicPrivateSettlementToriiClientV1Tests::nonZeroSignature,
            1_700_000_000_000L,
            "settlement-phase-certificate-recovery-1");

    final AtomicPrivateSettlementJsonResponseV1 received =
        client(executor).getPhaseCertificates(PAYLOAD, auth).join();

    assert executor.request.uri().getPath().equals(
            "/api/v1/nexus/private-settlements/legs/"
                + PAYLOAD.pathComponent()
                + "/phase-certificates")
        : "phase-certificate recovery path drift";
    assert "GET".equals(executor.request.method()) : "phase-certificate recovery must use GET";
    assert executor.request.replayPolicy() == RequestReplayPolicy.ONE_SHOT
        : "signed phase-certificate recovery must not be replayed";
    assert executor.request.headers().containsKey(CanonicalRequestSigner.HEADER_SIGNATURE)
        : "sponsor signature missing";
    assert !executor.request.headers().containsKey(OperatorRequestSigner.HEADER_SIGNATURE)
        : "sponsor recovery request must not carry role identity";
    assert received.toString().contains("[REDACTED]") : "recovery response rendered in logs";

    final Map<String, Object> missingCertificate = new LinkedHashMap<>(response);
    missingCertificate.remove("commit_certificate");
    assertThrows(
        CompletionException.class,
        () ->
            client(new CapturingExecutor(jsonResponse(missingCertificate)))
                .getPhaseCertificates(PAYLOAD, auth)
                .join());

    final Map<String, Object> nonObjectCertificate = new LinkedHashMap<>(response);
    nonObjectCertificate.put("prepare_certificate", List.of());
    assertThrows(
        CompletionException.class,
        () ->
            client(new CapturingExecutor(jsonResponse(nonObjectCertificate)))
                .getPhaseCertificates(PAYLOAD, auth)
                .join());

    final Map<String, Object> leakedField = new LinkedHashMap<>(response);
    leakedField.put("plaintext", "LEAK_CANARY");
    final CompletionException error =
        assertThrows(
            CompletionException.class,
            () ->
                client(new CapturingExecutor(jsonResponse(leakedField)))
                    .getPhaseCertificates(PAYLOAD, auth)
                    .join());
    assert !String.valueOf(error.getCause().getMessage()).contains("LEAK_CANARY")
        : "rejected recovery field leaked through error";
  }

  private static void bundleAdmissionUsesTheSharedExactNonterminalResponse() {
    final Map<String, Object> response =
        objectField(objectField(FIXTURE, "responses"), "bundle_submit");
    final CapturingExecutor executor = new CapturingExecutor(jsonResponse(response, 202));

    final AtomicPrivateSettlementJsonResponseV1 received =
        client(executor).submitBundle(bundleRequest(), sponsorAuth()).join();

    assert response.equals(
            asObject(
                JsonParser.parse(new String(received.bytes(), StandardCharsets.UTF_8)),
                "bundle admission response"))
        : "bundle admission response drift";
    assert "/api/v1/nexus/private-settlements/bundles".equals(
            executor.request.uri().getPath())
        : "bundle admission path drift";
    assert "POST".equals(executor.request.method()) : "bundle admission must use POST";
    assert executor.request.replayPolicy() == RequestReplayPolicy.ONE_SHOT
        : "bundle admission must be one-shot";
    assert executor.request.headers().containsKey(CanonicalRequestSigner.HEADER_SIGNATURE)
        : "bundle admission sponsor signature missing";
    assert !executor.request.headers().containsKey(OperatorRequestSigner.HEADER_SIGNATURE)
        : "bundle admission must not carry role identity";
    assert !response.containsKey("lifecycle")
        : "carrier admission must not claim terminal lifecycle";
  }

  private static void bundleAdmissionRejectsNoncanonicalHashesInvalidHeightsAndFieldDrift() {
    final Map<String, Object> response =
        objectField(objectField(FIXTURE, "responses"), "bundle_submit");
    final List<Map<String, Object>> invalidResponses = new ArrayList<>();

    final Map<String, Object> maximumHeight = new LinkedHashMap<>(response);
    maximumHeight.put("accepted_at_height", new BigInteger("18446744073709551615"));
    client(new CapturingExecutor(jsonResponse(maximumHeight, 202)))
        .submitBundle(bundleRequest(), sponsorAuth())
        .join();

    final CompletionException wrongCarrierStatus =
        assertThrows(
            CompletionException.class,
            () ->
                client(new CapturingExecutor(jsonResponse(response, 200)))
                    .submitBundle(bundleRequest(), sponsorAuth())
                    .join());
    assert wrongCarrierStatus.getCause() instanceof AtomicPrivateSettlementToriiExceptionV1
        : "carrier admission must require HTTP 202";

    final Map<String, Object> missing = new LinkedHashMap<>(response);
    missing.remove("carrier_id");
    invalidResponses.add(missing);

    final Map<String, Object> extra = new LinkedHashMap<>(response);
    extra.put("unexpected", Boolean.TRUE);
    invalidResponses.add(extra);

    final Map<String, Object> lifecycle = new LinkedHashMap<>(response);
    lifecycle.put("lifecycle", Map.of("status", "aborted"));
    invalidResponses.add(lifecycle);

    final Map<String, Object> wrongBundleType = new LinkedHashMap<>(response);
    wrongBundleType.put("bundle_id", List.of());
    invalidResponses.add(wrongBundleType);

    final Map<String, Object> wrongCarrierType = new LinkedHashMap<>(response);
    wrongCarrierType.put("carrier_id", Long.valueOf(42L));
    invalidResponses.add(wrongCarrierType);

    final Map<String, Object> rawBundleHash = new LinkedHashMap<>(response);
    rawBundleHash.put("bundle_id", stringField(IDENTIFIERS, "bundle_hex"));
    invalidResponses.add(rawBundleHash);

    final Map<String, Object> noncanonicalCase = new LinkedHashMap<>(response);
    noncanonicalCase.put(
        "bundle_id",
        AtomicPrivateSettlementIdentifierV1.parse("ab".repeat(32))
            .jsonLiteral()
            .toLowerCase(java.util.Locale.ROOT));
    invalidResponses.add(noncanonicalCase);

    final Map<String, Object> rawCarrierHash = new LinkedHashMap<>(response);
    rawCarrierHash.put("carrier_id", stringField(IDENTIFIERS, "payload_hex"));
    invalidResponses.add(rawCarrierHash);

    for (final Object invalidHeight :
        List.of(
            "105",
            new BigDecimal("105.0"),
            Long.valueOf(-1L),
            new BigInteger("18446744073709551616"))) {
      final Map<String, Object> candidate = new LinkedHashMap<>(response);
      candidate.put("accepted_at_height", invalidHeight);
      invalidResponses.add(candidate);
    }

    for (int index = 0; index < invalidResponses.size(); index++) {
      final Map<String, Object> candidate = invalidResponses.get(index);
      final CompletionException error =
          assertThrows(
              CompletionException.class,
              () ->
                  client(new CapturingExecutor(jsonResponse(candidate, 202)))
                      .submitBundle(bundleRequest(), sponsorAuth())
                      .join());
      assert error.getCause() instanceof AtomicPrivateSettlementToriiExceptionV1
          : "bundle admission case " + index + " must fail as a redacted settlement error";
    }
  }

  private static void auditorApprovalUsesPurposeSeparatedRoleHeaders() {
    final Map<String, Object> response =
        objectField(objectField(FIXTURE, "responses"), "audit_approval");
    final CapturingExecutor executor = new CapturingExecutor(jsonResponse(response));
    final AtomicPrivateSettlementToriiClientV1 client = client(executor);
    final AtomicPrivateSettlementPreparedRequestV1 request =
        AtomicPrivateSettlementPreparedRequestV1.fromNativePreparedJson(
            AtomicPrivateSettlementOperationV1.AUDIT_APPROVAL,
            "{\"approval\":{}}".getBytes(StandardCharsets.UTF_8));
    final OperatorSigningContext roleContext =
        new OperatorSigningContext(
            TestNetworkIds.canonical(),
            "ed0120" + "11".repeat(32),
            AtomicPrivateSettlementToriiClientV1Tests::nonZeroSignature);

    client.submitAuditApproval(PAYLOAD, request, roleContext).join();

    assert executor.request.uri().getPath().equals(
            "/api/v1/nexus/private-settlements/legs/"
                + PAYLOAD.pathComponent()
                + "/audit-approvals")
        : "audit approval path must bind the payload digest";
    assert executor.request.replayPolicy() == RequestReplayPolicy.ONE_SHOT
        : "auditor approval must be one-shot";
    assert executor.request.headers().containsKey(OperatorRequestSigner.HEADER_SIGNATURE)
        : "auditor role signature missing";
    assert !executor.request.headers().containsKey(CanonicalRequestSigner.HEADER_SIGNATURE)
        : "auditor request must not fall back to sponsor identity";
  }

  private static void publicBundleQueriesAreUnsignedAndBindReceiptIdentity() {
    final Map<String, Object> responses = objectField(FIXTURE, "responses");
    final CapturingExecutor statusExecutor =
        new CapturingExecutor(jsonResponse(objectField(responses, "bundle_status_aborted")));
    client(statusExecutor).getBundleStatus(BUNDLE).join();
    assert statusExecutor.request.replayPolicy() == RequestReplayPolicy.RETRY_SAFE
        : "public bundle status must be retry-safe";
    for (final String header : statusExecutor.request.headers().keySet()) {
      assert !header.regionMatches(true, 0, "X-Iroha", 0, "X-Iroha".length())
          : "public bundle status must remain unsigned";
    }

    final CapturingExecutor receiptExecutor =
        new CapturingExecutor(jsonResponse(objectField(responses, "receipt_pending")));
    client(receiptExecutor).getBundleReceipt(BUNDLE).join();
    assert receiptExecutor.request.uri().getPath().equals(
            "/api/v1/nexus/private-settlements/bundles/"
                + BUNDLE.pathComponent()
                + "/receipt")
        : "receipt path drift";

    final CompletionException wrongReceiptStatus =
        assertThrows(
            CompletionException.class,
            () ->
                client(
                        new CapturingExecutor(
                            jsonResponse(objectField(responses, "receipt_pending"), 201)))
                    .getBundleReceipt(BUNDLE)
                    .join());
    assert wrongReceiptStatus.getCause() instanceof AtomicPrivateSettlementToriiExceptionV1
        : "public receipt must require HTTP 200";

    final Map<String, Object> substituted =
        new LinkedHashMap<>(objectField(responses, "receipt_pending"));
    final Map<String, Object> value =
        new LinkedHashMap<>(objectField(substituted, "value"));
    value.put("bundle_id", stringField(IDENTIFIERS, "payload_json"));
    substituted.put("value", value);
    final CompletionException error =
        assertThrows(
            CompletionException.class,
            () -> client(new CapturingExecutor(jsonResponse(substituted)))
                .getBundleReceipt(BUNDLE)
                .join());
    assert error.getCause() instanceof AtomicPrivateSettlementToriiExceptionV1
        : "receipt substitution must fail as a redacted settlement error";
  }

  private static void rejectCodesAreAllowlistedBeforeEnteringPublicStatusErrors() {
    assert (
            "atomic private settlement request failed with HTTP 403; "
                    + "reject_code=APS_POLICY_DENIED")
        .equals(rejectionMessage("APS_POLICY_DENIED"))
        : "valid reject code must survive in the public status message";

    final String oversized = "A".repeat(129);
    final String oversizedMessage = rejectionMessage(oversized);
    assert "atomic private settlement request failed with HTTP 403".equals(oversizedMessage)
        : "oversized reject code must be omitted";
    assert !oversizedMessage.contains(oversized)
        : "oversized reject code leaked through the public status message";

    final String secretShaped = "memo=LEAK_CANARY_987654";
    final String secretShapedMessage = rejectionMessage(secretShaped);
    assert "atomic private settlement request failed with HTTP 403".equals(secretShapedMessage)
        : "secret-shaped reject code must be omitted";
    assert !secretShapedMessage.contains(secretShaped)
        : "secret-shaped reject code leaked through the public status message";
  }

  private static String rejectionMessage(final String rejectCode) {
    final TransportResponse rejection =
        TransportResponse.builder()
            .setStatusCode(403)
            .addHeader("X-Iroha-Reject-Code", rejectCode)
            .build();
    final CompletionException error =
        assertThrows(
            CompletionException.class,
            () ->
                client(new CapturingExecutor(rejection))
                    .getBundleStatus(BUNDLE)
                    .join());
    assert error.getCause() instanceof AtomicPrivateSettlementToriiExceptionV1
        : "HTTP rejection must fail as a settlement error";
    return String.valueOf(error.getCause().getMessage());
  }

  private static void malformedOrSubstitutedMaterialFailsWithoutLeakingErrorBodies() {
    assertThrows(
        IllegalArgumentException.class,
        () -> AtomicPrivateSettlementPreparedRequestV1.fromNativePreparedJson(
            AtomicPrivateSettlementOperationV1.AUDIT_APPROVAL,
            "{\"approval\":{},\"approval\":{}}".getBytes(StandardCharsets.UTF_8)));
    assertThrows(
        IllegalArgumentException.class,
        () -> AtomicPrivateSettlementPreparedRequestV1.fromNativePreparedJson(
            AtomicPrivateSettlementOperationV1.AUDIT_APPROVAL,
            new byte[] {0x7b, 0x22, (byte) 0xc3, 0x28, 0x22, 0x7d}));

    final TransportResponse rejection =
        TransportResponse.builder()
            .setStatusCode(400)
            .setBody("memo=LEAK_CANARY amount=987654".getBytes(StandardCharsets.UTF_8))
            .addHeader("Content-Type", "text/plain")
            .addHeader("X-Iroha-Reject-Code", "memo=LEAK_CANARY_987654")
            .build();
    final AtomicPrivateSettlementToriiClientV1 client =
        client(new CapturingExecutor(rejection));
    final AtomicPrivateSettlementPreparedRequestV1 wrong =
        AtomicPrivateSettlementPreparedRequestV1.fromNativePreparedJson(
            AtomicPrivateSettlementOperationV1.BUNDLE_SUBMIT,
            "{\"transaction\":{}}".getBytes(StandardCharsets.UTF_8));
    final ToriiCanonicalRequestAuth sponsor =
        new ToriiCanonicalRequestAuth(
            "alice@universal",
            AtomicPrivateSettlementToriiClientV1Tests::nonZeroSignature);
    assertThrows(IllegalArgumentException.class, () -> client.uploadLeg(wrong, sponsor));

    final AtomicPrivateSettlementPreparedRequestV1 approval =
        AtomicPrivateSettlementPreparedRequestV1.fromNativePreparedJson(
            AtomicPrivateSettlementOperationV1.AUDIT_APPROVAL,
            "{\"approval\":{}}".getBytes(StandardCharsets.UTF_8));
    final OperatorSigningContext auditor =
        new OperatorSigningContext(
            TestNetworkIds.canonical(),
            "ed0120" + "22".repeat(32),
            AtomicPrivateSettlementToriiClientV1Tests::nonZeroSignature);
    final CompletionException error =
        assertThrows(
            CompletionException.class,
            () -> client.submitAuditApproval(PAYLOAD, approval, auditor).join());
    assert !String.valueOf(error.getCause().getMessage()).contains("LEAK_CANARY")
        : "server body canary leaked through error";
    assert !String.valueOf(error.getCause().getMessage()).contains("987654")
        : "server amount leaked through error";

    final String responseCanary = "APS_PRIVATE_KEY_RESPONSE_CANARY_9F48A3";
    final String secretKey = "private_key_" + responseCanary;
    final TransportResponse malformedResponse =
        TransportResponse.builder()
            .setStatusCode(200)
            .setBody(
                ("{\""
                        + secretKey
                        + "\":\"first\",\""
                        + secretKey
                        + "\":\"second\"}")
                    .getBytes(StandardCharsets.UTF_8))
            .addHeader("Content-Type", "application/json")
            .build();
    final CompletionException malformedError =
        assertThrows(
            CompletionException.class,
            () ->
                client(new CapturingExecutor(malformedResponse))
                    .getBundleReceipt(BUNDLE)
                    .join());
    final Throwable responseFailure = malformedError.getCause();
    assert responseFailure instanceof AtomicPrivateSettlementToriiExceptionV1
        : "malformed response must fail as a settlement error";
    assert "atomic private settlement response is invalid".equals(responseFailure.getMessage())
        : "malformed response must use the fixed redacted message";
    assert responseFailure.getCause() == null
        : "malformed response parser cause must be discarded";
    assert !malformedError.toString().contains(responseCanary)
        : "malformed response canary leaked through the throwable";
    assert !responseFailure.toString().contains(responseCanary)
        : "malformed response canary leaked through the cause";
    assert !renderThrowable(malformedError).contains(responseCanary)
        : "malformed response canary leaked through the stack rendering";

    final TransportResponse redirected =
        new TransportResponse(
            200,
            JsonEncoder.encode(
                    objectField(
                        objectField(FIXTURE, "responses"), "bundle_status_aborted"))
                .getBytes(StandardCharsets.UTF_8),
            "",
            Map.of("Content-Type", List.of("application/json")),
            URI.create("https://collector.invalid/status"),
            true);
    assertThrows(
        CompletionException.class,
        () -> client(new CapturingExecutor(redirected)).getBundleStatus(BUNDLE).join());
  }

  private static AtomicPrivateSettlementToriiClientV1 client(
      final HttpTransportExecutor executor) {
    return AtomicPrivateSettlementToriiClientV1.builder()
        .executor(executor)
        .baseUri(URI.create("https://torii.example/api"))
        .localSigningContext(new LocalSigningContext(TestNetworkIds.canonical()))
        .build();
  }

  private static AtomicPrivateSettlementPreparedRequestV1 bundleRequest() {
    return AtomicPrivateSettlementPreparedRequestV1.fromNativePreparedJson(
        AtomicPrivateSettlementOperationV1.BUNDLE_SUBMIT,
        "{\"transaction\":{}}".getBytes(StandardCharsets.UTF_8));
  }

  private static ToriiCanonicalRequestAuth sponsorAuth() {
    return new ToriiCanonicalRequestAuth(
        "alice@universal", AtomicPrivateSettlementToriiClientV1Tests::nonZeroSignature);
  }

  private static String renderThrowable(final Throwable error) {
    final StringWriter output = new StringWriter();
    error.printStackTrace(new PrintWriter(output));
    return output.toString();
  }

  private static TransportResponse jsonResponse(final Map<String, Object> value) {
    return jsonResponse(value, 200);
  }

  private static TransportResponse jsonResponse(
      final Map<String, Object> value, final int statusCode) {
    return TransportResponse.builder()
        .setStatusCode(statusCode)
        .setBody(JsonEncoder.encode(value).getBytes(StandardCharsets.UTF_8))
        .addHeader("Content-Type", "application/json")
        .build();
  }

  private static byte[] nonZeroSignature(final byte[] message) {
    final byte[] signature = Arrays.copyOf(message, 64);
    if (signature.length != 0) {
      signature[signature.length - 1] |= 1;
    }
    return signature;
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> loadFixture() {
    Path current = Paths.get("").toAbsolutePath();
    while (current != null) {
      final Path candidate = current.resolve(FIXTURE_PATH);
      if (Files.isRegularFile(candidate)) {
        try {
          final Object parsed =
              JsonParser.parse(Files.readString(candidate, StandardCharsets.UTF_8));
          return asObject(parsed, "settlement fixture");
        } catch (final Exception error) {
          throw new IllegalStateException("failed to read settlement fixture", error);
        }
      }
      current = current.getParent();
    }
    throw new IllegalStateException(FIXTURE_PATH + " was not found");
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> asObject(final Object value, final String label) {
    if (!(value instanceof Map<?, ?>)) {
      throw new IllegalArgumentException(label + " must be an object");
    }
    return (Map<String, Object>) value;
  }

  private static Map<String, Object> objectField(
      final Map<String, Object> object, final String name) {
    return asObject(object.get(name), name);
  }

  @SuppressWarnings("unchecked")
  private static List<Object> listField(
      final Map<String, Object> object, final String name) {
    final Object value = object.get(name);
    if (!(value instanceof List<?>)) {
      throw new IllegalArgumentException(name + " must be an array");
    }
    return (List<Object>) value;
  }

  private static String stringField(
      final Map<String, Object> object, final String name) {
    final Object value = object.get(name);
    if (!(value instanceof String)) {
      throw new IllegalArgumentException(name + " must be a string");
    }
    return (String) value;
  }

  private static <T extends Throwable> T assertThrows(
      final Class<T> expected, final ThrowingAction action) {
    try {
      action.run();
    } catch (final Throwable error) {
      if (expected.isInstance(error)) {
        return expected.cast(error);
      }
      throw new AssertionError("unexpected failure type", error);
    }
    throw new AssertionError("expected " + expected.getSimpleName());
  }

  @FunctionalInterface
  private interface ThrowingAction {
    void run() throws Throwable;
  }

  private static final class CapturingExecutor implements HttpTransportExecutor {
    private final TransportResponse response;
    private TransportRequest request;

    private CapturingExecutor(final TransportResponse response) {
      this.response = response;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest candidate) {
      if (request != null) {
        throw new IllegalStateException("settlement requests must be dispatched exactly once");
      }
      request = candidate;
      final URI finalUri = response.finalUri() == null ? candidate.uri() : response.finalUri();
      return CompletableFuture.completedFuture(
          new TransportResponse(
              response.statusCode(),
              response.body(),
              response.message(),
              response.headers(),
              finalUri,
              response.redirected()));
    }
  }
}

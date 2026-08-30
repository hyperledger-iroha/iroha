package org.hyperledger.iroha.android.client;

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
    auditorApprovalUsesPurposeSeparatedRoleHeaders();
    publicBundleQueriesAreUnsignedAndBindReceiptIdentity();
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

  private static TransportResponse jsonResponse(final Map<String, Object> value) {
    return TransportResponse.builder()
        .setStatusCode(200)
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

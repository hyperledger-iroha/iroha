// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.client.KagemushaToriiModelsV1.OperationState;
import org.hyperledger.iroha.android.client.KagemushaToriiModelsV1.UnverifiedOperationStatus;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.model.FeePaymentIntent;
import org.hyperledger.iroha.android.model.TransactionAdmissionIntent;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.model.instructions.TopUpKagemushaV1Instruction;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.norito.SignedTransactionEncoder;
import org.hyperledger.iroha.android.offline.KagemushaNoritoV1;
import org.hyperledger.iroha.android.testing.TestAccountIds;
import org.hyperledger.iroha.android.tx.SignedTransaction;

/** Focused route and finality-boundary tests for the mirrored Java KAGEMUSHA V1 client. */
public final class KagemushaToriiClientV1Tests {
  private KagemushaToriiClientV1Tests() {}

  /** Runs the focused checks from the Gradle main-test harness. */
  public static void main(final String[] args) throws Exception {
    builderProvidesTheSameDefaultTransportAsTheKotlinFacade();
    readinessUsesTheCanonicalKagemushaRouteAndPreservesUnavailableState();
    operationPollingUsesTheCanonicalKagemushaRouteAndWithholdsTheResult();
    signedTopUpForwardsTheCanonicalVersionedTransactionUnchanged();
    submissionResponsesEnforceLocationStatusAndRetryContract();
    mutationRoutesAndCanonicalCodecsAreTheSolePublicV1Surface();
    System.out.println("[IrohaAndroid] KAGEMUSHA V1 Torii client tests passed.");
  }

  private static void builderProvidesTheSameDefaultTransportAsTheKotlinFacade() {
    assert KagemushaToriiClientV1.builder()
            .baseUri(URI.create("https://torii.example/"))
            .build()
        != null;
  }

  private static void readinessUsesTheCanonicalKagemushaRouteAndPreservesUnavailableState() {
    final CapturingExecutor executor = new CapturingExecutor(jsonResponse(Map.of(
        "kagemusha_handoff_capability", "kagemusha_handoff_v1",
        "wire_version", 1,
        "device_lifecycle_version", 1,
        "ready", false)));

    final KagemushaToriiModelsV1.Readiness readiness = client(executor).getReadiness().join();

    assert "/api/v1/kagemusha/readiness".equals(executor.request.uri().getPath());
    assert "GET".equals(executor.request.method());
    assert !readiness.ready() : "ready=false is valid readiness metadata";
    assert "kagemusha_handoff_v1".equals(readiness.kagemushaHandoffCapability());
  }

  private static void operationPollingUsesTheCanonicalKagemushaRouteAndWithholdsTheResult() {
    final byte[] operationId = filled(0xd1);
    final Map<String, Object> response = new LinkedHashMap<>();
    response.put("version", 1);
    response.put("operation_id", unsignedBytes(operationId));
    response.put("kind", taggedUnit("kind", "redemption"));
    response.put("state", taggedUnit("state", "applied"));
    response.put("result", Map.of("untrusted_finality", "opaque"));
    response.put("rejection", null);
    final CapturingExecutor executor = new CapturingExecutor(jsonResponse(response));

    final UnverifiedOperationStatus status = client(executor).getOperation(operationId).join();

    assert ("/api/v1/kagemusha/operations/" + lowerHex(operationId))
        .equals(executor.request.uri().getPath());
    assert "GET".equals(executor.request.method());
    assert status.state() == OperationState.APPLIED;
    assert status.toString().contains("[WITHHELD]");
    assert !status.toString().contains("opaque");
    final String released = status.verifyAgainst(
        "pinned-anchor",
        (canonicalJson, anchor) -> {
          assert "pinned-anchor".equals(anchor);
          return new String(canonicalJson, StandardCharsets.UTF_8);
        });
    assert released.contains("untrusted_finality");
  }

  private static void mutationRoutesAndCanonicalCodecsAreTheSolePublicV1Surface() {
    assert "/v1/kagemusha/readiness".equals(KagemushaToriiClientV1.READINESS_PATH);
    assert "/v1/kagemusha/top-up".equals(KagemushaToriiClientV1.TOP_UP_PATH);
    assert "/v1/kagemusha/redeem".equals(KagemushaToriiClientV1.REDEEM_PATH);
    assert "/v1/kagemusha/operations/".equals(KagemushaToriiClientV1.OPERATION_PATH_PREFIX);
    assert KagemushaNoritoV1.MAXIMUM_TOP_UP_REQUEST_BYTES == 16 * 1024;
    assert "iroha.kagemusha.v1.top_up".equals(TopUpKagemushaV1Instruction.WIRE_ID);
    assert "iroha_data_model::isi::kagemusha_v1::TopUpKagemushaV1"
        .equals(TopUpKagemushaV1Instruction.SCHEMA_NAME);
    expectIllegalArgument(
        () ->
            KagemushaNoritoV1.decodeTopUpRequestShapeExact(
                new byte[KagemushaNoritoV1.MAXIMUM_TOP_UP_REQUEST_BYTES + 1]));
  }

  private static void signedTopUpForwardsTheCanonicalVersionedTransactionUnchanged()
      throws Exception {
    final SignedTransaction transaction = signedTransaction(0x41);
    final byte[] operationId = filled(0x42);
    final Map<String, Object> response = new LinkedHashMap<>();
    response.put("version", 1);
    response.put("operation_id", unsignedBytes(operationId));
    response.put("kind", taggedUnit("kind", "top_up"));
    response.put("state", taggedUnit("state", "pending"));
    response.put("result", null);
    response.put("rejection", null);
    final CapturingExecutor executor = new CapturingExecutor(jsonResponse(
        response,
        202,
        Map.of(
            "Location", KagemushaToriiClientV1.OPERATION_PATH_PREFIX + lowerHex(operationId),
            "Retry-After", "1")));

    client(executor).submitTopUp(transaction, operationId).join();

    assert "/api/v1/kagemusha/top-up".equals(executor.request.uri().getPath());
    assert "POST".equals(executor.request.method());
    assert "application/x-norito".equals(firstHeader(executor.request, "Content-Type"));
    assert lowerHex(operationId).equals(firstHeader(executor.request, "Idempotency-Key"));
    assert Arrays.equals(
        SignedTransactionEncoder.encodeVersioned(transaction), executor.request.body());

    expectIllegalArgument(() -> client(executor).submitTopUp(transaction, new byte[31]));
    expectIllegalArgument(() -> client(executor).submitTopUp(transaction, new byte[32]));
  }

  private static void submissionResponsesEnforceLocationStatusAndRetryContract() {
    final SignedTransaction transaction = signedTransaction(0x43);
    final byte[] operationId = filled(0x44);
    final Map<String, Object> pending = operationStatus(operationId, "pending", null, null);
    final Map<String, Object> applied = operationStatus(
        operationId, "applied", Map.of("untrusted_finality", "opaque"), null);
    final Map<String, Object> rejection = new LinkedHashMap<>();
    rejection.put("code", taggedUnit("code", "invalid_request"));
    rejection.put("detail_digest", unsignedBytes(filled(0x45)));
    final Map<String, Object> rejected =
        operationStatus(operationId, "rejected", null, rejection);

    final UnverifiedOperationStatus appliedStatus =
        client(
                new CapturingExecutor(
                    jsonResponse(applied, 200, submissionHeaders(operationId, null))))
            .submitTopUp(transaction, operationId)
            .join();
    assert appliedStatus.state() == OperationState.APPLIED;
    final UnverifiedOperationStatus rejectedStatus =
        client(
                new CapturingExecutor(
                    jsonResponse(rejected, 200, submissionHeaders(operationId, null))))
            .submitTopUp(transaction, operationId)
            .join();
    assert rejectedStatus.state() == OperationState.REJECTED;

    final Map<String, String> missingLocation = new LinkedHashMap<>();
    missingLocation.put("Retry-After", "1");
    final TransportResponse[] invalidResponses = new TransportResponse[] {
        jsonResponse(pending, 202, missingLocation),
        jsonResponse(pending, 202, submissionHeaders(operationId, "0")),
        jsonResponse(pending, 202, submissionHeaders(operationId, "+1")),
        jsonResponse(pending, 202, submissionHeaders(operationId, "tomorrow")),
        jsonResponse(pending, 202, submissionHeaders(operationId, null)),
        jsonResponse(applied, 202, submissionHeaders(operationId, "1")),
        jsonResponse(pending, 200, submissionHeaders(operationId, null)),
        jsonResponse(applied, 200, submissionHeaders(operationId, "1")),
        jsonResponse(applied, 200, submissionHeaders(filled(0x7f), null)),
    };
    for (final TransportResponse response : invalidResponses) {
      expectCompletionFailure(
          () ->
              client(new CapturingExecutor(response))
                  .submitTopUp(transaction, operationId)
                  .join());
    }
  }

  private static KagemushaToriiClientV1 client(final HttpTransportExecutor executor) {
    return KagemushaToriiClientV1.builder()
        .executor(executor)
        .baseUri(URI.create("https://torii.example/api/"))
        .build();
  }

  private static Map<String, Object> taggedUnit(final String tag, final String value) {
    final Map<String, Object> result = new LinkedHashMap<>();
    result.put(tag, value);
    result.put("value", null);
    return result;
  }

  private static Map<String, Object> operationStatus(
      final byte[] operationId,
      final String state,
      final Object resultValue,
      final Object rejectionValue) {
    final Map<String, Object> result = new LinkedHashMap<>();
    result.put("version", 1);
    result.put("operation_id", unsignedBytes(operationId));
    result.put("kind", taggedUnit("kind", "top_up"));
    result.put("state", taggedUnit("state", state));
    result.put("result", resultValue);
    result.put("rejection", rejectionValue);
    return result;
  }

  private static Map<String, String> submissionHeaders(
      final byte[] operationId, final String retryAfter) {
    final Map<String, String> result = new LinkedHashMap<>();
    result.put("Location", KagemushaToriiClientV1.OPERATION_PATH_PREFIX + lowerHex(operationId));
    if (retryAfter != null) {
      result.put("Retry-After", retryAfter);
    }
    return result;
  }

  private static List<Integer> unsignedBytes(final byte[] bytes) {
    return Arrays.stream(toUnsignedInts(bytes)).boxed().toList();
  }

  private static SignedTransaction signedTransaction(final int seed) {
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setFeePayment(FeePaymentIntent.authority(Collections.emptyList(), 1L))
            .setNetworkId(org.hyperledger.iroha.android.testing.TestNetworkIds.fromSeed(seed))
            .setAuthority(TestAccountIds.ed25519Authority(0x37))
            .setCreationTimeMs(1_700_000_000_000L + seed)
            .setInstructionBytes(new byte[] {(byte) seed, (byte) (seed + 1)})
            .setTimeToLiveMs(5_000L)
            .setNonce(seed + 1L)
            .setAdmissionIntent(TransactionAdmissionIntent.QUEUE_PLAN_SYNCED)
            .setMetadata(Collections.emptyMap())
            .build();
    final NoritoJavaCodecAdapter codec =
        new NoritoJavaCodecAdapter(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    try {
      return new SignedTransaction(
          codec.encodeTransaction(payload),
          filledBytes(64, seed + 1),
          filledBytes(32, seed + 2),
          codec.schemaName());
    } catch (final Exception error) {
      throw new IllegalStateException("failed to encode KAGEMUSHA top-up test transaction", error);
    }
  }

  private static int[] toUnsignedInts(final byte[] bytes) {
    final int[] result = new int[bytes.length];
    for (int index = 0; index < bytes.length; index++) {
      result[index] = bytes[index] & 0xff;
    }
    return result;
  }

  private static byte[] filled(final int value) {
    return filledBytes(32, value);
  }

  private static byte[] filledBytes(final int size, final int value) {
    final byte[] result = new byte[size];
    Arrays.fill(result, (byte) value);
    return result;
  }

  private static String firstHeader(final TransportRequest request, final String name) {
    return request.headers().entrySet().stream()
        .filter(entry -> name.equalsIgnoreCase(entry.getKey()))
        .flatMap(entry -> entry.getValue().stream())
        .findFirst()
        .orElse(null);
  }

  private static void expectIllegalArgument(final Runnable action) {
    try {
      action.run();
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      // Expected.
    }
  }

  private static void expectCompletionFailure(final Runnable action) {
    try {
      action.run();
      throw new AssertionError("expected CompletionException");
    } catch (final CompletionException expected) {
      assert expected.getCause()
          instanceof org.hyperledger.iroha.sdk.client.KagemushaToriiExceptionV1;
    }
  }

  private static String lowerHex(final byte[] value) {
    final StringBuilder result = new StringBuilder(value.length * 2);
    for (final byte item : value) {
      result.append(String.format("%02x", item & 0xff));
    }
    return result.toString();
  }

  private static TransportResponse jsonResponse(final Map<String, ?> value) {
    return jsonResponse(value, 200, Collections.emptyMap());
  }

  private static TransportResponse jsonResponse(
      final Map<String, ?> value,
      final int status,
      final Map<String, String> headers) {
    final TransportResponse.Builder response = TransportResponse.builder()
        .setStatusCode(status)
        .setBody(JsonEncoder.encode(value).getBytes(StandardCharsets.UTF_8))
        .addHeader("Content-Type", "application/json");
    headers.forEach(response::addHeader);
    return response.build();
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
        throw new IllegalStateException("KAGEMUSHA request dispatched more than once");
      }
      request = candidate;
      return CompletableFuture.completedFuture(new TransportResponse(
          response.statusCode(),
          response.body(),
          response.message(),
          response.headers(),
          candidate.uri(),
          false));
    }
  }
}

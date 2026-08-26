package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.net.URI;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import java.util.Locale;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import java.util.concurrent.atomic.AtomicInteger;
import org.hyperledger.iroha.android.client.LocalSigningContext;
import org.hyperledger.iroha.android.client.transport.TransportExecutor;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.model.NetworkId;
import org.junit.Test;

/** Adversarial parity tests for the public Java Offline Cash V1 Torii facade. */
public final class OfflineCashToriiV1Tests {

  @Test
  public void fixtureLoaderEnforcesTheExactEager40RowContract() {
    final List<String> rows = OfflineCashToriiV1Fixtures.canonicalRowsForTest();
    assertEquals(40, rows.size());
    assertEquals(40, OfflineCashToriiV1Fixtures.parseRows(rows).size());

    final String networkId = fixtureValue(rows, "network_id");
    final String topUpRequest = fixtureValue(rows, "top_up_request");
    final List<List<String>> invalidRowSets = new ArrayList<>();
    invalidRowSets.add(withExtraRow(rows, "unexpected_fixture=00"));
    invalidRowSets.add(new ArrayList<>(rows.subList(0, rows.size() - 1)));
    invalidRowSets.add(withExtraRow(rows, rows.get(0)));
    invalidRowSets.add(
        replaceFixtureRow(rows, "network_id", networkId.toUpperCase(Locale.ROOT)));
    invalidRowSets.add(
        replaceFixtureRow(
            rows, "network_id", networkId.substring(0, networkId.length() - 1) + "0"));
    invalidRowSets.add(replaceFixtureRow(rows, "top_up_submitted_at_ms", "01"));
    invalidRowSets.add(replaceFixtureRow(rows, "top_up_request", "0"));
    invalidRowSets.add(
        replaceFixtureRow(rows, "top_up_request", topUpRequest.toUpperCase(Locale.ROOT)));
    invalidRowSets.add(replaceFixtureRow(rows, "top_up_request", ""));
    invalidRowSets.add(withExtraRow(rows, "missing_separator"));
    for (final List<String> invalidRows : invalidRowSets) {
      assertThrows(
          IllegalStateException.class, () -> OfflineCashToriiV1Fixtures.parseRows(invalidRows));
    }
  }

  @Test
  public void canonicalWrappersRequireAuthoritativeSemanticBindings() {
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new KagemushaRecursiveSpendProver.SubmissionRequestProjection(
                new byte[32], filled(32, (byte) 1), "1"));
    final byte[] topUpSource = OfflineCashToriiV1Fixtures.topUpRequest();
    final byte[] expectedTopUp = topUpSource.clone();
    final OfflineCashToriiV1.TopUpRequestV1 topUp =
        new OfflineCashToriiV1.TopUpRequestV1(topUpSource);
    Arrays.fill(topUpSource, (byte) 0);
    assertArrayEquals(expectedTopUp, topUp.encodeCanonical());
    Arrays.fill(topUp.encodeCanonical(), (byte) 0);
    assertArrayEquals(expectedTopUp, topUp.encodeCanonical());
    assertEquals(topUp, OfflineCashToriiV1.TopUpRequestV1.decodeCanonical(expectedTopUp));

    final OfflineCashToriiV1.RedeemRequestV1 redeem =
        new OfflineCashToriiV1.RedeemRequestV1(OfflineCashToriiV1Fixtures.redeemRequest());
    assertArrayEquals(OfflineCashToriiV1Fixtures.redeemRequest(), redeem.encodeCanonical());

    assertThrows(
        IllegalArgumentException.class,
        () ->
            new OfflineCashToriiV1.TopUpRequestV1(
                OfflineCashToriiV1Fixtures.invalidBindingTopUpRequest()));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new OfflineCashToriiV1.OperationReferenceV1(
                OfflineCashToriiV1Fixtures.zeroTimeReference()));
    assertThrows(
        IllegalArgumentException.class,
        () ->
            new OfflineCashToriiV1.OperationReferenceV1(
                OfflineCashToriiV1Fixtures.invalidTransactionHashReference()));
    for (final byte[] invalidStatus :
        Arrays.asList(
            OfflineCashToriiV1Fixtures.zeroSubmittedPendingStatus(),
            OfflineCashToriiV1Fixtures.zeroHeightStatus(),
            OfflineCashToriiV1Fixtures.zeroTimeStatus(),
            OfflineCashToriiV1Fixtures.invalidTransactionHashStatus(),
            OfflineCashToriiV1Fixtures.wrongRejectionCodeStatus(),
            OfflineCashToriiV1Fixtures.rejectionDetailsStatus(),
            OfflineCashToriiV1Fixtures.oversizedRejectionMessageStatus())) {
      assertThrows(
          IllegalArgumentException.class,
          () -> new OfflineCashToriiV1.OperationStatusV1(invalidStatus));
    }
    assertEquals(
        OfflineCashToriiV1.OperationStateV1.APPLIED,
        new OfflineCashToriiV1.OperationStatusV1(
                OfflineCashToriiV1Fixtures.foreignNetworkTopUpStatus())
            .project()
            .state());

    final OfflineCashToriiV1.OperationStatusProjectionV1 pending =
        new OfflineCashToriiV1.OperationStatusV1(
                OfflineCashToriiV1Fixtures.topUpPendingStatus())
            .project();
    assertEquals(OfflineCashToriiV1.OperationStateV1.PENDING, pending.state());
    assertEquals(OfflineCashToriiV1.OperationKindV1.TOP_UP, pending.kind());
    assertNotNull(pending.submittedAtMilliseconds());
    assertTrue(pending.submittedAtMilliseconds().longValue() > 0);

    final OfflineCashToriiV1.OperationStatusProjectionV1 rejected =
        new OfflineCashToriiV1.OperationStatusV1(OfflineCashToriiV1Fixtures.rejectedStatus())
            .project();
    assertEquals(OfflineCashToriiV1.OperationStateV1.REJECTED, rejected.state());
    assertEquals(OfflineCashToriiV1.OperationKindV1.REDEEM, rejected.kind());
    assertEquals("offline_operation_rejected", rejected.rejection().code());
    assertEquals("rejected", rejected.rejection().message());

    final OfflineCashToriiV1.OperationStatusProjectionV1 appliedRedeem =
        new OfflineCashToriiV1.OperationStatusV1(
                OfflineCashToriiV1Fixtures.redeemAppliedStatus())
            .project();
    assertEquals(OfflineCashToriiV1.OperationStateV1.APPLIED, appliedRedeem.state());
    assertEquals(OfflineCashToriiV1.OperationKindV1.REDEEM, appliedRedeem.kind());
    assertEquals(Long.valueOf(9), appliedRedeem.finalizedBlockHeight());
    assertEquals(Long.valueOf(1_725_000_000_102L), appliedRedeem.serverTimeMilliseconds());
  }

  @Test
  public void commandAndPollResponsesRemainExactlyBound() {
    final String operationId = OfflineCashToriiV1Fixtures.topUpOperationId();
    final OfflineCashToriiV1.TopUpRequestV1 topUp =
        new OfflineCashToriiV1.TopUpRequestV1(OfflineCashToriiV1Fixtures.topUpRequest());
    final AtomicInteger dispatches = new AtomicInteger();
    final TransportExecutor noTransport =
        request -> {
          dispatches.incrementAndGet();
          throw new AssertionError("locally rejected request reached transport");
        };
    final OfflineCashToriiV1.ClientV1 bound = client(noTransport, signingContext());

    assertThrows(
        IllegalArgumentException.class,
        () -> bound.submitTopUp(topUp, OfflineCashToriiV1Fixtures.redeemOperationId()));
    final LocalSigningContext foreignContext =
        new LocalSigningContext(
            NetworkId.parse(
                "32c903e5b3497e34c2b844ebfe8a39c19e6cf8f95d44c1ffb8ba9dcb42f91149"));
    assertThrows(
        IllegalArgumentException.class,
        () -> client(noTransport, foreignContext).submitTopUp(topUp, operationId));
    assertEquals(0, dispatches.get());

    final List<TransportResponse> invalidAccepted =
        Arrays.asList(
            accepted(OfflineCashToriiV1Fixtures.wrongIdReference(), operationId, null, null),
            accepted(OfflineCashToriiV1Fixtures.wrongKindReference(), operationId, null, null),
            accepted(OfflineCashToriiV1Fixtures.wrongTimeReference(), operationId, null, null),
            accepted(OfflineCashToriiV1Fixtures.zeroTimeReference(), operationId, null, null),
            accepted(OfflineCashToriiV1Fixtures.wrongUriReference(), operationId, null, null),
            accepted(
                OfflineCashToriiV1Fixtures.invalidTransactionHashReference(),
                operationId,
                null,
                null),
            accepted(OfflineCashToriiV1Fixtures.topUpReference(), operationId, "missing", null),
            accepted(OfflineCashToriiV1Fixtures.topUpReference(), operationId, null, "missing"),
            accepted(
                OfflineCashToriiV1Fixtures.topUpReference(), operationId, "wrong", null),
            accepted(OfflineCashToriiV1Fixtures.topUpReference(), operationId, null, "0"),
            accepted(OfflineCashToriiV1Fixtures.topUpReference(), operationId, null, "01"),
            accepted(OfflineCashToriiV1Fixtures.topUpReference(), operationId, null, "1\u0661"),
            acceptedWithDuplicateHeader(
                OfflineCashToriiV1Fixtures.topUpReference(), operationId, "Location"),
            acceptedWithDuplicateHeader(
                OfflineCashToriiV1Fixtures.topUpReference(), operationId, "Retry-After"),
            accepted(
                OfflineCashToriiV1Fixtures.topUpReference(),
                operationId,
                null,
                "18446744073709551616"));
    for (final TransportResponse response : invalidAccepted) {
      assertFutureFailure(
          client(request -> CompletableFuture.completedFuture(response), signingContext())
              .submitTopUp(topUp, operationId));
    }

    final List<TransportRequest> requests = new ArrayList<>();
    final OfflineCashToriiV1.ClientV1 validClient =
        client(
            request -> {
              requests.add(request);
              return CompletableFuture.completedFuture(
                  accepted(
                      OfflineCashToriiV1Fixtures.topUpReference(),
                      operationId,
                      null,
                      "18446744073709551615"));
            },
            signingContext());
    final OfflineCashToriiV1.OperationReferenceV1 reference =
        validClient.submitTopUp(topUp, operationId).join();
    assertArrayEquals(OfflineCashToriiV1Fixtures.topUpReference(), reference.encodeCanonical());
    assertEquals(1, requests.size());
    assertEquals("POST", requests.get(0).method());
    assertEquals("/v1/offline/top-up", requests.get(0).uri().getPath());
    assertEquals(Arrays.asList(operationId), requests.get(0).headers().get("Idempotency-Key"));
    assertArrayEquals(OfflineCashToriiV1Fixtures.topUpRequest(), requests.get(0).body());

    for (final byte[] invalidStatus :
        Arrays.asList(
            OfflineCashToriiV1Fixtures.wrongIdStatus(),
            OfflineCashToriiV1Fixtures.zeroSubmittedPendingStatus(),
            OfflineCashToriiV1Fixtures.zeroHeightStatus(),
            OfflineCashToriiV1Fixtures.zeroTimeStatus(),
            OfflineCashToriiV1Fixtures.invalidTransactionHashStatus(),
            OfflineCashToriiV1Fixtures.foreignNetworkTopUpStatus(),
            OfflineCashToriiV1Fixtures.wrongRejectionCodeStatus(),
            OfflineCashToriiV1Fixtures.rejectionDetailsStatus(),
            OfflineCashToriiV1Fixtures.oversizedRejectionMessageStatus())) {
      assertFutureFailure(
          client(
                  request ->
                      CompletableFuture.completedFuture(
                          response(
                              200,
                              OfflineCashToriiV1.ClientV1.NORITO_MEDIA_TYPE,
                              invalidStatus)),
                  signingContext())
              .getOperation(operationId));
    }
  }

  @Test
  public void appliedTopUpProjectionValidatesEveryTerminalEvidenceBinding() {
    final OfflineCashToriiV1.OperationStatusProjectionV1 projection =
        new OfflineCashToriiV1.OperationStatusV1(
                OfflineCashToriiV1Fixtures.topUpAppliedStatus())
            .project();
    assertEquals(OfflineCashToriiV1.OperationStateV1.APPLIED, projection.state());
    assertEquals(OfflineCashToriiV1.OperationKindV1.TOP_UP, projection.kind());
    assertArrayEquals(hex(OfflineCashToriiV1Fixtures.topUpOperationId()), projection.operationId());
    assertEquals(
        Long.valueOf(OfflineCashToriiV1Fixtures.topUpFinalizedBlockHeight()),
        projection.finalizedBlockHeight());
    assertEquals(
        Long.valueOf(OfflineCashToriiV1Fixtures.topUpServerTimeMilliseconds()),
        projection.serverTimeMilliseconds());
    final OfflineCashToriiV1.FinalizedTopUpV1 finalized = projection.finalizedTopUp();
    assertNotNull(finalized);
    assertEquals(projection.finalizedBlockHeight().longValue(), finalized.finalizedBlockHeight());
    assertEquals(
        projection.serverTimeMilliseconds().longValue(), finalized.serverTimeMilliseconds());
    assertTrue(finalized.anchorCanonical().length > 0);
    assertTrue(finalized.finalityProofCanonical().length > 0);

    for (final byte[] invalidStatus :
        Arrays.asList(
            OfflineCashToriiV1Fixtures.invalidTopUpAnchorStatus(),
            OfflineCashToriiV1Fixtures.invalidTopUpProofStatus(),
            OfflineCashToriiV1Fixtures.wrongTopUpOperationStatus(),
            OfflineCashToriiV1Fixtures.wrongTopUpTransactionStatus(),
            OfflineCashToriiV1Fixtures.wrongTopUpHeightStatus(),
            OfflineCashToriiV1Fixtures.wrongTopUpProofNetworkStatus(),
            OfflineCashToriiV1Fixtures.wrongTopUpProofAnchorStatus(),
            OfflineCashToriiV1Fixtures.wrongTopUpProofHeightStatus())) {
      assertThrows(
          IllegalArgumentException.class,
          () -> new OfflineCashToriiV1.OperationStatusV1(invalidStatus));
    }
  }

  private static OfflineCashToriiV1.ClientV1 client(
      final TransportExecutor transport, final LocalSigningContext signingContext) {
    return OfflineCashToriiV1.ClientV1.create(
        URI.create("https://torii.example"), transport, signingContext);
  }

  private static LocalSigningContext signingContext() {
    return new LocalSigningContext(NetworkId.parse(OfflineCashToriiV1Fixtures.networkId()));
  }

  private static TransportResponse accepted(
      final byte[] body,
      final String operationId,
      final String locationOverride,
      final String retryAfterOverride) {
    final TransportResponse.Builder builder =
        TransportResponse.builder()
            .setStatusCode(202)
            .setBody(body)
            .addHeader("content-type", OfflineCashToriiV1.ClientV1.NORITO_MEDIA_TYPE);
    if (!"missing".equals(locationOverride)) {
      builder.addHeader(
          "location",
          locationOverride == null
              ? OfflineCashToriiV1.ClientV1.OPERATIONS_PATH + "/" + operationId
              : "/v1/offline/operations/" + OfflineCashToriiV1Fixtures.redeemOperationId());
    }
    if (!"missing".equals(retryAfterOverride)) {
      builder.addHeader("retry-after", retryAfterOverride == null ? "1" : retryAfterOverride);
    }
    return builder.build();
  }

  private static TransportResponse acceptedWithDuplicateHeader(
      final byte[] body, final String operationId, final String duplicateHeader) {
    final String location = OfflineCashToriiV1.ClientV1.OPERATIONS_PATH + "/" + operationId;
    final TransportResponse.Builder builder =
        TransportResponse.builder()
            .setStatusCode(202)
            .setBody(body)
            .addHeader("content-type", OfflineCashToriiV1.ClientV1.NORITO_MEDIA_TYPE)
            .addHeader("location", location)
            .addHeader("retry-after", "1");
    if ("Location".equals(duplicateHeader)) {
      builder.addHeader("Location", location);
    } else if ("Retry-After".equals(duplicateHeader)) {
      builder.addHeader("Retry-After", "1");
    } else {
      throw new IllegalArgumentException("unsupported duplicate header fixture");
    }
    return builder.build();
  }

  private static TransportResponse response(
      final int status, final String mediaType, final byte[] body) {
    return TransportResponse.builder()
        .setStatusCode(status)
        .setBody(body)
        .addHeader("Content-Type", mediaType)
        .build();
  }

  private static void assertFutureFailure(final CompletableFuture<?> future) {
    assertThrows(CompletionException.class, future::join);
  }

  private static String fixtureValue(final List<String> rows, final String name) {
    final String prefix = name + "=";
    for (final String row : rows) {
      if (row.startsWith(prefix)) return row.substring(prefix.length());
    }
    throw new AssertionError("missing fixture row " + name);
  }

  private static List<String> replaceFixtureRow(
      final List<String> rows, final String name, final String value) {
    final String prefix = name + "=";
    final List<String> replaced = new ArrayList<>(rows.size());
    for (final String row : rows) {
      replaced.add(row.startsWith(prefix) ? prefix + value : row);
    }
    return replaced;
  }

  private static List<String> withExtraRow(final List<String> rows, final String extra) {
    final List<String> extended = new ArrayList<>(rows);
    extended.add(extra);
    return extended;
  }

  private static byte[] filled(final int size, final byte value) {
    final byte[] result = new byte[size];
    Arrays.fill(result, value);
    return result;
  }

  private static byte[] hex(final String value) {
    final byte[] result = new byte[value.length() / 2];
    for (int index = 0; index < result.length; index++) {
      result[index] =
          (byte)
              ((Character.digit(value.charAt(index * 2), 16) << 4)
                  | Character.digit(value.charAt(index * 2 + 1), 16));
    }
    return result;
  }
}

package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.offline.OfflineToriiException;
import org.hyperledger.iroha.android.offline.OfflineOperationCodec;
import org.hyperledger.iroha.android.offline.OfflineOperationKind;
import org.hyperledger.iroha.android.offline.OfflineOperationReference;
import org.hyperledger.iroha.android.offline.OfflineOperationState;
import org.hyperledger.iroha.android.offline.OfflineOperationStatus;
import org.hyperledger.iroha.android.offline.OfflineReadiness;
import org.hyperledger.iroha.android.offline.OfflineRedeemRequest;
import org.hyperledger.iroha.android.offline.OfflineTopUpRequest;
import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;

public final class OfflineToriiClientTests {

  private OfflineToriiClientTests() {}

  public static void main(final String[] args) {
    operationReferenceMatchesRustGoldenArchive();
    operationStatusesMatchRustGoldenArchives();
    typedOperationStatusesRoundTrip();
    readinessUsesCanonicalGetPathAndParsesResponse();
    operationsUseCanonicalPathsAndNoritoBodies();
    requestsDeriveAndValidateCanonicalOperationIds();
    propagatesNon2xxResponses();
    propagatesRejectCodeFromNon2xxResponses();
    rejectsInsecureAuthorizationHeader();
    System.out.println("[IrohaAndroid] OfflineToriiClientTests passed.");
  }

  private static void operationReferenceMatchesRustGoldenArchive() {
    final byte[] archive = hexBytes(RUST_OPERATION_REFERENCE_HEX);
    final OfflineOperationReference decoded = OfflineOperationCodec.decodeReference(archive);
    final String operationId = repeat("1", 64);

    assert operationId.equals(decoded.operationId()) : "golden operation_id mismatch";
    assert decoded.kind() == OfflineOperationKind.TOP_UP : "golden kind mismatch";
    assert decoded.state() == OfflineOperationState.PENDING : "golden state mismatch";
    assert "transaction-hash".equals(decoded.transactionHash())
        : "golden transaction_hash mismatch";
    assert ("/v1/offline/operations/" + operationId).equals(decoded.statusUri())
        : "golden status_uri mismatch";
    assert new BigInteger("18446744073709551615").equals(decoded.submittedAtMs())
        : "golden submitted_at_ms mismatch";
    assert java.util.Arrays.equals(archive, OfflineOperationCodec.encodeReference(decoded))
        : "Java encoding must reproduce the Rust golden archive";
  }

  private static void operationStatusesMatchRustGoldenArchives() {
    final String operationId = repeat("1", 64);

    final byte[] pendingArchive = hexBytes(RUST_PENDING_STATUS_HEX);
    final OfflineOperationStatus.Pending pending =
        (OfflineOperationStatus.Pending) OfflineOperationCodec.decodeStatus(pendingArchive);
    assert operationId.equals(pending.operationId()) : "pending golden operation_id mismatch";
    assert pending.kind() == OfflineOperationKind.TOP_UP : "pending golden kind mismatch";
    assert "transaction-hash".equals(pending.transactionHash())
        : "pending golden transaction_hash mismatch";
    assert new BigInteger("18446744073709551615").equals(pending.submittedAtMs())
        : "pending golden submitted_at_ms mismatch";
    assert java.util.Arrays.equals(pendingArchive, OfflineOperationCodec.encodeStatus(pending))
        : "pending status must re-encode to the Rust golden";
    final byte[] wrongSchema = java.util.Arrays.copyOf(pendingArchive, pendingArchive.length);
    wrongSchema[6] ^= 1;
    try {
      OfflineOperationCodec.decodeStatus(wrongSchema);
      throw new AssertionError("status decoder must reject a foreign schema hash");
    } catch (final IllegalArgumentException expected) {
      // Expected schema-bound rejection.
    }

    final byte[] rejectedArchive = hexBytes(RUST_REJECTED_STATUS_HEX);
    final OfflineOperationStatus.Rejected rejected =
        (OfflineOperationStatus.Rejected) OfflineOperationCodec.decodeStatus(rejectedArchive);
    assert operationId.equals(rejected.operationId()) : "rejected golden operation_id mismatch";
    assert rejected.kind() == OfflineOperationKind.REDEEM : "rejected golden kind mismatch";
    assert "offline_operation_rejected".equals(rejected.error().code())
        : "rejected golden error code mismatch";
    assert "rejected".equals(rejected.error().message())
        : "rejected golden error message mismatch";
    assert rejected.error().details() == null : "rejected golden details mismatch";
    assert java.util.Arrays.equals(rejectedArchive, OfflineOperationCodec.encodeStatus(rejected))
        : "rejected status must re-encode to the Rust golden";

    final byte[] appliedArchive = hexBytes(RUST_APPLIED_REDEEM_STATUS_HEX);
    final OfflineOperationStatus.Applied applied =
        (OfflineOperationStatus.Applied) OfflineOperationCodec.decodeStatus(appliedArchive);
    assert operationId.equals(applied.operationId()) : "applied golden operation_id mismatch";
    final OfflineOperationStatus.RedeemResult result =
        ((OfflineOperationStatus.Result.Redeem) applied.result()).value();
    assert "transaction-hash".equals(result.transactionHash())
        : "applied golden transaction_hash mismatch";
    assert new BigInteger("18446744073709551615").equals(result.finalizedBlockHeight())
        : "applied golden finalized height mismatch";
    assert BigInteger.valueOf(42).equals(result.serverTimeMs())
        : "applied golden server time mismatch";
    assert java.util.Arrays.equals(appliedArchive, OfflineOperationCodec.encodeStatus(applied))
        : "applied status must re-encode to the Rust golden";
  }

  private static void typedOperationStatusesRoundTrip() {
    final String operationId = repeat("11", 32);
    final OfflineOperationStatus.Pending pending =
        new OfflineOperationStatus.Pending(
            operationId,
            OfflineOperationKind.TOP_UP,
            "transaction-hash",
            new BigInteger("18446744073709551615"));
    final OfflineOperationStatus.Pending decodedPending =
        (OfflineOperationStatus.Pending)
            OfflineOperationCodec.decodeStatus(OfflineOperationCodec.encodeStatus(pending));
    assert operationId.equals(decodedPending.operationId()) : "pending operation_id mismatch";
    assert decodedPending.kind() == OfflineOperationKind.TOP_UP : "pending kind mismatch";
    assert new BigInteger("18446744073709551615").equals(decodedPending.submittedAtMs())
        : "pending submitted_at_ms mismatch";

    final OfflineOperationStatus.Applied applied =
        new OfflineOperationStatus.Applied(
            operationId,
            new OfflineOperationStatus.Result.Redeem(
                new OfflineOperationStatus.RedeemResult(
                    "transaction-hash",
                    new BigInteger("18446744073709551615"),
                    BigInteger.valueOf(42))));
    final OfflineOperationStatus.Applied decodedApplied =
        (OfflineOperationStatus.Applied)
            OfflineOperationCodec.decodeStatus(OfflineOperationCodec.encodeStatus(applied));
    final OfflineOperationStatus.RedeemResult redeemResult =
        ((OfflineOperationStatus.Result.Redeem) decodedApplied.result()).value();
    assert new BigInteger("18446744073709551615")
        .equals(redeemResult.finalizedBlockHeight()) : "applied height mismatch";
    assert BigInteger.valueOf(42).equals(redeemResult.serverTimeMs())
        : "applied server_time_ms mismatch";

    final OfflineOperationStatus.ErrorDetails details =
        new OfflineOperationStatus.ErrorDetails(
            "torii",
            "policy_rejected",
            new OfflineOperationStatus.QueueErrorSnapshot(
                "saturated", BigInteger.TEN, BigInteger.TEN, true),
            BigInteger.valueOf(5),
            "/v1/offline/redeem",
            "proof",
            "valid",
            "invalid",
            "taira",
            Integer.valueOf(369),
            "transaction-hash",
            "rejected",
            "refresh proof",
            new OfflineOperationStatus.AxtErrorDetails(
                "axt_rejected",
                "policy",
                BigInteger.ONE,
                BigInteger.valueOf(2),
                Long.valueOf(3),
                BigInteger.valueOf(4),
                BigInteger.valueOf(5)));
    final OfflineOperationStatus.Rejected rejected =
        new OfflineOperationStatus.Rejected(
            operationId,
            OfflineOperationKind.REDEEM,
            "transaction-hash",
            new OfflineOperationStatus.Error("rejected", "Transaction rejected", details));
    final OfflineOperationStatus.Rejected decodedRejected =
        (OfflineOperationStatus.Rejected)
            OfflineOperationCodec.decodeStatus(OfflineOperationCodec.encodeStatus(rejected));
    assert operationId.equals(decodedRejected.operationId()) : "rejected operation_id mismatch";
    assert decodedRejected.kind() == OfflineOperationKind.REDEEM : "rejected kind mismatch";
    assert "rejected".equals(decodedRejected.error().code()) : "rejected error code mismatch";
    assert "Transaction rejected".equals(decodedRejected.error().message())
        : "rejected error message mismatch";
    assert "policy_rejected".equals(decodedRejected.error().details().rejectCode)
        : "rejected details mismatch";
    assert BigInteger.TEN.equals(decodedRejected.error().details().queue.capacity)
        : "rejected queue mismatch";
    assert Long.valueOf(3).equals(decodedRejected.error().details().axt.lane)
        : "rejected AXT mismatch";
  }

  private static void readinessUsesCanonicalGetPathAndParsesResponse() {
    final StubExecutor executor =
        new StubExecutor(
            200,
            """
            {
              "asset_definition_id": "xor#wonderland",
              "evaluated_block_height": 18446744073709551615,
              "ready": false,
              "blockers": [
                {"code": "offline_disabled", "message": "Offline transfers are disabled"}
              ]
            }
            """);
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .timeout(Duration.ofSeconds(5))
            .addHeader("X-Test", "1")
            .build();

    final OfflineReadiness readiness = client.getOfflineReadiness("xor#wonderland").join();

    assert "GET".equals(executor.lastRequest.method()) : "readiness must use GET";
    assert executor.lastRequest.uri().getPath().endsWith("/v1/offline/readiness")
        : "readiness path mismatch";
    assert "asset_definition_id=xor%23wonderland"
        .equals(executor.lastRequest.uri().getRawQuery()) : "readiness query mismatch";
    assert "application/json".equals(firstHeader(executor.lastRequest, "Accept"))
        : "accept header mismatch";
    assert "xor#wonderland".equals(readiness.assetDefinitionId())
        : "asset_definition_id mismatch";
    assert new BigInteger("18446744073709551615").equals(readiness.evaluatedBlockHeight())
        : "evaluated_block_height mismatch";
    assert !readiness.ready() : "ready mismatch";
    assert readiness.blockers().size() == 1 : "blockers mismatch";
    assert "offline_disabled".equals(readiness.blockers().get(0).code())
        : "blocker code mismatch";
  }

  private static void operationsUseCanonicalPathsAndNoritoBodies() {
    final String operationId = repeat("11", 32);
    final byte[] operationIdBytes = new byte[32];
    java.util.Arrays.fill(operationIdBytes, (byte) 0x11);
    final OfflineOperationReference topUpReference =
        new OfflineOperationReference(
            operationId,
            OfflineOperationKind.TOP_UP,
            OfflineOperationState.PENDING,
            "transaction-hash",
            "/v1/offline/operations/" + operationId,
            new BigInteger("18446744073709551615"));
    final byte[] responseArchive = OfflineOperationCodec.encodeReference(topUpReference);
    final byte[] topUpArchive = topUpRequestArchive(operationIdBytes);
    final byte[] expectedTopUpArchive = java.util.Arrays.copyOf(topUpArchive, topUpArchive.length);
    final StubExecutor executor = new StubExecutor(202, responseArchive);
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();

    final OfflineOperationReference accepted =
        client.submitTopUp(new OfflineTopUpRequest(topUpArchive)).join();
    java.util.Arrays.fill(topUpArchive, (byte) 0);
    assert topUpReference.equals(accepted) : "top-up reference mismatch";
    assert "POST".equals(executor.lastRequest.method()) : "top-up must use POST";
    assert "/v1/offline/top-up".equals(executor.lastRequest.uri().getPath())
        : "top-up path mismatch";
    assert java.util.Arrays.equals(expectedTopUpArchive, executor.lastRequest.body())
        : "top-up body must be direct Norito";
    assert "application/x-norito".equals(firstHeader(executor.lastRequest, "Content-Type"))
        : "top-up content type mismatch";
    assert operationId.equals(firstHeader(executor.lastRequest, "Idempotency-Key"))
        : "top-up idempotency key mismatch";

    final OfflineOperationReference redeemReference =
        new OfflineOperationReference(
            operationId,
            OfflineOperationKind.REDEEM,
            OfflineOperationState.PENDING,
            "transaction-hash",
            "/v1/offline/operations/" + operationId,
            new BigInteger("18446744073709551615"));
    executor.body = OfflineOperationCodec.encodeReference(redeemReference);
    final byte[] redeemArchive = redeemRequestArchive(operationIdBytes);
    assert redeemReference.equals(
        client.submitRedeem(new OfflineRedeemRequest(redeemArchive)).join())
        : "redeem reference mismatch";
    assert "/v1/offline/redeem".equals(executor.lastRequest.uri().getPath())
        : "redeem path mismatch";
    assert java.util.Arrays.equals(redeemArchive, executor.lastRequest.body())
        : "redeem body must be direct canonical Norito";

    executor.status = 200;
    executor.body =
        OfflineOperationCodec.encodeStatus(
            new OfflineOperationStatus.Pending(
                operationId,
                OfflineOperationKind.TOP_UP,
                "transaction-hash",
                new BigInteger("18446744073709551615")));
    final OfflineOperationStatus status = client.getOperationStatus(operationId).join();
    assert operationId.equals(status.operationId()) : "status operation_id mismatch";
    assert ((OfflineOperationStatus.Pending) status).kind() == OfflineOperationKind.TOP_UP
        : "status kind mismatch";
    assert "GET".equals(executor.lastRequest.method()) : "status must use GET";
    assert ("/v1/offline/operations/" + operationId)
        .equals(executor.lastRequest.uri().getPath()) : "status path mismatch";
  }

  private static void requestsDeriveAndValidateCanonicalOperationIds() {
    final byte[] operationId = new byte[32];
    for (int index = 0; index < operationId.length; index++) {
      operationId[index] = (byte) (index + 1);
    }
    operationId[0] = (byte) 0xAB;
    operationId[1] = (byte) 0xCD;

    final byte[] topUpArchive = topUpRequestArchive(operationId);
    final byte[] expectedArchive = java.util.Arrays.copyOf(topUpArchive, topUpArchive.length);
    final OfflineTopUpRequest topUp = new OfflineTopUpRequest(topUpArchive);
    assert lowercaseHex(operationId).equals(topUp.operationId())
        : "top-up operation_id must be derived as lowercase hex";
    java.util.Arrays.fill(topUpArchive, (byte) 0);
    assert java.util.Arrays.equals(expectedArchive, topUp.noritoArchive())
        : "top-up request must defensively copy its input archive";
    final byte[] returnedArchive = topUp.noritoArchive();
    java.util.Arrays.fill(returnedArchive, (byte) 0);
    assert java.util.Arrays.equals(expectedArchive, topUp.noritoArchive())
        : "top-up request must defensively copy its returned archive";

    final OfflineRedeemRequest redeem = new OfflineRedeemRequest(redeemRequestArchive(operationId));
    assert lowercaseHex(operationId).equals(redeem.operationId())
        : "redeem operation_id must be derived as lowercase hex";

    assertRejects(
        () -> new OfflineTopUpRequest(redeemRequestArchive(operationId)),
        "top-up must reject a redeem schema");
    assertRejects(
        () -> new OfflineRedeemRequest(redeemRequestArchive(new byte[32])),
        "redeem must reject a zero operation_id");
    assertRejects(
        () -> new OfflineTopUpRequest(topUpRequestArchive(new byte[31])),
        "top-up must reject a non-32-byte operation_id");
    assertRejects(
        () ->
            new OfflineTopUpRequest(
                canonicalRequestArchive(
                    TOP_UP_REQUEST_SCHEMA, 9, 6, operationId, new byte[0])),
        "top-up must reject trailing fields");
    assertRejects(
        () ->
            new OfflineTopUpRequest(
                canonicalRequestArchive(
                    TOP_UP_REQUEST_SCHEMA, 8, 6, operationId, new byte[] {0x7F})),
        "top-up must reject trailing bytes");
    assertRejects(
        () ->
            new OfflineRedeemRequest(
                canonicalRequestArchive(
                    REDEEM_REQUEST_SCHEMA, 10, 9, operationId, new byte[0])),
        "redeem must reject a missing field");
    assertRejects(
        () ->
            new OfflineTopUpRequest(
                canonicalRequestArchive(
                    TOP_UP_REQUEST_SCHEMA, 8, 6, operationId, new byte[0], 0)),
        "top-up must reject non-canonical fixed-width field framing");
    assertRejects(
        () -> new OfflineTopUpRequest(withHeaderPadding(topUpRequestArchive(operationId))),
        "top-up must reject header padding");
  }

  private static void propagatesNon2xxResponses() {
    final StubExecutor executor = new StubExecutor(500, "{\"error\":\"boom\"}");
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    try {
      client.getOfflineReadiness("xor#wonderland").join();
    } catch (final CompletionException ex) {
      assert ex.getCause() instanceof OfflineToriiException : "expected OfflineToriiException";
      assert ex.getCause().getMessage().contains("500") : "status missing from message";
      assert ex.getCause().getMessage().contains("boom") : "body missing from message";
      final OfflineToriiException error = (OfflineToriiException) ex.getCause();
      assert Integer.valueOf(500).equals(error.statusCode().orElse(null))
          : "status code not surfaced";
      assert error.responseBody().orElse("").contains("boom")
          : "response body not surfaced";
      assert error.rejectCode().isEmpty() : "unexpected reject code";
      return;
    }
    throw new AssertionError("Expected CompletionException for non-2xx responses");
  }

  private static void propagatesRejectCodeFromNon2xxResponses() {
    final StubExecutor executor =
        new StubExecutor(
            400,
            "{\"error\":\"not ready\"}",
            "Bad Request",
            Map.of("X-IrOhA-ReJeCt-CoDe", List.of("offline_unavailable")));
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(executor)
            .baseUri(URI.create("https://example.com"))
            .build();
    try {
      client.getOfflineReadiness("xor#wonderland").join();
    } catch (final CompletionException ex) {
      assert ex.getCause() instanceof OfflineToriiException : "expected OfflineToriiException";
      final OfflineToriiException error = (OfflineToriiException) ex.getCause();
      assert Integer.valueOf(400).equals(error.statusCode().orElse(null))
          : "status code not surfaced";
      assert "offline_unavailable".equals(error.rejectCode().orElse(null))
          : "reject code not surfaced";
      assert error.getMessage().contains("reject_code=offline_unavailable")
          : "reject code missing from message";
      return;
    }
    throw new AssertionError("Expected CompletionException for reject code propagation");
  }

  private static void rejectsInsecureAuthorizationHeader() {
    final OfflineToriiClient client =
        OfflineToriiClient.builder()
            .executor(new StubExecutor(200, "{}"))
            .baseUri(URI.create("http://example.com"))
            .addHeader("Authorization", "Bearer secret")
            .build();
    try {
      client.getOfflineReadiness("xor#wonderland");
    } catch (final IllegalArgumentException ex) {
      assert ex.getMessage().contains("insecure transport over http")
          : "security message mismatch";
      return;
    }
    throw new AssertionError("Expected insecure credentialed HTTP request to fail");
  }

  private static final class StubExecutor implements HttpTransportExecutor {
    private int status;
    private byte[] body;
    private final String message;
    private final Map<String, List<String>> headers;
    private TransportRequest lastRequest;
    private String lastBody = "";

    private StubExecutor(final int status, final String body) {
      this(status, body, "", Map.of());
    }

    private StubExecutor(final int status, final byte[] body) {
      this.status = status;
      this.body = java.util.Arrays.copyOf(body, body.length);
      this.message = "";
      this.headers = Map.of();
    }

    private StubExecutor(
        final int status,
        final String body,
        final String message,
        final Map<String, List<String>> headers) {
      this.status = status;
      this.body = body.getBytes(StandardCharsets.UTF_8);
      this.message = message;
      this.headers = headers;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      this.lastRequest = request;
      this.lastBody = new String(request.body(), StandardCharsets.UTF_8);
      return CompletableFuture.completedFuture(
          new TransportResponse(status, body, message, headers));
    }
  }

  private static String firstHeader(final TransportRequest request, final String name) {
    for (final var entry : request.headers().entrySet()) {
      if (entry.getKey().equalsIgnoreCase(name)) {
        final List<String> values = entry.getValue();
        if (!values.isEmpty()) {
          return values.get(0);
        }
      }
    }
    return "";
  }

  private static byte[] topUpRequestArchive(final byte[] operationId) {
    return canonicalRequestArchive(
        TOP_UP_REQUEST_SCHEMA, 8, 6, operationId, new byte[0]);
  }

  private static byte[] redeemRequestArchive(final byte[] operationId) {
    return canonicalRequestArchive(
        REDEEM_REQUEST_SCHEMA, 11, 9, operationId, new byte[0]);
  }

  private static byte[] canonicalRequestArchive(
      final String schema,
      final int fieldCount,
      final int operationIdFieldIndex,
      final byte[] operationId,
      final byte[] trailingBytes) {
    return canonicalRequestArchive(
        schema,
        fieldCount,
        operationIdFieldIndex,
        operationId,
        trailingBytes,
        NoritoHeader.COMPACT_LEN);
  }

  private static byte[] canonicalRequestArchive(
      final String schema,
      final int fieldCount,
      final int operationIdFieldIndex,
      final byte[] operationId,
      final byte[] trailingBytes,
      final int flags) {
    final NoritoEncoder encoder = new NoritoEncoder(flags);
    for (int fieldIndex = 0; fieldIndex < fieldCount; fieldIndex++) {
      final byte[] field =
          fieldIndex == operationIdFieldIndex
              ? java.util.Arrays.copyOf(operationId, operationId.length)
              : new byte[] {(byte) (fieldIndex + 1)};
      encoder.writeLength(field.length, (flags & NoritoHeader.COMPACT_LEN) != 0);
      encoder.writeBytes(field);
    }
    final byte[] encodedFields = encoder.toByteArray();
    final byte[] payload =
        java.util.Arrays.copyOf(
            encodedFields, encodedFields.length + trailingBytes.length);
    System.arraycopy(
        trailingBytes, 0, payload, encodedFields.length, trailingBytes.length);
    final NoritoHeader header =
        new NoritoHeader(
            SchemaHash.hash16(schema),
            payload.length,
            CRC64.compute(payload),
            flags,
            NoritoHeader.COMPRESSION_NONE);
    final byte[] headerBytes = header.encode();
    final byte[] archive =
        java.util.Arrays.copyOf(headerBytes, headerBytes.length + payload.length);
    System.arraycopy(payload, 0, archive, headerBytes.length, payload.length);
    return archive;
  }

  private static byte[] withHeaderPadding(final byte[] archive) {
    final byte[] padded = new byte[archive.length + 1];
    System.arraycopy(archive, 0, padded, 0, NoritoHeader.HEADER_LENGTH);
    System.arraycopy(
        archive,
        NoritoHeader.HEADER_LENGTH,
        padded,
        NoritoHeader.HEADER_LENGTH + 1,
        archive.length - NoritoHeader.HEADER_LENGTH);
    return padded;
  }

  private static String lowercaseHex(final byte[] value) {
    final char[] digits = "0123456789abcdef".toCharArray();
    final char[] result = new char[value.length * 2];
    for (int index = 0; index < value.length; index++) {
      final int unsigned = value[index] & 0xFF;
      result[index * 2] = digits[unsigned >>> 4];
      result[index * 2 + 1] = digits[unsigned & 0x0F];
    }
    return new String(result);
  }

  private static void assertRejects(final Runnable action, final String message) {
    try {
      action.run();
    } catch (final IllegalArgumentException expected) {
      return;
    }
    throw new AssertionError(message);
  }

  private static String repeat(final String value, final int count) {
    final StringBuilder result = new StringBuilder(value.length() * count);
    for (int index = 0; index < count; index++) {
      result.append(value);
    }
    return result.toString();
  }

  private static byte[] hexBytes(final String value) {
    if ((value.length() & 1) != 0) {
      throw new IllegalArgumentException("hex must contain an even number of characters");
    }
    final byte[] result = new byte[value.length() / 2];
    for (int index = 0; index < result.length; index++) {
      final int offset = index * 2;
      result[index] = (byte) Integer.parseInt(value.substring(offset, offset + 2), 16);
    }
    return result;
  }

  private static final String RUST_OPERATION_REFERENCE_HEX =
      "4e5254300000e8e2244e45e4be2a975e34957141128b00c000000000000000fe"
          + "8a8b6e958d244702414031313131313131313131313131313131313131313131"
          + "3131313131313131313131313131313131313131313131313131313131313131"
          + "313131313131313131310400000000040000000011107472616e73616374696f"
          + "6e2d6861736858572f76312f6f66666c696e652f6f7065726174696f6e732f31"
          + "3131313131313131313131313131313131313131313131313131313131313131"
          + "3131313131313131313131313131313131313131313131313131313131313108"
          + "ffffffffffffffff";

  private static final String TOP_UP_REQUEST_SCHEMA =
      "iroha_data_model::offline::model::KagemushaRecursiveSpendTopUpRequestV2";

  private static final String REDEEM_REQUEST_SCHEMA =
      "iroha_data_model::offline::model::KagemushaRecursiveSpendRedeemRequestV2";

  private static final String RUST_PENDING_STATUS_HEX =
      "4e5254300000fb04214104df1bdcd39249bddd4db23a006600000000000000b3fae818809b7b8e02000000000000000000000000414031313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131040000000011107472616e73616374696f6e2d6861736808ffffffffffffffff";

  private static final String RUST_REJECTED_STATUS_HEX =
      "4e5254300000fb04214104df1bdcd39249bddd4db23a0086000000000000008878a32fe86d887302000000000000000002000000414031313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131040100000011107472616e73616374696f6e2d68617368281b1a6f66666c696e655f6f7065726174696f6e5f72656a6563746564090872656a65637465640100";

  private static final String RUST_APPLIED_REDEEM_STATUS_HEX =
      "4e5254300000fb04214104df1bdcd39249bddd4db23a007000000000000000451e52608aefd9710200000000000000000100000041403131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313131313129010000002411107472616e73616374696f6e2d6861736808ffffffffffffffff082a00000000000000";
}

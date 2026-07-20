package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.Arrays;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.atomic.AtomicInteger;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.norito.NoritoJavaCodecAdapter;
import org.hyperledger.iroha.android.testing.TestAccountIds;
import org.hyperledger.iroha.android.tx.SignedTransactionHasher;
import org.hyperledger.iroha.android.tx.SignedTransaction;

public final class HttpClientTransportStatusTests {

  private HttpClientTransportStatusTests() {}

  public static void main(final String[] args) {
    waitForTransactionStatusWaitsThroughCommitUntilApplied();
    waitForTransactionStatusTreatsNotFoundAsPending();
    waitForTransactionStatusIgnoresNoritoBodyOnNotFound();
    waitForTransactionStatusThrowsOnFailure();
    waitForTransactionStatusFailureIncludesRejectionReason();
    waitForTransactionStatusFailureUsesCurrentDiagnosticReason();
    waitForTransactionStatusSurfacesUnexpectedHttpStatusDetails();
    waitForTransactionStatusUsesCompactJsonMessage();
    waitForTransactionStatusUsesNestedJsonErrorMessage();
    waitForTransactionStatusUsesErrorsArrayMessage();
    waitForTransactionStatusTruncatesOversizedErrorBody();
    waitForTransactionStatusMatchesSharedErrorMessageContractFixture();
    waitForTransactionStatusHonoursMaxAttempts();
    waitForTransactionStatusSaturatesOverflowingDeadline();
    waitForTransactionStatusFailsOnInvalidPayload();
    waitForTransactionStatusRejectsNonAuthoritativeTerminalPayloads();
    waitForTransactionStatusAcceptsQueuedHttpResponseCode();
    waitForTransactionStatusRejectsNoritoWhenJsonWasRequested();
    waitForTransactionStatusRejectsNonCanonicalRequestHashes();
    submitTransactionProvidesCanonicalHashForPolling();
    submitTransactionPrefersAuthoritativeReceiptHashHeaderForPolling();
    System.out.println("[IrohaAndroid] HTTP client status tests passed.");
  }

  private static void waitForTransactionStatusWaitsThroughCommitUntilApplied() {
    final String hash = canonicalHash("deadbeef");
    final SequencedExecutor executor = new SequencedExecutor(
        newResponse(200, statusPayload(hash, "Queued")),
        newResponse(200, statusPayload(hash, "Committed")),
        newResponse(200, statusPayload(hash, "Applied")));
    final HttpClientTransport transport = HttpClientTransport.withExecutor(
        executor,
        ClientConfig.builder()
            .setBaseUri(URI.create("http://localhost:8080"))
            .setRequestTimeout(Duration.ofSeconds(5))
            .build());

    final StringBuilder observed = new StringBuilder();
    final PipelineStatusOptions options =
        PipelineStatusOptions.builder()
            .intervalMillis(0L)
            .observer((status, payload, attempt) -> {
              observed.append(status).append("@").append(attempt).append(";");
            })
            .build();

    final Map<String, Object> result =
        transport.waitForTransactionStatus(hash, options).join();

    assert "Applied".equals(
            PipelineStatusExtractor.extractStatusKind(result).orElse(null))
        : "Expected Applied status";
    assert observed.toString().contains("Queued@1")
        && observed.toString().contains("Committed@2")
        && observed.toString().contains("Applied@3")
        : "Observer should capture pending, committed, and Applied statuses";
  }

  private static void waitForTransactionStatusTreatsNotFoundAsPending() {
    final String hash = canonicalHash("deadbeef");
    final SequencedExecutor executor = new SequencedExecutor(
        newResponse(404, new byte[0]),
        newResponse(200, statusPayload(hash, "Committed")),
        newResponse(200, statusPayload(hash, "Applied")));
    final HttpClientTransport transport = HttpClientTransport.withExecutor(
        executor,
        ClientConfig.builder()
            .setBaseUri(URI.create("http://localhost:8080"))
            .setRequestTimeout(Duration.ofSeconds(5))
            .build());

    final Map<String, Object> result =
        transport
            .waitForTransactionStatus(
                hash, PipelineStatusOptions.builder().intervalMillis(0L).build())
            .join();

    assert "Applied".equals(
            PipelineStatusExtractor.extractStatusKind(result).orElse(null))
        : "Expected Applied status after 404 retry";
  }

  private static void waitForTransactionStatusIgnoresNoritoBodyOnNotFound() {
    final String hash = canonicalHash("deadbeef");
    final byte[] noritoBody = new byte[] {'N', 'R', 'T', '0', 0x01};
    final SequencedExecutor executor = new SequencedExecutor(
        newResponse(404, noritoBody),
        newResponse(200, statusPayload(hash, "Committed")),
        newResponse(200, statusPayload(hash, "Applied")));
    final HttpClientTransport transport = HttpClientTransport.withExecutor(
        executor,
        ClientConfig.builder()
            .setBaseUri(URI.create("http://localhost:8080"))
            .setRequestTimeout(Duration.ofSeconds(5))
            .build());

    final Map<String, Object> result =
        transport
            .waitForTransactionStatus(
                hash, PipelineStatusOptions.builder().intervalMillis(0L).build())
            .join();

    assert "Applied".equals(
            PipelineStatusExtractor.extractStatusKind(result).orElse(null))
        : "Expected Applied status after Norito 404 retry";
  }

  private static void waitForTransactionStatusThrowsOnFailure() {
    final String hash = canonicalHash("cafebabe");
    final HttpClientTransport transport = HttpClientTransport.withExecutor(
        request -> CompletableFuture.completedFuture(
            newResponse(200, statusPayload(hash, "Rejected"))),
        ClientConfig.builder().setBaseUri(URI.create("http://localhost:8080")).build());

    boolean threw = false;
    try {
      transport
          .waitForTransactionStatus(
              hash, PipelineStatusOptions.builder().intervalMillis(0L).build())
          .join();
    } catch (final RuntimeException ex) {
      threw = true;
      final Throwable cause = ex.getCause() != null ? ex.getCause() : ex;
      assert cause instanceof TransactionStatusException : "Expected TransactionStatusException";
      assert "Rejected".equals(((TransactionStatusException) cause).status())
          : "Expected rejected status";
    }
    assert threw : "Expected waitForTransactionStatus to throw on failure status";
  }

  private static void waitForTransactionStatusFailureIncludesRejectionReason() {
    final String rejectionReason = "build_claim_missing";
    final String hash = canonicalHash("cafed00d");
    final HttpClientTransport transport = HttpClientTransport.withExecutor(
        request -> CompletableFuture.completedFuture(
            newResponse(200, statusPayloadWithRejectionReason(hash, rejectionReason))),
        ClientConfig.builder().setBaseUri(URI.create("http://localhost:8080")).build());

    boolean threw = false;
    try {
      transport
          .waitForTransactionStatus(
              hash, PipelineStatusOptions.builder().intervalMillis(0L).build())
          .join();
    } catch (final RuntimeException ex) {
      threw = true;
      final Throwable cause = ex.getCause() != null ? ex.getCause() : ex;
      assert cause instanceof TransactionStatusException : "Expected TransactionStatusException";
      final TransactionStatusException statusError = (TransactionStatusException) cause;
      assert rejectionReason.equals(statusError.rejectionReason().orElse(null))
          : "Expected rejection reason to be surfaced";
      assert statusError.getMessage().contains("reason=" + rejectionReason)
          : "Expected rejection reason in exception message";
    }
    assert threw : "Expected waitForTransactionStatus to throw on failure status";
  }

  private static void waitForTransactionStatusFailureUsesCurrentDiagnosticReason() {
    final String rejectionReason = "allowance_exceeded";
    final String hash = canonicalHash("cafe0001");
    final HttpClientTransport transport = HttpClientTransport.withExecutor(
        request -> CompletableFuture.completedFuture(
            newResponse(200, statusPayloadWithRejectionReason(hash, rejectionReason))),
        ClientConfig.builder().setBaseUri(URI.create("http://localhost:8080")).build());

    boolean threw = false;
    try {
      transport
          .waitForTransactionStatus(
              hash, PipelineStatusOptions.builder().intervalMillis(0L).build())
          .join();
    } catch (final RuntimeException ex) {
      threw = true;
      final Throwable cause = ex.getCause() != null ? ex.getCause() : ex;
      assert cause instanceof TransactionStatusException : "Expected TransactionStatusException";
      final TransactionStatusException statusError = (TransactionStatusException) cause;
      assert rejectionReason.equals(statusError.rejectionReason().orElse(null))
          : "Expected diagnostic rejection reason to be surfaced";
    }
    assert threw : "Expected waitForTransactionStatus to throw on failure status";
  }

  private static void waitForTransactionStatusSurfacesUnexpectedHttpStatusDetails() {
    final HttpClientTransport transport = HttpClientTransport.withExecutor(
        request ->
            CompletableFuture.completedFuture(
                newResponse(
                    429,
                    "{\"error\":\"rate limited\"}".getBytes(StandardCharsets.UTF_8),
                    Map.of("X-Iroha-Reject-Code", java.util.List.of("rate_limited")))),
        ClientConfig.builder().setBaseUri(URI.create("http://localhost:8080")).build());

    boolean threw = false;
    try {
      transport
          .waitForTransactionStatus(
              canonicalHash("abcd"), PipelineStatusOptions.builder().intervalMillis(0L).build())
          .join();
    } catch (final RuntimeException ex) {
      threw = true;
      final Throwable cause = ex.getCause() != null ? ex.getCause() : ex;
      assert cause instanceof TransactionStatusHttpException
          : "Expected TransactionStatusHttpException";
      final TransactionStatusHttpException statusError = (TransactionStatusHttpException) cause;
      assert canonicalHash("abcd").equals(statusError.hashHex())
          : "Expected hash to propagate";
      assert statusError.statusCode() == 429
          : "Expected status code to propagate";
      assert "rate_limited".equals(statusError.rejectCode().orElse(null))
          : "Expected reject code to propagate";
      assert statusError.responseBody().orElse("").contains("rate limited")
          : "Expected response body to propagate";
      assert cause.getMessage().contains("429")
          : "Expected status code in exception message";
      assert cause.getMessage().contains("reject_code=rate_limited")
          : "Expected reject code in exception message";
      assert cause.getMessage().contains("rate limited")
          : "Expected body preview in exception message";
    }
    assert threw : "Expected waitForTransactionStatus to fail for unexpected HTTP status";
  }

  private static void waitForTransactionStatusUsesCompactJsonMessage() {
    final HttpClientTransport transport = HttpClientTransport.withExecutor(
        request ->
            CompletableFuture.completedFuture(
                newResponse(
                    422,
                    "{\"status\":\"invalid\",\"code\":\"E123\"}"
                        .getBytes(StandardCharsets.UTF_8))),
        ClientConfig.builder().setBaseUri(URI.create("http://localhost:8080")).build());

    boolean threw = false;
    try {
      transport
          .waitForTransactionStatus(
              canonicalHash("feed"), PipelineStatusOptions.builder().intervalMillis(0L).build())
          .join();
    } catch (final RuntimeException ex) {
      threw = true;
      final Throwable cause = ex.getCause() != null ? ex.getCause() : ex;
      assert cause instanceof TransactionStatusHttpException
          : "Expected TransactionStatusHttpException";
      final TransactionStatusHttpException statusError = (TransactionStatusHttpException) cause;
      assert "{\"code\":\"E123\",\"status\":\"invalid\"}".equals(statusError.responseBody().orElse(null))
          : "Expected compact sorted JSON error body";
      assert cause.getMessage().contains("{\"code\":\"E123\",\"status\":\"invalid\"}")
          : "Expected compact sorted JSON in exception text";
    }
    assert threw : "Expected waitForTransactionStatus to fail for message-less JSON errors";
  }

  private static void waitForTransactionStatusUsesNestedJsonErrorMessage() {
    final HttpClientTransport transport = HttpClientTransport.withExecutor(
        request ->
            CompletableFuture.completedFuture(
                newResponse(
                    502,
                    "{\"error\":{\"detail\":\"upstream status pipeline unavailable\"}}"
                        .getBytes(StandardCharsets.UTF_8))),
        ClientConfig.builder().setBaseUri(URI.create("http://localhost:8080")).build());

    boolean threw = false;
    try {
      transport
          .waitForTransactionStatus(
              canonicalHash("face"), PipelineStatusOptions.builder().intervalMillis(0L).build())
          .join();
    } catch (final RuntimeException ex) {
      threw = true;
      final Throwable cause = ex.getCause() != null ? ex.getCause() : ex;
      assert cause instanceof TransactionStatusHttpException
          : "Expected TransactionStatusHttpException";
      final TransactionStatusHttpException statusError = (TransactionStatusHttpException) cause;
      assert statusError.statusCode() == 502
          : "Expected status code to propagate";
      assert "upstream status pipeline unavailable".equals(statusError.responseBody().orElse(null))
          : "Expected nested detail message to be extracted";
      assert cause.getMessage().contains("upstream status pipeline unavailable")
          : "Expected nested message in exception text";
    }
    assert threw : "Expected waitForTransactionStatus to fail for nested JSON status errors";
  }

  private static void waitForTransactionStatusUsesErrorsArrayMessage() {
    final HttpClientTransport transport = HttpClientTransport.withExecutor(
        request ->
            CompletableFuture.completedFuture(
                newResponse(
                    422,
                    "{\"errors\":[{\"message\":\"status query validation failed\"}]}"
                        .getBytes(StandardCharsets.UTF_8))),
        ClientConfig.builder().setBaseUri(URI.create("http://localhost:8080")).build());

    boolean threw = false;
    try {
      transport
          .waitForTransactionStatus(
              canonicalHash("bead"), PipelineStatusOptions.builder().intervalMillis(0L).build())
          .join();
    } catch (final RuntimeException ex) {
      threw = true;
      final Throwable cause = ex.getCause() != null ? ex.getCause() : ex;
      assert cause instanceof TransactionStatusHttpException
          : "Expected TransactionStatusHttpException";
      final TransactionStatusHttpException statusError = (TransactionStatusHttpException) cause;
      assert statusError.statusCode() == 422
          : "Expected status code to propagate";
      assert "status query validation failed".equals(statusError.responseBody().orElse(null))
          : "Expected first array message to be extracted";
      assert cause.getMessage().contains("status query validation failed")
          : "Expected array message in exception text";
    }
    assert threw : "Expected waitForTransactionStatus to fail for array-shaped status errors";
  }

  private static void waitForTransactionStatusTruncatesOversizedErrorBody() {
    final String oversized = "x".repeat(700);
    final HttpClientTransport transport = HttpClientTransport.withExecutor(
        request -> CompletableFuture.completedFuture(newResponse(500, oversized.getBytes(StandardCharsets.UTF_8))),
        ClientConfig.builder().setBaseUri(URI.create("http://localhost:8080")).build());

    boolean threw = false;
    try {
      transport
          .waitForTransactionStatus(
              canonicalHash("beef"), PipelineStatusOptions.builder().intervalMillis(0L).build())
          .join();
    } catch (final RuntimeException ex) {
      threw = true;
      final Throwable cause = ex.getCause() != null ? ex.getCause() : ex;
      assert cause instanceof TransactionStatusHttpException
          : "Expected TransactionStatusHttpException";
      final TransactionStatusHttpException statusError = (TransactionStatusHttpException) cause;
      final String body = statusError.responseBody().orElse("");
      assert body.length() == 515 : "Expected truncated body preview length";
      assert body.endsWith("...") : "Expected truncated body preview suffix";
    }
    assert threw : "Expected waitForTransactionStatus to fail for oversized error body";
  }

  @SuppressWarnings("unchecked")
  private static void waitForTransactionStatusMatchesSharedErrorMessageContractFixture() {
    final Path fixturePath = findSharedFixturePath();
    final String fixtureText;
    try {
      fixtureText = Files.readString(fixturePath, StandardCharsets.UTF_8);
    } catch (final Exception ex) {
      throw new IllegalStateException("Failed to read tx-status error contract fixture", ex);
    }
    final Object parsed = JsonParser.parse(fixtureText);
    if (!(parsed instanceof Map<?, ?> root)) {
      throw new IllegalStateException("tx-status contract fixture root must be an object");
    }
    final Object casesValue = root.get("cases");
    if (!(casesValue instanceof List<?> rawCases)) {
      throw new IllegalStateException("tx-status contract fixture cases must be an array");
    }
    for (final Object rawCase : rawCases) {
      if (!(rawCase instanceof Map<?, ?> fixtureCase)) {
        throw new IllegalStateException("fixture case must be an object");
      }
      final String id = String.valueOf(fixtureCase.get("id"));
      final int statusCode = ((Number) fixtureCase.get("status_code")).intValue();
      final String rejectCodeHeader =
          fixtureCase.get("reject_code_header") instanceof String
              ? (String) fixtureCase.get("reject_code_header")
              : null;
      final String rejectCodeHeaderName =
          fixtureCase.get("reject_code_header_name") instanceof String
              ? (String) fixtureCase.get("reject_code_header_name")
              : "X-Iroha-Reject-Code";
      final byte[] body;
      if (fixtureCase.get("body_json") != null) {
        body = encodeFixtureJson(fixtureCase.get("body_json")).getBytes(StandardCharsets.UTF_8);
      } else if (fixtureCase.get("body_text") instanceof String bodyText) {
        body = bodyText.getBytes(StandardCharsets.UTF_8);
      } else {
        body = new byte[0];
      }
      final Map<String, List<String>> headers =
          rejectCodeHeader == null
              ? Map.of()
              : Map.of(rejectCodeHeaderName, List.of(rejectCodeHeader));
      final HttpClientTransport transport =
          HttpClientTransport.withExecutor(
              request ->
                  CompletableFuture.completedFuture(
                      newResponse(statusCode, body, headers)),
              ClientConfig.builder().setBaseUri(URI.create("http://localhost:8080")).build());

      boolean threw = false;
      try {
        transport
            .waitForTransactionStatus(
                canonicalHash("babe"), PipelineStatusOptions.builder().intervalMillis(0L).build())
            .join();
      } catch (final RuntimeException ex) {
        threw = true;
        final Throwable cause = ex.getCause() != null ? ex.getCause() : ex;
        assert cause instanceof TransactionStatusHttpException
            : id + ": expected TransactionStatusHttpException";
        final TransactionStatusHttpException statusError = (TransactionStatusHttpException) cause;
        assert statusError.statusCode() == statusCode : id + ": status code mismatch";
        if (fixtureCase.get("expected_reject_code") instanceof String expectedRejectCode) {
          assert expectedRejectCode.equals(statusError.rejectCode().orElse(null))
              : id + ": reject code mismatch";
        }
        if (fixtureCase.get("expected_message") instanceof String expectedMessage) {
          assert expectedMessage.equals(statusError.responseBody().orElse(null))
              : id + ": message mismatch";
        }
        if (fixtureCase.get("expected_message_length") instanceof Number expectedLength) {
          assert statusError.responseBody().orElse("").length() == expectedLength.intValue()
              : id + ": message length mismatch";
        }
        if (fixtureCase.get("expected_message_suffix") instanceof String expectedSuffix) {
          assert statusError.responseBody().orElse("").endsWith(expectedSuffix)
              : id + ": message suffix mismatch";
        }
      }
      assert threw : id + ": expected waitForTransactionStatus to fail";
    }
  }

  private static Path findSharedFixturePath() {
    Path current = Path.of("").toAbsolutePath();
    while (current != null) {
      final Path candidate = current.resolve("fixtures/sdk/tx_status_error_message_contract.json");
      if (Files.exists(candidate)) {
        return candidate;
      }
      current = current.getParent();
    }
    throw new IllegalStateException("Unable to locate fixtures/sdk/tx_status_error_message_contract.json");
  }

  @SuppressWarnings("unchecked")
  private static String encodeFixtureJson(final Object value) {
    if (value == null) {
      return "null";
    }
    if (value instanceof String text) {
      return "\"" + escapeJsonString(text) + "\"";
    }
    if (value instanceof Boolean || value instanceof Number) {
      return String.valueOf(value);
    }
    if (value instanceof List<?> list) {
      final StringBuilder builder = new StringBuilder("[");
      boolean first = true;
      for (final Object entry : list) {
        if (!first) {
          builder.append(',');
        }
        first = false;
        builder.append(encodeFixtureJson(entry));
      }
      builder.append(']');
      return builder.toString();
    }
    if (value instanceof Map<?, ?> map) {
      final StringBuilder builder = new StringBuilder("{");
      boolean first = true;
      for (final Map.Entry<?, ?> entry : map.entrySet()) {
        if (!first) {
          builder.append(',');
        }
        first = false;
        builder
            .append('"')
            .append(escapeJsonString(String.valueOf(entry.getKey())))
            .append('"')
            .append(':')
            .append(encodeFixtureJson(entry.getValue()));
      }
      builder.append('}');
      return builder.toString();
    }
    return "\"" + escapeJsonString(String.valueOf(value)) + "\"";
  }

  private static String escapeJsonString(final String value) {
    final StringBuilder escaped = new StringBuilder();
    for (int i = 0; i < value.length(); i++) {
      final char ch = value.charAt(i);
      switch (ch) {
        case '"' -> escaped.append("\\\"");
        case '\\' -> escaped.append("\\\\");
        case '\b' -> escaped.append("\\b");
        case '\f' -> escaped.append("\\f");
        case '\n' -> escaped.append("\\n");
        case '\r' -> escaped.append("\\r");
        case '\t' -> escaped.append("\\t");
        default -> {
          if (ch < 0x20) {
            escaped.append(String.format("\\u%04x", (int) ch));
          } else {
            escaped.append(ch);
          }
        }
      }
    }
    return escaped.toString();
  }

  private static void waitForTransactionStatusHonoursMaxAttempts() {
    final String hash = canonicalHash("feed");
    final HttpClientTransport transport = HttpClientTransport.withExecutor(
        request -> CompletableFuture.completedFuture(
            newResponse(200, statusPayload(hash, "Queued"))),
        ClientConfig.builder().setBaseUri(URI.create("http://localhost:8080")).build());

    boolean threw = false;
    try {
      transport
          .waitForTransactionStatus(
              hash, PipelineStatusOptions.builder().intervalMillis(0L).maxAttempts(2).timeoutMillis(null).build())
          .join();
    } catch (final RuntimeException ex) {
      threw = true;
      final Throwable cause = ex.getCause() != null ? ex.getCause() : ex;
      assert cause instanceof TransactionTimeoutException : "Expected TransactionTimeoutException";
      assert ((TransactionTimeoutException) cause).attempts() == 2 : "Expected two attempts";
    }
    assert threw : "Expected waitForTransactionStatus to time out";
  }

  private static void waitForTransactionStatusSaturatesOverflowingDeadline() {
    final String hash = canonicalHash("feed");
    final AtomicInteger requests = new AtomicInteger();
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            request -> {
              requests.incrementAndGet();
              return CompletableFuture.completedFuture(
                  newResponse(200, statusPayload(hash, "Queued")));
            },
            ClientConfig.builder().setBaseUri(URI.create("http://localhost:8080")).build());

    boolean threw = false;
    try {
      transport
          .waitForTransactionStatus(
              hash,
              PipelineStatusOptions.builder()
                  .intervalMillis(0L)
                  .maxAttempts(2)
                  .timeoutMillis(Long.MAX_VALUE)
                  .build())
          .join();
    } catch (final RuntimeException ex) {
      threw = true;
      final Throwable cause = ex.getCause() != null ? ex.getCause() : ex;
      assert cause instanceof TransactionTimeoutException
          : "Expected TransactionTimeoutException";
      assert ((TransactionTimeoutException) cause).attempts() == 2
          : "Expected two attempts";
    }
    assert threw : "Expected waitForTransactionStatus to reach maxAttempts";
    assert requests.get() == 2 : "Overflowing timeout must not expire after one request";
  }

  private static void waitForTransactionStatusFailsOnInvalidPayload() {
    final HttpClientTransport transport = HttpClientTransport.withExecutor(
        request -> CompletableFuture.completedFuture(
            newResponse(200, "[]".getBytes(StandardCharsets.UTF_8))),
        ClientConfig.builder().setBaseUri(URI.create("http://localhost:8080")).build());

    boolean threw = false;
    try {
      transport
          .waitForTransactionStatus(
              canonicalHash("feedface"), PipelineStatusOptions.builder().intervalMillis(0L).build())
          .join();
    } catch (final RuntimeException ex) {
      threw = true;
      final Throwable cause = ex.getCause() != null ? ex.getCause() : ex;
      assert cause instanceof IllegalStateException
          : "Expected parsing error to propagate";
    }
    assert threw : "Expected waitForTransactionStatus to fail on invalid payload";
  }

  private static void waitForTransactionStatusRejectsNonAuthoritativeTerminalPayloads() {
    final String hash = canonicalHash("f00d");
    final List<byte[]> invalidPayloads =
        List.of(
            statusPayload(canonicalHash("bad0"), "Applied"),
            statusPayload(hash, "Applied", "local", "state", 7),
            statusPayload(hash, "Applied", "global", "cache", 7),
            statusPayload(hash, "Applied", "global", "state", null),
            statusPayload(hash, "Applied", "global", "state", 0),
            statusPayload(hash, "Applied", "global", "state", -1),
            statusPayload(hash, "Applied", "global", "state", 1.5),
            statusPayload(hash, "Rejected", "global", "cache", null));

    for (final byte[] payload : invalidPayloads) {
      final HttpClientTransport transport =
          HttpClientTransport.withExecutor(
              request -> CompletableFuture.completedFuture(newResponse(200, payload)),
              ClientConfig.builder().setBaseUri(URI.create("http://localhost:8080")).build());
      try {
        transport
            .waitForTransactionStatus(
                hash, PipelineStatusOptions.builder().intervalMillis(0L).build())
            .join();
      } catch (final RuntimeException expected) {
        final Throwable cause = expected.getCause() != null ? expected.getCause() : expected;
        assert cause instanceof IllegalStateException
            : "Expected non-authoritative status to fail closed";
        continue;
      }
      throw new AssertionError("Expected non-authoritative terminal payload rejection");
    }
  }

  private static void waitForTransactionStatusAcceptsQueuedHttpResponseCode() {
    final String hash = canonicalHash("202");
    final SequencedExecutor executor =
        new SequencedExecutor(
            newResponse(202, statusPayload(hash, "Queued")),
            newResponse(200, statusPayload(hash, "Applied")));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder().setBaseUri(URI.create("http://localhost:8080")).build());
    final Map<String, Object> payload =
        transport
            .waitForTransactionStatus(
                hash, PipelineStatusOptions.builder().intervalMillis(0L).build())
            .join();
    assert "Applied".equals(PipelineStatusExtractor.extractStatusKind(payload).orElse(null))
        : "HTTP 202 queued responses must remain pending until authoritative application";
  }

  private static void waitForTransactionStatusRejectsNoritoWhenJsonWasRequested() {
    final String hash = canonicalHash("0bad");
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            request ->
                CompletableFuture.completedFuture(
                    newResponse(200, new byte[] {'N', 'R', 'T', '0', 0x01})),
            ClientConfig.builder().setBaseUri(URI.create("http://localhost:8080")).build());
    try {
      transport
          .waitForTransactionStatus(
              hash, PipelineStatusOptions.builder().intervalMillis(0L).build())
          .join();
    } catch (final RuntimeException expected) {
      final Throwable cause = expected.getCause() != null ? expected.getCause() : expected;
      assert cause instanceof IllegalStateException
          : "Expected content negotiation mismatch to fail closed";
      return;
    }
    throw new AssertionError("Expected non-JSON status payload rejection");
  }

  private static void waitForTransactionStatusRejectsNonCanonicalRequestHashes() {
    final String hash = canonicalHash("abcd");
    for (final String invalid :
        List.of(hash.toUpperCase(java.util.Locale.ROOT), " " + hash, hash + " ", "0x" + hash)) {
      final AtomicInteger dispatches = new AtomicInteger();
      final HttpClientTransport transport =
          HttpClientTransport.withExecutor(
              request -> {
                dispatches.incrementAndGet();
                return CompletableFuture.completedFuture(newResponse(404, new byte[0]));
              },
              ClientConfig.builder().setBaseUri(URI.create("http://localhost:8080")).build());
      try {
        transport.waitForTransactionStatus(invalid, PipelineStatusOptions.builder().build());
      } catch (final IllegalArgumentException expected) {
        assert dispatches.get() == 0 : "Invalid hash must fail before network dispatch";
        continue;
      }
      throw new AssertionError("Expected non-canonical request hash rejection: " + invalid);
    }
  }

  private static void submitTransactionProvidesCanonicalHashForPolling() {
    final SignedTransaction transaction = sampleTransaction((byte) 0x10);
    final String expectedHash = SignedTransactionHasher.hashHex(transaction);
    final TrackingExecutor executor = new TrackingExecutor(expectedHash);
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("http://localhost:8080"))
                .setRequestTimeout(Duration.ofSeconds(1))
                .build());

    final ClientResponse response = transport.submitTransaction(transaction).join();
    assert expectedHash.equals(response.hashHex().orElse(null))
        : "submitTransaction must return canonical hash";

    final Map<String, Object> payload =
        transport
            .waitForTransactionStatus(
                expectedHash, PipelineStatusOptions.builder().intervalMillis(0L).build())
            .join();

    assert "Applied".equals(PipelineStatusExtractor.extractStatusKind(payload).orElse(null))
        : "Expected Applied status after polling";
    assert executor.observedExpectedHash()
        : "Status polling must include canonical hash and scope=auto in request URI";
  }

  private static void submitTransactionPrefersAuthoritativeReceiptHashHeaderForPolling() {
    final SignedTransaction transaction = sampleTransaction((byte) 0x11);
    final String localHash = SignedTransactionHasher.hashHex(transaction);
    final String authoritativeHash =
        "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
    final TrackingExecutor executor =
        new TrackingExecutor(authoritativeHash, authoritativeHash.toUpperCase(java.util.Locale.ROOT));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("http://localhost:8080"))
                .setRequestTimeout(Duration.ofSeconds(1))
                .build());

    final ClientResponse response = transport.submitTransaction(transaction).join();
    assert !localHash.equals(response.hashHex().orElse(null))
        : "authoritative Torii receipt hash should override the local fallback hash";
    assert authoritativeHash.equals(response.hashHex().orElse(null))
        : "submitTransaction must expose the Torii receipt hash header";

    transport
        .waitForTransactionStatus(
            response.hashHex().orElseThrow(),
            PipelineStatusOptions.builder().intervalMillis(0L).build())
        .join();

    assert executor.observedExpectedHash()
        : "Status polling must use the authoritative receipt hash";
  }

  private static TransportResponse newResponse(final int status, final byte[] body) {
    return new TransportResponse(status, body, "", Map.of());
  }

  private static String canonicalHash(final String prefix) {
    if (prefix == null || !prefix.matches("[0-9a-f]+") || prefix.length() > 64) {
      throw new IllegalArgumentException("canonical hash prefix must be lowercase hexadecimal");
    }
    final StringBuilder hash = new StringBuilder(prefix);
    while (hash.length() < 64) {
      hash.append('0');
    }
    return hash.toString();
  }

  private static TransportResponse newResponse(
      final int status, final byte[] body, final Map<String, java.util.List<String>> headers) {
    return new TransportResponse(status, body, "", headers);
  }

  private static byte[] statusPayload(final String hash, final String kind) {
    final boolean terminal =
        "Applied".equals(kind) || "Rejected".equals(kind) || "Expired".equals(kind);
    return statusPayload(
        hash,
        kind,
        "global",
        terminal ? "state" : "cache",
        "Applied".equals(kind) ? 7 : null);
  }

  private static byte[] statusPayload(
      final String hash,
      final String kind,
      final String scope,
      final String resolvedFrom,
      final Number blockHeight) {
    final String renderedBlockHeight =
        blockHeight == null ? "" : ",\"block_height\":" + blockHeight;
    final String json =
        "{\"hash\":\""
            + hash
            + "\",\"status\":{\"kind\":\""
            + kind
            + "\""
            + renderedBlockHeight
            + "},\"summary\":\""
            + kind
            + "\",\"diagnostics\":[],\"scope\":\""
            + scope
            + "\",\"resolved_from\":\""
            + resolvedFrom
            + "\"}";
    return json.getBytes(StandardCharsets.UTF_8);
  }

  private static byte[] statusPayloadWithRejectionReason(
      final String hash, final String rejectionReason) {
    final String escaped = escapeJsonString(rejectionReason);
    final String json =
        "{\"hash\":\""
            + hash
            + "\",\"status\":{\"kind\":\"Rejected\"},\"summary\":\"Rejected: "
            + escaped
            + "\",\"diagnostics\":[{\"category\":\"validation\",\"message\":\""
            + escaped
            + "\",\"decoded_reason\":\""
            + escaped
            + "\"}],\"scope\":\"global\",\"resolved_from\":\"state\"}";
    return json.getBytes(StandardCharsets.UTF_8);
  }

  private static SignedTransaction sampleTransaction(final byte seed) {
    final TransactionPayload payload =
        TransactionPayload.builder().setFeePayment(org.hyperledger.iroha.android.model.FeePaymentIntent.authority(java.util.Collections.emptyList(), 1L))
            .setChainId(String.format("%08x", seed))
            .setAuthority(TestAccountIds.ed25519Authority(0x27))
            .setCreationTimeMs(1_700_000_000_000L + (seed & 0xFF))
            .setInstructionBytes(new byte[] {seed, (byte) (seed + 1)})
            .setTimeToLiveMs(5_000L)
            .setNonce((seed & 0xFF) + 1)
            .setMetadata(Map.of("note", "tx-" + seed))
            .build();
    final NoritoJavaCodecAdapter codec = new NoritoJavaCodecAdapter();
    final byte[] encoded;
    try {
      encoded = codec.encodeTransaction(payload);
    } catch (final Exception ex) {
      throw new IllegalStateException("Failed to encode transaction payload", ex);
    }
    final byte[] signature = new byte[64];
    final byte[] publicKey = new byte[32];
    Arrays.fill(signature, (byte) (seed + 1));
    Arrays.fill(publicKey, (byte) (seed + 2));
    return new SignedTransaction(encoded, signature, publicKey, codec.schemaName());
  }

  private static final class SequencedExecutor implements HttpTransportExecutor {
    private final TransportResponse[] responses;
    private int index = 0;

    private SequencedExecutor(final TransportResponse... responses) {
      this.responses = Objects.requireNonNull(responses, "responses");
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      if (index >= responses.length) {
        return CompletableFuture.completedFuture(responses[responses.length - 1]);
      }
      return CompletableFuture.completedFuture(responses[index++]);
    }
  }

  private static final class TrackingExecutor implements HttpTransportExecutor {
    private final String expectedHash;
    private final String submitHeaderHash;
    private final AtomicInteger pollCount = new AtomicInteger(0);
    private volatile boolean observedExpectedHash = false;

    private TrackingExecutor(final String expectedHash) {
      this(expectedHash, null);
    }

    private TrackingExecutor(final String expectedHash, final String submitHeaderHash) {
      this.expectedHash = expectedHash;
      this.submitHeaderHash = submitHeaderHash;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      if ("POST".equals(request.method())) {
        final Map<String, List<String>> headers =
            submitHeaderHash == null
                ? Map.of()
                : Map.of("x-iroha-transaction-hash", List.of(submitHeaderHash));
        return CompletableFuture.completedFuture(new TransportResponse(202, new byte[0], "", headers));
      }
      if ("GET".equals(request.method())) {
        final String query = request.uri().getQuery();
        if (query != null
            && query.contains("hash=" + expectedHash)
            && query.contains("scope=auto")) {
          observedExpectedHash = true;
        }
        final int count = pollCount.getAndIncrement();
        if (count == 0) {
          return CompletableFuture.completedFuture(
              new TransportResponse(200, statusPayload(expectedHash, "Queued"), "", Map.of()));
        }
        if (count == 1) {
          return CompletableFuture.completedFuture(
              new TransportResponse(200, statusPayload(expectedHash, "Committed"), "", Map.of()));
        }
        return CompletableFuture.completedFuture(
            new TransportResponse(200, statusPayload(expectedHash, "Applied"), "", Map.of()));
      }
      throw new IllegalStateException("Unexpected HTTP method " + request.method());
    }

    boolean observedExpectedHash() {
      return observedExpectedHash;
    }
  }
}

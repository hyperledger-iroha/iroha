// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertInstanceOf;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.sdk.client.transport.TransportRequest;
import org.hyperledger.iroha.sdk.client.transport.TransportResponse;
import org.junit.jupiter.api.Test;

/** Java consumers use the Kotlin-owned anonymous SORA Parliament family client directly. */
final class AnonymousBallotApiV1JavaConsumerTest {
  private static final URI BASE = URI.create("https://torii.example/api");

  @Test
  void pathsAndLimitsAreJavaConstantsAndStaticBuilders() {
    final byte[] scope = new byte[AnonymousBallotApiV1.SCOPE_BYTES];
    Arrays.fill(scope, (byte) 0xab);
    assertEquals("/v1/gov/anonymous-ballots/boards/" + repeat("ab", 64),
        AnonymousBallotApiV1.boardReadPath(scope));
    assertEquals(AnonymousBallotApiV1.boardReadPath(scope),
        AnonymousBallotApiV1.boardReadPath(repeat("ab", 64)));
    assertEquals("/v1/gov/anonymous-ballots/attempts/" + repeat("01", 32),
        AnonymousBallotApiV1.attemptReadPath(filled(AnonymousBallotApiV1.GOVERNANCE_ATTEMPT_ID_BYTES, 1)));
    assertEquals("/v1/gov/anonymous-ballots", AnonymousBallotApiV1.SUBMIT_PATH);
    assertEquals("application/x-norito", AnonymousBallotApiV1.MEDIA_TYPE);
    assertEquals(202, AnonymousBallotApiV1.SUBMIT_HANDLED_STATUS);
    assertEquals(262_144, AnonymousBallotApiV1.ENVELOPE_MAX_BYTES);
    assertEquals(1_048_576, AnonymousBallotApiV1.BOARD_MAX_BYTES);
    assertEquals(16_777_216, AnonymousBallotApiV1.ATTEMPT_MAX_BYTES);
    assertThrows(IllegalArgumentException.class,
        () -> AnonymousBallotApiV1.boardReadPath(new byte[63]));
    assertThrows(IllegalArgumentException.class,
        () -> AnonymousBallotApiV1.attemptReadPath(repeat("AB", 32)));
  }

  @Test
  void submissionCarriesOnlyTheMediaTypeAndRefusalsExposeStatusAndCode() {
    final Map<String, String> configured = new LinkedHashMap<>();
    configured.put("X-API-Token", "listener-secret");
    configured.put("Authorization", "Bearer session");
    configured.put(CanonicalRequestSigner.HEADER_ACCOUNT, "alice@universal");
    configured.put("X-Request-Id", "correlate-me");
    final CapturingExecutor executor = new CapturingExecutor(
        new TransportResponse(202, new byte[0], "Accepted", null, null, false));
    final byte[] envelope = filled(512, 7);
    try (AnonymousBallotToriiClientV1 client = AnonymousBallotToriiClientV1.builder()
        .executor(executor)
        .baseUri(BASE)
        .defaultHeaders(configured)
        .build()) {
      final Void handled = client.submitAnonymousBallot(envelope).join();
      assertNull(handled);
      final TransportRequest request = executor.requests.get(0);
      assertEquals("POST", request.method);
      assertEquals("/api/v1/gov/anonymous-ballots", request.uri.getRawPath());
      assertEquals(Collections.singletonMap("Content-Type",
          Collections.singletonList("application/x-norito")), request.getHeaders());
      assertArrayEquals(envelope, request.getBody());

      final Map<String, List<String>> headers = new LinkedHashMap<>();
      headers.put("Content-Type", Collections.singletonList("application/json"));
      headers.put("Retry-After", Collections.singletonList("3"));
      executor.next = new TransportResponse(429,
          "{\"code\":\"proof_rate_limited\",\"message\":\"throttled\"}"
              .getBytes(StandardCharsets.UTF_8),
          "", headers, null, false);
      final CompletionException failure = assertThrows(CompletionException.class,
          () -> client.submitAnonymousBallot(envelope).join());
      final AnonymousBallotToriiExceptionV1 refusal =
          assertInstanceOf(AnonymousBallotToriiExceptionV1.class, failure.getCause());
      assertEquals(429, refusal.status);
      assertEquals("proof_rate_limited", refusal.code);
      assertEquals(Long.valueOf(3), refusal.retryAfterSeconds);
      assertInstanceOf(ToriiApiException.class, refusal);
      assertThrows(IllegalArgumentException.class,
          () -> client.submitAnonymousBallot(new byte[0]));
    }
  }

  @Test
  void readsReturnTheCanonicalNoritoBytesUndecoded() {
    final byte[] record = filled(256, 3);
    final CapturingExecutor executor = new CapturingExecutor(new TransportResponse(200, record, "OK",
        Collections.singletonMap("Content-Type", Collections.singletonList("application/x-norito")),
        null, false));
    final ClientConfig config = ClientConfig.builder().setBaseUri(BASE).build();
    try (HttpClientTransport transport = new HttpClientTransport(executor, config);
        AnonymousBallotToriiClientV1 client = transport.newAnonymousBallotToriiClientV1()) {
      assertArrayEquals(record, client.getAnonymousBallotBoard(repeat("cd", 64)).join());
      assertArrayEquals(record,
          client.getAnonymousParliamentAttempt(filled(AnonymousBallotApiV1.GOVERNANCE_ATTEMPT_ID_BYTES, 9)).join());
      final TransportRequest attempt = executor.requests.get(1);
      assertEquals("GET", attempt.method);
      assertEquals("/api/v1/gov/anonymous-ballots/attempts/" + repeat("09", 32),
          attempt.uri.getRawPath());
      assertEquals(Long.valueOf(AnonymousBallotApiV1.ATTEMPT_MAX_BYTES), attempt.maximumResponseBytes);
      assertEquals(Collections.singletonList("application/x-norito"), attempt.getHeaders().get("Accept"));

      executor.next = new TransportResponse(404,
          "{\"code\":\"anonymous_parliament_attempt_not_found\",\"message\":\"none\"}"
              .getBytes(StandardCharsets.UTF_8),
          "", null, null, false);
      final CompletionException failure = assertThrows(CompletionException.class,
          () -> client.getAnonymousParliamentAttempt(repeat("09", 32)).join());
      final AnonymousBallotToriiExceptionV1 missing =
          assertInstanceOf(AnonymousBallotToriiExceptionV1.class, failure.getCause());
      assertEquals(404, missing.status);
      assertEquals("anonymous_parliament_attempt_not_found", missing.code);
      assertNull(missing.retryAfterSeconds);
    }
    assertEquals(3, executor.requests.size());
  }

  private static byte[] filled(int size, int value) {
    final byte[] bytes = new byte[size];
    Arrays.fill(bytes, (byte) value);
    return bytes;
  }

  private static String repeat(String value, int count) {
    final StringBuilder result = new StringBuilder(value.length() * count);
    for (int index = 0; index < count; index++) result.append(value);
    return result.toString();
  }

  private static final class CapturingExecutor implements HttpTransportExecutor {
    private final List<TransportRequest> requests = new ArrayList<>();
    private TransportResponse next;

    private CapturingExecutor(TransportResponse next) {
      this.next = next;
    }

    @Override
    public CompletableFuture<TransportResponse> execute(TransportRequest request) {
      requests.add(request);
      return CompletableFuture.completedFuture(next);
    }
  }
}

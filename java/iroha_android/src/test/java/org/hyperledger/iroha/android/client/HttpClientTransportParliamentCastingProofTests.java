package org.hyperledger.iroha.android.client;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertNotNull;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.math.BigInteger;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.Signature;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import java.util.concurrent.atomic.AtomicInteger;
import org.hyperledger.iroha.android.client.transport.RequestReplayPolicy;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.client.transport.TransportResponse;
import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.junit.Test;

/** Exact one-shot transport tests for the Parliament timed-OVN casting-proof route. */
public final class HttpClientTransportParliamentCastingProofTests {

  @Test
  public void castingProofPostIsExactAuthenticatedBoundedAndOneShot() {
    final byte[] responsePayload = new byte[] {2, 1, 0, 1};
    final NoritoHeader responseHeader =
        new NoritoHeader(
            decodeHex(ParliamentApiV1.TIMED_OVN_CASTING_PROOF_RESPONSE_SCHEMA_HASH_HEX),
            responsePayload.length,
            CRC64.compute(responsePayload),
            NoritoHeader.COMPACT_LEN,
            NoritoHeader.COMPRESSION_NONE);
    final byte[] responseFrame = concat(responseHeader.encode(), responsePayload);
    final Map<String, List<String>> responseHeaders = new LinkedHashMap<>();
    responseHeaders.put("Content-Type", Collections.singletonList("application/x-norito"));
    responseHeaders.put(
        "Content-Length", Collections.singletonList(Integer.toString(responseFrame.length)));
    final OneResponseExecutor executor =
        new OneResponseExecutor(
            new TransportResponse(
                200,
                responseFrame,
                "ok",
                responseHeaders));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            CanonicalRequestSigningTestSupport.signedClientConfig(
                "https://torii.example/api"));
    final ToriiCanonicalRequestAuth auth =
        canonicalRequestAuth();
    final String ballotId = repeat("33", 32);

    final ParliamentApiV1.TimedOvnCastingProofResponse response =
        transport
            .getParliamentTimedOvnCastingProofPageV1(
                ballotId, BigInteger.valueOf(17L), auth)
            .join();
    assertArrayEquals(responseFrame, response.canonicalNorito());
    final TransportRequest request = executor.lastRequest;
    assertNotNull(request);
    assertEquals("POST", request.method());
    assertEquals(
        "/api/v1/gov/parliament/ballots/" + ballotId + "/casting-proof",
        request.uri().getRawPath());
    assertArrayEquals(
        ParliamentApiV1.timedOvnCastingProofRequestNorito(17L), request.body());
    assertEquals(
        Collections.singletonList("application/x-norito"),
        request.headers().get("Content-Type"));
    assertEquals(
        Collections.singletonList("application/x-norito"), request.headers().get("Accept"));
    assertEquals(
        Collections.singletonList("identity"), request.headers().get("Accept-Encoding"));
    assertEquals(
        Long.valueOf(ParliamentApiV1.MAX_TIMED_OVN_CASTING_PROOF_RESPONSE_BYTES),
        request.maximumResponseBytes());
    assertEquals(RequestReplayPolicy.ONE_SHOT, request.replayPolicy());
    assertEquals(1, executor.requestCount);
    assertTrue(request.headers().containsKey(CanonicalRequestSigner.HEADER_SIGNATURE));

    final Map<String, List<String>> encodedHeaders = new LinkedHashMap<>();
    encodedHeaders.put("Content-Type", Collections.singletonList("application/x-norito"));
    encodedHeaders.put("Content-Encoding", Collections.singletonList("gzip"));
    final OneResponseExecutor encodedExecutor =
        new OneResponseExecutor(
            new TransportResponse(
                200,
                responseFrame,
                "ok",
                encodedHeaders));
    final HttpClientTransport encodedTransport =
        HttpClientTransport.withExecutor(
            encodedExecutor,
            CanonicalRequestSigningTestSupport.signedClientConfig(
                "https://torii.example"));
    assertThrows(
        CompletionException.class,
        () ->
            encodedTransport
                .requestParliamentTimedOvnCastingProofV1(ballotId, 17L, auth)
                .join());
    assertEquals(1, encodedExecutor.requestCount);
  }

  @Test
  public void castingContextGetIsAuthenticatedAndBounded() {
    final OneResponseExecutor executor =
        new OneResponseExecutor(
            new TransportResponse(404, new byte[0], "not found", Collections.emptyMap()));
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            CanonicalRequestSigningTestSupport.signedClientConfig(
                "https://torii.example/api"));
    final String ballotId = repeat("44", 32);

    assertThrows(
        CompletionException.class,
        () ->
            transport
                .getParliamentTimedOvnCastingContextV1(ballotId, canonicalRequestAuth())
                .join());
    final TransportRequest request = executor.lastRequest;
    assertNotNull(request);
    assertEquals("GET", request.method());
    assertEquals(
        "/api/v1/gov/parliament/ballots/" + ballotId + "/casting-context",
        request.uri().getRawPath());
    assertArrayEquals(new byte[0], request.body());
    assertEquals(Long.valueOf(ParliamentApiV1.MAX_STATE_BYTES), request.maximumResponseBytes());
    assertEquals(RequestReplayPolicy.ONE_SHOT, request.replayPolicy());
    assertTrue(request.headers().containsKey(CanonicalRequestSigner.HEADER_SIGNATURE));
    assertEquals(1, executor.requestCount);
  }

  @Test
  public void castingProofPagingDurablyAdvancesStaleAnchorBeyondSixtyThreeHeights() {
    final SequenceResponseExecutor executor =
        new SequenceResponseExecutor(castingProofResponseFrame());
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            CanonicalRequestSigningTestSupport.signedClientConfig(
                "https://torii.example/api"));
    final byte[] firstContext = filled(32, 0x11);
    final byte[] secondContext = filled(32, 0x22);
    final byte[] terminalContext = filled(32, 0x33);
    final AtomicInteger verifierCalls = new AtomicInteger();
    final List<ParliamentApiV1.TimedOvnCastingProofPageVerification> persisted =
        new java.util.ArrayList<>();
    final CompletableFuture<Void> firstPersistence = new CompletableFuture<>();

    final CompletableFuture<ParliamentApiV1.TimedOvnCastingProofTerminal> future =
        transport.requestParliamentTimedOvnCastingProofUntilTerminalV1(
            repeat("55", 32),
            7L,
            firstContext,
            unpinnedCanonicalRequestAuth(),
            (response, height, context) -> {
              final int call = verifierCalls.getAndIncrement();
              if (call == 0) {
                assertEquals(BigInteger.valueOf(7L), height);
                assertArrayEquals(firstContext, context);
                return new ParliamentApiV1.TimedOvnCastingProofPageVerification(
                    BigInteger.valueOf(70L), secondContext, true);
              }
              if (call == 1) {
                assertEquals(BigInteger.valueOf(70L), height);
                assertArrayEquals(secondContext, context);
                return new ParliamentApiV1.TimedOvnCastingProofPageVerification(
                    BigInteger.valueOf(75L), terminalContext, false);
              }
              throw new AssertionError("unexpected casting-proof page");
            },
            verification -> {
              persisted.add(verification);
              return persisted.size() == 1
                  ? firstPersistence
                  : CompletableFuture.completedFuture(null);
            });

    assertEquals(1, executor.requests.size());
    assertTrue(!future.isDone());
    firstPersistence.complete(null);
    final ParliamentApiV1.TimedOvnCastingProofTerminal terminal = future.join();

    assertEquals(2, executor.requests.size());
    assertEquals(2, persisted.size());
    assertEquals(2, terminal.verifiedPageCount);
    assertEquals(BigInteger.valueOf(70L), terminal.verificationAnchorHeight);
    assertArrayEquals(secondContext, terminal.verificationAnchorContextId());
    assertEquals(BigInteger.valueOf(75L), terminal.verification.evaluatedBlockHeight);
    assertTrue(!terminal.verification.moreAvailable);
    assertArrayEquals(
        ParliamentApiV1.timedOvnCastingProofRequestNorito(7L),
        executor.requests.get(0).body());
    assertArrayEquals(
        ParliamentApiV1.timedOvnCastingProofRequestNorito(70L),
        executor.requests.get(1).body());
  }

  @Test
  public void castingProofPagingRejectsNativeAdvancePastPageBound() {
    final SequenceResponseExecutor executor =
        new SequenceResponseExecutor(castingProofResponseFrame());
    final HttpClientTransport transport =
        HttpClientTransport.withExecutor(
            executor,
            CanonicalRequestSigningTestSupport.signedClientConfig(
                "https://torii.example"));
    final AtomicInteger persistCalls = new AtomicInteger();
    final CompletionException failure =
        assertThrows(
            CompletionException.class,
            () ->
                transport
                    .requestParliamentTimedOvnCastingProofUntilTerminalV1(
                        repeat("66", 32),
                        7L,
                        filled(32, 0x11),
                        unpinnedCanonicalRequestAuth(),
                        (response, height, context) ->
                            new ParliamentApiV1.TimedOvnCastingProofPageVerification(
                                BigInteger.valueOf(71L), filled(32, 0x22), true),
                        verification -> {
                          persistCalls.incrementAndGet();
                          return CompletableFuture.completedFuture(null);
                        })
                    .join());
    assertTrue(failure.getCause() instanceof IllegalArgumentException);
    assertEquals(0, persistCalls.get());
    assertEquals(1, executor.requests.size());
  }

  private static byte[] castingProofResponseFrame() {
    final byte[] payload = new byte[] {2, 1, 0, 1};
    final NoritoHeader header =
        new NoritoHeader(
            decodeHex(ParliamentApiV1.TIMED_OVN_CASTING_PROOF_RESPONSE_SCHEMA_HASH_HEX),
            payload.length,
            CRC64.compute(payload),
            NoritoHeader.COMPACT_LEN,
            NoritoHeader.COMPRESSION_NONE);
    return concat(header.encode(), payload);
  }

  private static byte[] decodeHex(final String value) {
    final byte[] decoded = new byte[value.length() / 2];
    for (int index = 0; index < decoded.length; index++) {
      decoded[index] =
          (byte) Integer.parseInt(value.substring(index * 2, index * 2 + 2), 16);
    }
    return decoded;
  }

  private static String repeat(final String value, final int count) {
    final StringBuilder result = new StringBuilder(value.length() * count);
    for (int index = 0; index < count; index++) {
      result.append(value);
    }
    return result.toString();
  }

  private static ToriiCanonicalRequestAuth canonicalRequestAuth() {
    try {
      final KeyPair keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
      return new ToriiCanonicalRequestAuth(
          "alice@universal",
          message -> {
            try {
              final Signature signer = Signature.getInstance("Ed25519");
              signer.initSign(keyPair.getPrivate());
              signer.update(message);
              return signer.sign();
            } catch (final Exception ex) {
              throw new IllegalStateException("failed to sign casting-proof request", ex);
            }
          },
          Long.valueOf(1_700_000_000_100L),
          "parliament-casting-proof");
    } catch (final Exception ex) {
      throw new IllegalStateException("failed to create casting-proof request signer", ex);
    }
  }

  private static ToriiCanonicalRequestAuth unpinnedCanonicalRequestAuth() {
    try {
      final KeyPair keyPair = KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
      return new ToriiCanonicalRequestAuth(
          "alice@universal",
          message -> {
            try {
              final Signature signer = Signature.getInstance("Ed25519");
              signer.initSign(keyPair.getPrivate());
              signer.update(message);
              return signer.sign();
            } catch (final Exception ex) {
              throw new IllegalStateException("failed to sign casting-proof request", ex);
            }
          });
    } catch (final Exception ex) {
      throw new IllegalStateException("failed to create casting-proof request signer", ex);
    }
  }

  private static byte[] filled(final int size, final int value) {
    final byte[] result = new byte[size];
    Arrays.fill(result, (byte) value);
    return result;
  }

  private static byte[] concat(final byte[] first, final byte[] second) {
    final byte[] output = Arrays.copyOf(first, first.length + second.length);
    System.arraycopy(second, 0, output, first.length, second.length);
    return output;
  }

  private static final class OneResponseExecutor implements HttpTransportExecutor {
    private final TransportResponse response;
    private TransportRequest lastRequest;
    private int requestCount;

    private OneResponseExecutor(final TransportResponse response) {
      this.response = Objects.requireNonNull(response, "response");
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      lastRequest = Objects.requireNonNull(request, "request");
      requestCount++;
      return CompletableFuture.completedFuture(response);
    }
  }

  private static final class SequenceResponseExecutor implements HttpTransportExecutor {
    private final byte[] responseBody;
    private final List<TransportRequest> requests = new java.util.ArrayList<>();

    private SequenceResponseExecutor(final byte[] responseBody) {
      this.responseBody = responseBody.clone();
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      requests.add(Objects.requireNonNull(request, "request"));
      final Map<String, List<String>> headers = new LinkedHashMap<>();
      headers.put("Content-Type", Collections.singletonList("application/x-norito"));
      headers.put(
          "Content-Length", Collections.singletonList(Integer.toString(responseBody.length)));
      return CompletableFuture.completedFuture(
          new TransportResponse(200, responseBody, "ok", headers));
    }
  }
}

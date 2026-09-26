// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNotNull;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.math.BigInteger;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.security.KeyPairGenerator;
import java.util.Collections;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.sdk.client.transport.TransportRequest;
import org.hyperledger.iroha.sdk.client.transport.TransportResponse;
import org.hyperledger.iroha.sdk.core.model.NetworkId;
import org.junit.jupiter.api.Test;

/** Java applications use the Kotlin-owned authenticated election tally query. */
final class ElectionTallyJavaConsumerTest {
  @Test
  void typedQueryKeepsUnsignedWeightsAndExactSnapshot() throws Exception {
    final BigInteger high = BigInteger.ONE.shiftLeft(64);
    final String hash = blockHash();
    final String json = "{\"evaluated_block_height\":18446744073709551615,"
        + "\"evaluated_block_hash\":\"" + hash + "\",\"finalized\":false,"
        + "\"tally\":[" + high + ",2]}";
    final CapturingExecutor executor = new CapturingExecutor(json);
    final NetworkId network = network();
    final ClientConfig config = ClientConfig.builder()
        .setBaseUri(URI.create("https://torii.example/api"))
        .setLocalSigningContext(new LocalSigningContext(network))
        .build();
    final ToriiCanonicalRequestAuth auth = new ToriiCanonicalRequestAuth(
        "alice@universal",
        RequestSigner.ed25519(KeyPairGenerator.getInstance("Ed25519").generateKeyPair().getPrivate()),
        1_700_000_000_100L, "java-election-tally");
    try (HttpClientTransport client = new HttpClientTransport(executor, config)) {
      final ElectionTallyV1 response = client.getElectionTally("election-1", auth).join();
      assertEquals(new BigInteger("18446744073709551615"), response.evaluatedBlockHeight);
      assertEquals(hash, response.evaluatedBlockHash);
      assertFalse(response.finalized);
      assertEquals(high, response.getTally().get(0));
      assertEquals(BigInteger.valueOf(2), response.getTally().get(1));
      assertThrows(UnsupportedOperationException.class,
          () -> response.getTally().add(BigInteger.valueOf(3)));
      assertNotNull(executor.lastRequest);
      final TransportRequest request = executor.lastRequest;
      assertEquals("POST", request.method);
      assertEquals("/api/v1/zk/vote/tally", request.uri.getRawPath());
      assertEquals("{\"election_id\":\"election-1\"}",
          new String(request.getBody(), StandardCharsets.UTF_8));
      assertNotNull(request.getHeaders().get(CanonicalRequestSigner.HEADER_SIGNATURE));
      assertEquals(1, executor.calls);
    }
  }

  @Test
  void malformedAggregateFailsBeforeTypedResult() throws Exception {
    final String json = "{\"evaluated_block_height\":7,"
        + "\"evaluated_block_hash\":\"" + blockHash() + "\","
        + "\"finalized\":true,\"tally\":[340282366920938463463374607431768211455,1]}";
    final CapturingExecutor executor = new CapturingExecutor(json);
    final ClientConfig config = ClientConfig.builder()
        .setBaseUri(URI.create("https://torii.example/api"))
        .setLocalSigningContext(new LocalSigningContext(network()))
        .build();
    final ToriiCanonicalRequestAuth auth = new ToriiCanonicalRequestAuth(
        "alice@universal",
        RequestSigner.ed25519(KeyPairGenerator.getInstance("Ed25519").generateKeyPair().getPrivate()),
        1_700_000_000_100L, "java-election-tally-invalid");
    try (HttpClientTransport client = new HttpClientTransport(executor, config)) {
      assertThrows(CompletionException.class, () -> client.getElectionTally("election-1", auth).join());
    }
  }

  private static NetworkId network() {
    final byte[] bytes = new byte[32];
    bytes[31] = 1;
    return NetworkId.fromBytes(bytes);
  }

  private static String blockHash() {
    final StringBuilder value = new StringBuilder(64);
    for (int index = 0; index < 32; index++) {
      value.append("ab");
    }
    return value.toString();
  }

  private static final class CapturingExecutor implements HttpTransportExecutor {
    private final byte[] payload;
    private int calls;
    private TransportRequest lastRequest;

    private CapturingExecutor(String payload) {
      this.payload = payload.getBytes(StandardCharsets.UTF_8);
    }

    @Override
    public CompletableFuture<TransportResponse> execute(TransportRequest request) {
      calls++;
      lastRequest = request;
      return CompletableFuture.completedFuture(new TransportResponse(
          200, payload, "OK",
          Collections.singletonMap("Content-Type", Collections.singletonList("application/json")),
          request.uri, false));
    }
  }
}

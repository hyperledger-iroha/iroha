// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;

import java.math.BigInteger;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.time.Duration;
import java.util.Collections;
import java.util.concurrent.CompletableFuture;
import org.hyperledger.iroha.sdk.client.transport.TransportRequest;
import org.hyperledger.iroha.sdk.client.transport.TransportResponse;
import org.junit.jupiter.api.Test;

/** Java applications consume the Kotlin-owned public DA proof-policy operation. */
public final class DaToriiClientJavaConsumerTest {
  @Test
  public void proofPoliciesUseCanonicalRouteAndReturnTypedBundle() {
    final String policyHash =
        "hash:0F923F0F972DB7373EFB38439B74651907459ECE1EF94564CCECF063F8893D85#C1CB";
    final String responseJson =
        "{\"version\":1,\"policy_hash\":\"" + policyHash + "\",\"policies\":[{"
            + "\"lane_id\":4294967295,\"dataspace_id\":18446744073709551615,"
            + "\"alias\":\"public-da\","
            + "\"proof_scheme\":{\"type\":\"MerkleSha256\",\"value\":null}}]}";
    final CapturingExecutor executor = new CapturingExecutor();
    final Duration timeout = Duration.ofSeconds(7);

    try (DaToriiClient client = DaToriiClient.builder()
        .baseUri(URI.create("https://torii.example/api/"))
        .timeout(timeout)
        .addHeader("X-Java-Consumer", "da-policy-test")
        .executor(executor)
        .build()) {
      final CompletableFuture<DaModels.ProofPolicyBundle> pending = client.getProofPolicies();
      assertEquals(1, executor.requests);
      assertFalse(pending.isDone(), "the typed future must await the injected transport");
      final TransportRequest request = executor.last;
      assertEquals("GET", request.method);
      assertEquals(URI.create("https://torii.example/api/v1/da/proof-policies"), request.uri);
      assertEquals(Collections.singletonList("application/json"), request.getHeaders().get("Accept"));
      assertEquals(Collections.singletonList("da-policy-test"),
          request.getHeaders().get("X-Java-Consumer"));
      assertEquals(0, request.getBody().length);
      assertFalse(request.getHeaders().containsKey("Content-Type"));
      assertEquals(timeout, request.timeout);
      assertEquals(Long.valueOf(8L * 1024 * 1024), request.maximumResponseBytes);

      executor.response.complete(new TransportResponse(
          200, responseJson.getBytes(StandardCharsets.UTF_8), "OK",
          Collections.singletonMap("Content-Type", Collections.singletonList("application/json")),
          request.uri, false));

      final DaModels.ProofPolicyBundle bundle = pending.join();
      assertEquals(1, bundle.getVersion());
      assertEquals(policyHash, bundle.getPolicyHash());
      assertEquals(1, bundle.getPolicies().size());
      final DaModels.ProofPolicy policy = bundle.getPolicies().get(0);
      assertEquals(4294967295L, policy.getLaneId());
      assertEquals(new BigInteger("18446744073709551615"), policy.getDataspaceId());
      assertEquals("public-da", policy.getAlias());
      assertEquals(DaModels.ProofScheme.MERKLE_SHA256, policy.getProofScheme());
      assertEquals(1, executor.requests);
    }
  }

  private static final class CapturingExecutor implements HttpTransportExecutor {
    private final CompletableFuture<TransportResponse> response = new CompletableFuture<>();
    private int requests;
    private TransportRequest last;

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      requests++;
      last = request;
      return response;
    }
  }
}

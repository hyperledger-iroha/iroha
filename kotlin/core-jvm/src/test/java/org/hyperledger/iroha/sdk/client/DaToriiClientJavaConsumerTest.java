// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;

import java.math.BigInteger;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.util.Arrays;
import org.hyperledger.iroha.sdk.address.AccountAddress;
import org.hyperledger.iroha.sdk.core.model.NetworkId;
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
            + "\"proof_scheme\":{\"type\":\"SorafsPorBlake3\",\"value\":null}}]}";
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
      assertEquals(DaModels.ProofScheme.SORAFS_POR_BLAKE3, policy.getProofScheme());
      assertEquals(1, executor.requests);
    }
  }

  @Test
  public void signedPinProofPreservesBothWitnessesWhenJavaSubmitsItForVerification() throws Exception {
    Path root = Paths.get("").toAbsolutePath();
    final Path relative = Paths.get("fixtures", "da", "ingest", "pin_intent_v1.json");
    while (root != null && !Files.isRegularFile(root.resolve(relative))) {
      root = root.getParent();
    }
    if (root == null) {
      throw new AssertionError("shared signed DA fixture is missing");
    }
    final String intent = new String(Files.readAllBytes(root.resolve(relative)), StandardCharsets.UTF_8);
    final String hash = "hash:0F923F0F972DB7373EFB38439B74651907459ECE1EF94564CCECF063F8893D85#C1CB";
    final String json = "{\"intent\":" + intent + ",\"location\":{\"block_height\":1,\"index_in_bundle\":0},"
        + "\"bundle_hash\":\"" + hash + "\",\"bundle_len\":1,\"root\":\"" + hash + "\",\"path\":[]}";
    final CapturingExecutor prover = new CapturingExecutor();
    final CapturingExecutor verifier = new CapturingExecutor();
    final URI base = URI.create("https://torii.example/");
    final KeyPair keys = KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
    final byte[] spki = keys.getPublic().getEncoded();
    final String account = AccountAddress.fromAccount(Arrays.copyOfRange(spki, spki.length - 32, spki.length),
        "ed25519").toI105Default();
    final LocalSigningContext context = new LocalSigningContext(NetworkId.parse(hash));
    final ToriiCanonicalRequestAuth auth = new ToriiCanonicalRequestAuth(account, RequestSigner.ed25519(keys.getPrivate()));
    try (DaToriiClient read = DaToriiClient.builder().executor(prover).baseUri(base).authenticatedProofs(context, auth).build();
         DaToriiClient check = DaToriiClient.builder().executor(verifier).baseUri(base).authenticatedProofs(context, auth).build()) {
      final CompletableFuture<DaModels.PinIntentProof> pending = read.provePinIntent();
      assertEquals(Collections.singletonList(AccountAddress.fromI105(account, null).canonicalHex()), prover.last.getHeaders().get(CanonicalRequestSigner.HEADER_ACCOUNT));
      prover.completeJson(json);
      final DaModels.PinIntentProof proof = pending.join();
      assertEquals(1, proof.getIntent().getAuthorization().getSignatures().size());
      assertEquals(1, proof.getIntent().getPinScopeAuthorization().getSignatures().size());
      final CompletableFuture<DaModels.VerifyResponse> result = check.verifyPinIntent(proof);
      assertEquals("/v1/da/pin-intents/verify", verifier.last.uri.getPath());
      assertEquals(DaJson.INSTANCE.parse(json.getBytes(StandardCharsets.UTF_8), "fixture"),
          DaJson.INSTANCE.parse(verifier.last.getBody(), "posted proof"));
      verifier.completeJson("{\"valid\":true,\"error\":null}");
      assertEquals(true, result.join().getValid());
    }
  }

  private static final class CapturingExecutor implements HttpTransportExecutor {
    private final CompletableFuture<TransportResponse> response = new CompletableFuture<>();
    private int requests;
    private TransportRequest last;

    private void completeJson(final String json) {
      response.complete(new TransportResponse(200, json.getBytes(StandardCharsets.UTF_8), "OK",
          Collections.singletonMap("Content-Type", Collections.singletonList("application/json")), last.uri, false));
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      requests++;
      last = request;
      return response;
    }
  }
}

// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client;

import static org.junit.jupiter.api.Assertions.assertArrayEquals;
import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertNull;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.io.IOException;
import java.math.BigInteger;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.Signature;
import java.time.Duration;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.concurrent.CompletableFuture;

import org.hyperledger.iroha.sdk.address.AccountAddress;
import org.hyperledger.iroha.sdk.client.transport.TransportRequest;
import org.hyperledger.iroha.sdk.client.transport.TransportResponse;
import org.hyperledger.iroha.sdk.core.model.NetworkId;
import org.junit.jupiter.api.Test;

/** Java consumers of the Kotlin data availability client. */
public final class DaToriiClientJavaConsumerTest {
  private static final String FIXTURE_HASH =
      "hash:0F923F0F972DB7373EFB38439B74651907459ECE1EF94564CCECF063F8893D85#C1CB";
  private static final URI TORII = URI.create("https://torii.example/api/");
  private static final List<String> SIGNATURE_HEADERS = Arrays.asList(
      CanonicalRequestSigner.HEADER_ACCOUNT, CanonicalRequestSigner.HEADER_SIGNATURE,
      CanonicalRequestSigner.HEADER_TIMESTAMP_MS, CanonicalRequestSigner.HEADER_NONCE);
  private static final String EMPTY_INTENTS = "{\"intents\":[],\"next_cursor\":null}";
  private static final String VERIFIED = "{\"valid\":true,\"error\":null}";

  @Test
  public void proofPoliciesUseCanonicalRouteAndReturnTypedBundle() {
    final String responseJson =
        "{\"version\":1,\"policy_hash\":\"" + FIXTURE_HASH + "\",\"policies\":[{"
            + "\"lane_id\":4294967295,\"dataspace_id\":18446744073709551615,"
            + "\"alias\":\"public-da\","
            + "\"proof_scheme\":{\"type\":\"SorafsPorBlake3\",\"value\":null}}]}";
    final CapturingExecutor executor = new CapturingExecutor();
    final Duration timeout = Duration.ofSeconds(7);

    try (DaToriiClient client = DaToriiClient.builder()
        .baseUri(TORII)
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

      executor.completeJson(responseJson);

      final DaModels.ProofPolicyBundle bundle = pending.join();
      assertEquals(1, bundle.getVersion());
      assertEquals(FIXTURE_HASH, bundle.getPolicyHash());
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
    final String json = pinProofJson(signedIntent());
    final CapturingExecutor prover = new CapturingExecutor();
    final CapturingExecutor verifier = new CapturingExecutor();
    final URI base = URI.create("https://torii.example/");
    final Signer signer = new Signer();
    try (DaToriiClient read = DaToriiClient.builder().executor(prover).baseUri(base)
            .authenticatedProofs(signer.context, signer.auth).build();
         DaToriiClient check = DaToriiClient.builder().executor(verifier).baseUri(base)
            .authenticatedProofs(signer.context, signer.auth).build()) {
      final CompletableFuture<DaModels.PinIntentProof> pending = read.provePinIntent();
      signer.assertSigned(prover.last);
      prover.completeJson(json);
      final DaModels.PinIntentProof proof = pending.join();
      assertEquals(1, proof.getIntent().getAuthorization().getSignatures().size());
      assertEquals(1, proof.getIntent().getPinScopeAuthorization().getSignatures().size());
      final CompletableFuture<DaModels.VerifyResponse> result = check.verifyPinIntent(proof);
      assertEquals("/v1/da/pin-intents/verify", verifier.last.uri.getPath());
      assertEquals(DaJson.INSTANCE.parse(json.getBytes(StandardCharsets.UTF_8), "fixture"),
          DaJson.INSTANCE.parse(verifier.last.getBody(), "posted proof"));
      verifier.completeJson(VERIFIED);
      assertTrue(result.join().getValid());
    }
  }

  @Test
  public void transportFactorySignsProofRequestsFromItsClientConfig() throws Exception {
    final Signer signer = new Signer();
    final CapturingExecutor executor = new CapturingExecutor();
    final ClientConfig config = ClientConfig.builder()
        .setBaseUri(TORII)
        .setLocalSigningContext(signer.context)
        .setCanonicalAuth(signer.auth)
        .build();
    try (HttpClientTransport transport = new HttpClientTransport(executor, config);
         DaToriiClient da = transport.newDaToriiClient()) {
      assertNull(executor.reply(da.listPinIntents(), EMPTY_INTENTS).getNextCursor());
      assertEquals(TORII.resolve("v1/da/pin-intents"), executor.last.uri);
      assertUnsigned(executor.last);

      final DaModels.PinIntentProof proof = executor.reply(da.provePinIntent(), pinProofJson(signedIntent()));
      assertEquals(TORII.resolve("v1/da/pin-intents/prove"), executor.last.uri);
      signer.assertSigned(executor.last);

      assertTrue(executor.reply(da.verifyPinIntent(proof), VERIFIED).getValid());
      assertEquals(TORII.resolve("v1/da/pin-intents/verify"), executor.last.uri);
      assertArrayEquals(proof.toJsonBytes(), executor.last.getBody());
      signer.assertSigned(executor.last);
      assertEquals(3, executor.requests);
    }
  }

  @Test
  public void transportFactoryWithoutAnAccountSignerKeepsListingsPublicAndRefusesProofs() {
    final CapturingExecutor executor = new CapturingExecutor();
    final ClientConfig config = ClientConfig.builder().setBaseUri(TORII).build();
    try (HttpClientTransport transport = new HttpClientTransport(executor, config);
         DaToriiClient da = transport.newDaToriiClient()) {
      assertNull(executor.reply(da.listPinIntents(), EMPTY_INTENTS).getNextCursor());
      assertEquals(TORII.resolve("v1/da/pin-intents"), executor.last.uri);
      assertUnsigned(executor.last);
      assertThrows(IllegalStateException.class, () -> da.provePinIntent());
      assertThrows(IllegalStateException.class, () -> da.proveCommitment());
      assertEquals(1, executor.requests, "no unsigned proof request may reach the executor");
    }
  }

  @Test
  public void callerSuppliedAccountSignatureHeadersAreRefusedBeforeDispatch() throws Exception {
    final Signer signer = new Signer();
    for (final String name : SIGNATURE_HEADERS) {
      final CapturingExecutor executor = new CapturingExecutor();
      try (DaToriiClient client = DaToriiClient.builder().executor(executor).baseUri(TORII)
          .addHeader(name.toLowerCase(Locale.ROOT), "caller-supplied")
          .authenticatedProofs(signer.context, signer.auth).build()) {
        assertThrows(IllegalArgumentException.class, () -> client.provePinIntent(), name);
        assertEquals(0, executor.requests, name);
      }
    }
  }

  @Test
  public void commitmentProofRoutesAreSignedForTheirExactRequest() throws Exception {
    final Signer signer = new Signer();
    final DaModels.Digest32 digest = DaModels.Digest32.fromHex(String.join("", Collections.nCopies(32, "10")));
    final DaModels.CommitmentRecord record = new DaModels.CommitmentRecord(
        1L, BigInteger.ONE, BigInteger.ONE, digest, digest, DaModels.ProofScheme.SORAFS_POR_BLAKE3, digest, null,
        new DaModels.RetentionPolicy(BigInteger.ONE, BigInteger.TEN, 1, DaModels.StorageClass.HOT, "da"),
        digest, String.join("", Collections.nCopies(64, "5A")));
    final DaModels.CommitmentProof proof = new DaModels.CommitmentProof(
        record, new DaModels.Location(BigInteger.ONE, 0L), FIXTURE_HASH, 1L, FIXTURE_HASH,
        Collections.<DaModels.MerklePathItem>emptyList());
    final CapturingExecutor executor = new CapturingExecutor();
    try (DaToriiClient client = DaToriiClient.builder().executor(executor).baseUri(TORII)
        .authenticatedProofs(signer.context, signer.auth).build()) {
      assertNull(executor.reply(client.proveCommitment(), "null"));
      assertEquals(TORII.resolve("v1/da/commitments/prove"), executor.last.uri);
      signer.assertSigned(executor.last);

      assertTrue(executor.reply(client.verifyCommitment(proof), VERIFIED).getValid());
      assertEquals(TORII.resolve("v1/da/commitments/verify"), executor.last.uri);
      assertArrayEquals(proof.toJsonBytes(), executor.last.getBody());
      signer.assertSigned(executor.last);
    }
  }

  @Test
  public void pinProofKeepsTheFullUnsignedEpochThroughProveAndVerify() throws Exception {
    final Signer signer = new Signer();
    final BigInteger maximum = new BigInteger("18446744073709551615");
    final String original = signedIntent();
    final String wide = original.replace("\"epoch\": 42", "\"epoch\": " + maximum);
    assertFalse(wide.equals(original), "the shared fixture must carry the epoch this case widens");
    final CapturingExecutor executor = new CapturingExecutor();
    try (DaToriiClient client = DaToriiClient.builder().executor(executor).baseUri(TORII)
        .authenticatedProofs(signer.context, signer.auth).build()) {
      final DaModels.PinIntentProof proof = executor.reply(client.provePinIntent(), pinProofJson(wide));
      assertEquals(TORII.resolve("v1/da/pin-intents/prove"), executor.last.uri);
      assertEquals(maximum, proof.getIntent().getEpoch());
      assertTrue(executor.reply(client.verifyPinIntent(proof), VERIFIED).getValid());
      assertEquals(TORII.resolve("v1/da/pin-intents/verify"), executor.last.uri);
      final Map<?, ?> posted = (Map<?, ?>) DaJson.INSTANCE.parse(executor.last.getBody(), "posted proof");
      final Map<?, ?> intent = (Map<?, ?>) posted.get("intent");
      assertEquals(maximum, intent.get("epoch"));
      assertEquals(DaJson.INSTANCE.parse(wide.getBytes(StandardCharsets.UTF_8), "signed intent"), intent);
      assertTrue(intent.containsKey("alias"));
      assertNull(intent.get("alias"));
      assertFalse(intent.containsKey("owner"), "the retired top-level owner must be absent");
    }
  }

  private static String signedIntent() throws IOException {
    Path root = Paths.get("").toAbsolutePath();
    final Path relative = Paths.get("fixtures", "da", "ingest", "pin_intent_v1.json");
    while (root != null && !Files.isRegularFile(root.resolve(relative))) {
      root = root.getParent();
    }
    if (root == null) {
      throw new AssertionError("shared signed DA fixture is missing");
    }
    return new String(Files.readAllBytes(root.resolve(relative)), StandardCharsets.UTF_8);
  }

  private static String pinProofJson(final String intent) {
    return "{\"intent\":" + intent + ",\"location\":{\"block_height\":1,\"index_in_bundle\":0},"
        + "\"bundle_hash\":\"" + FIXTURE_HASH + "\",\"bundle_len\":1,\"root\":\"" + FIXTURE_HASH + "\",\"path\":[]}";
  }

  private static void assertUnsigned(final TransportRequest request) {
    for (final String sent : request.getHeaders().keySet()) {
      for (final String name : SIGNATURE_HEADERS) {
        assertFalse(name.equalsIgnoreCase(sent), "a public route must not carry " + sent);
      }
    }
  }

  private static final class Signer {
    private final KeyPair keys;
    private final String account;
    private final NetworkId network = NetworkId.parse(FIXTURE_HASH);
    private final LocalSigningContext context = new LocalSigningContext(network);
    private final ToriiCanonicalRequestAuth auth;

    private Signer() throws Exception {
      keys = KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
      final byte[] spki = keys.getPublic().getEncoded();
      account = AccountAddress.fromAccount(Arrays.copyOfRange(spki, spki.length - 32, spki.length), "ed25519")
          .toI105Default();
      auth = new ToriiCanonicalRequestAuth(account, RequestSigner.ed25519(keys.getPrivate()));
    }

    private void assertSigned(final TransportRequest request) throws Exception {
      assertEquals(Collections.singletonList(AccountAddress.fromI105(account, null).canonicalHex()),
          request.getHeaders().get(CanonicalRequestSigner.HEADER_ACCOUNT));
      final byte[] message = CanonicalRequestSigner.canonicalRequestSignatureMessage(
          network, request.method, request.uri, request.getBody(),
          Long.parseLong(only(request, CanonicalRequestSigner.HEADER_TIMESTAMP_MS)),
          only(request, CanonicalRequestSigner.HEADER_NONCE));
      final Signature verifier = Signature.getInstance("Ed25519");
      verifier.initVerify(keys.getPublic());
      verifier.update(message);
      assertTrue(verifier.verify(Base64.getDecoder().decode(only(request, CanonicalRequestSigner.HEADER_SIGNATURE))),
          "the signature must verify over this exact network, method, URI and body");
    }

    private static String only(final TransportRequest request, final String name) {
      final List<String> values = request.getHeaders().get(name);
      assertEquals(1, values == null ? 0 : values.size(), name);
      return values.get(0);
    }
  }

  private static final class CapturingExecutor implements HttpTransportExecutor {
    private CompletableFuture<TransportResponse> response;
    private int requests;
    private TransportRequest last;

    private <T> T reply(final CompletableFuture<T> pending, final String json) {
      completeJson(json);
      return pending.join();
    }

    private void completeJson(final String json) {
      response.complete(new TransportResponse(200, json.getBytes(StandardCharsets.UTF_8), "OK",
          Collections.singletonMap("Content-Type", Collections.singletonList("application/json")), last.uri, false));
    }

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      requests++;
      last = request;
      response = new CompletableFuture<>();
      return response;
    }
  }
}

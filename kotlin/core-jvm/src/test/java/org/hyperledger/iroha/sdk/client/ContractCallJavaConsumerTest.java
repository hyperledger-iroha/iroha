// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.client;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertFalse;
import static org.junit.jupiter.api.Assertions.assertThrows;
import static org.junit.jupiter.api.Assertions.assertTrue;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.Signature;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CompletionException;
import org.hyperledger.iroha.sdk.address.AccountAddress;
import org.hyperledger.iroha.sdk.client.transport.RequestReplayPolicy;
import org.hyperledger.iroha.sdk.client.transport.TransportRequest;
import org.hyperledger.iroha.sdk.client.transport.TransportResponse;
import org.hyperledger.iroha.sdk.core.model.ContractInvocation;
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent;
import org.hyperledger.iroha.sdk.core.model.NetworkId;
import org.junit.jupiter.api.Test;

/** Java applications use the Kotlin-owned authenticated contract preparation API. */
public final class ContractCallJavaConsumerTest {
  @Test
  public void contractPreparationSignsExactBodyAndRejectsForeignAuthorityBeforeDispatch()
      throws Exception {
    final KeyPair key = KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
    final String authority = account(key);
    final byte[] networkBytes = new byte[32];
    networkBytes[31] = 1;
    final NetworkId network = NetworkId.fromBytes(networkBytes);
    final RejectingExecutor executor = new RejectingExecutor();
    final HttpClientTransport transport =
        new HttpClientTransport(
            executor,
            ClientConfig.builder()
                .setBaseUri(URI.create("https://torii.example/api"))
                .setLocalSigningContext(new LocalSigningContext(network))
                .build());
    final String address =
        "irohac1qyqqqqqqqqqqqq95fes93ygegsv5enq9mqsz6x4lv4vp9gg4yxgjw";
    final byte[] codeHash = new byte[32];
    codeHash[31] = 1;
    final ContractCallDraftIntent intent =
        new ContractCallDraftIntent(
            new ContractInvocation(address, codeHash, "ping", null), Collections.emptyMap());
    final FeePaymentIntent fee = FeePaymentIntent.authority(Collections.emptyList(), 5_000L);
    final long timestamp = 1_700_000_000_030L;
    final String nonce = "java-contract-prepare";
    final ToriiCanonicalRequestAuth auth =
        new ToriiCanonicalRequestAuth(
            authority, RequestSigner.ed25519(key.getPrivate()), timestamp, nonce);

    assertThrows(
        CompletionException.class,
        () -> transport.prepareContractCall(
            authority, fee, address, null, "ping", null, intent, auth).join());
    assertEquals(1, executor.requests, "an unavailable prepare must not trigger a retry");
    final TransportRequest request = executor.last;
    assertEquals("POST", request.method);
    assertEquals("https://torii.example/api/v1/contracts/call", request.uri.toString());
    assertEquals(RequestReplayPolicy.ONE_SHOT, request.replayPolicy);
    assertEquals(accountAddress(key).canonicalHex(),
        request.getHeaders().get(CanonicalRequestSigner.HEADER_ACCOUNT).get(0));
    final String body = new String(request.getBody(), StandardCharsets.UTF_8);
    assertFalse(body.contains("private_key"));
    assertFalse(body.contains("transaction_payload_b64"));
    final Signature verifier = Signature.getInstance("Ed25519");
    verifier.initVerify(key.getPublic());
    verifier.update(CanonicalRequestSigner.canonicalRequestSignatureMessage(
        network, request.method, request.uri, request.getBody(), timestamp, nonce));
    assertTrue(verifier.verify(Base64.getDecoder().decode(
        request.getHeaders().get(CanonicalRequestSigner.HEADER_SIGNATURE).get(0))));

    final KeyPair foreign = KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
    final ToriiCanonicalRequestAuth foreignAuth = new ToriiCanonicalRequestAuth(
        account(foreign), RequestSigner.ed25519(foreign.getPrivate()), timestamp, nonce);
    assertThrows(
        IllegalArgumentException.class,
        () -> transport.prepareContractCall(
            authority, fee, address, null, "ping", null, intent, foreignAuth));
    assertEquals(1, executor.requests, "foreign HTTP authority must fail before dispatch");
  }

  private static String account(final KeyPair key) throws Exception {
    return accountAddress(key).toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
  }

  private static AccountAddress accountAddress(final KeyPair key) throws Exception {
    final byte[] encoded = key.getPublic().getEncoded();
    return AccountAddress.fromAccount(
        Arrays.copyOfRange(encoded, encoded.length - 32, encoded.length), "ed25519");
  }

  private static final class RejectingExecutor implements HttpTransportExecutor {
    private int requests;
    private TransportRequest last;

    @Override
    public CompletableFuture<TransportResponse> execute(final TransportRequest request) {
      requests++;
      last = request;
      return CompletableFuture.completedFuture(new TransportResponse(
          503, "unavailable".getBytes(StandardCharsets.UTF_8), "unavailable",
          Collections.emptyMap(), request.uri, false));
    }
  }
}

package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.security.PublicKey;
import java.security.Signature;
import java.util.Base64;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.client.transport.TransportRequest;
import org.hyperledger.iroha.android.model.NetworkId;

/** Shared exact-network helpers for canonical HTTP request signing tests. */
final class CanonicalRequestSigningTestSupport {
  static final NetworkId VERIFYING_KEY_NETWORK_ID =
      NetworkId.parse(
          "hash:32C903E5B3497E34C2B844EBFE8A39C19E6CF8F95D44C1FFB8BA9DCB42F91149#A2F0");

  private CanonicalRequestSigningTestSupport() {}

  static ClientConfig signedClientConfig(final String baseUri) {
    return ClientConfig.builder()
        .setBaseUri(URI.create(baseUri))
        .setLocalSigningContext(new LocalSigningContext(VERIFYING_KEY_NETWORK_ID))
        .build();
  }

  static String canonicalAccountHeader(final String accountId) {
    try {
      return AccountAddress.parseEncodedIgnoringCurveSupport(accountId, null)
          .address
          .canonicalHex();
    } catch (final AccountAddress.AccountAddressException error) {
      throw new IllegalArgumentException("invalid canonical I105 test account", error);
    }
  }

  static void assertCanonicalSignature(
      final TransportRequest request,
      final PublicKey publicKey,
      final long timestampMs,
      final String nonce)
      throws Exception {
    final byte[] signature =
        Base64.getDecoder()
            .decode(request.headers().get(CanonicalRequestSigner.HEADER_SIGNATURE).get(0));
    final byte[] message =
        CanonicalRequestSigner.canonicalRequestSignatureMessage(
            VERIFYING_KEY_NETWORK_ID,
            request.method(),
            request.uri(),
            request.body(),
            timestampMs,
            nonce);
    final Signature verifier = Signature.getInstance("Ed25519");
    verifier.initVerify(publicKey);
    verifier.update(message);
    assert verifier.verify(signature) : "canonical request signature mismatch";
  }
}

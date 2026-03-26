package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.Signature;
import java.util.Base64;
import java.util.Map;

public final class CanonicalRequestSignerTests {

  private CanonicalRequestSignerTests() {}

  public static void main(final String[] args) throws Exception {
    canonicalQuerySortsPairs();
    headersCarryVerifiableSignature();
    System.out.println("[IrohaAndroid] Canonical request signer tests passed.");
  }

  private static void canonicalQuerySortsPairs() {
    final String rendered =
        CanonicalRequestSigner.canonicalQueryString("b=2&a=3&b=1&space=a+b");
    assert "a=3&b=1&b=2&space=a+b".equals(rendered)
        : "canonical query mismatch: " + rendered;
  }

  private static void headersCarryVerifiableSignature() throws Exception {
    final KeyPairGenerator generator = KeyPairGenerator.getInstance("Ed25519");
    final KeyPair keyPair = generator.generateKeyPair();
    final URI uri =
        new URI("http://localhost:8080/v1/accounts/sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB/assets?limit=5");
    final byte[] body = "{\"foo\":1}".getBytes(StandardCharsets.UTF_8);
    final long timestampMs = 1_717_171_717_000L;
    final String nonce = "android-canonical-nonce";

    final Map<String, String> headers =
        CanonicalRequestSigner.buildHeaders(
            "get", uri, body, "sorauロ1Npテユヱヌq11pウリ2ア5ヌヲiCJKjRヤzキNMNニケユPCウルFvオE9LBLB", keyPair.getPrivate(), timestampMs, nonce);
    final byte[] message =
        CanonicalRequestSigner.canonicalRequestSignatureMessage(
            "get", uri, body, timestampMs, nonce);
    final byte[] signature =
        Base64.getDecoder().decode(headers.get(CanonicalRequestSigner.HEADER_SIGNATURE));

    final Signature verifier = Signature.getInstance("Ed25519");
    verifier.initVerify(keyPair.getPublic());
    verifier.update(message);
    assert Long.toString(timestampMs)
            .equals(headers.get(CanonicalRequestSigner.HEADER_TIMESTAMP_MS))
        : "timestamp header mismatch";
    assert nonce.equals(headers.get(CanonicalRequestSigner.HEADER_NONCE))
        : "nonce header mismatch";
    assert verifier.verify(signature) : "signature verification failed";
  }
}

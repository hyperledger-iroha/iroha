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
        new URI("http://localhost:8080/v1/accounts/6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV/assets?limit=5");
    final byte[] body = "{\"foo\":1}".getBytes(StandardCharsets.UTF_8);

    final Map<String, String> headers =
        CanonicalRequestSigner.buildHeaders(
            "get", uri, body, "6cmzPVPX56eBcmRhnGrr3u5gDWjq3TbpwCwsNquHectzPZcFFA7THvV", keyPair.getPrivate());
    final byte[] message = CanonicalRequestSigner.canonicalRequestMessage("get", uri, body);
    final byte[] signature =
        Base64.getDecoder().decode(headers.get(CanonicalRequestSigner.HEADER_SIGNATURE));

    final Signature verifier = Signature.getInstance("Ed25519");
    verifier.initVerify(keyPair.getPublic());
    verifier.update(message);
    assert verifier.verify(signature) : "signature verification failed";
  }
}

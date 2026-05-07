package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.Signature;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.Map;

public final class CanonicalRequestSignerTests {

  private CanonicalRequestSignerTests() {}

  public static void main(final String[] args) throws Exception {
    canonicalQuerySortsPairs();
    headersCarryVerifiableSignature();
    unsignedBodyAuthJsonRemovesOnlyTopLevelProofFields();
    bodySignatureFieldsCarryVerifiableSignature();
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
        new URI("http://localhost:8080/v1/accounts/sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB/assets?limit=5");
    final byte[] body = "{\"foo\":1}".getBytes(StandardCharsets.UTF_8);
    final long timestampMs = 1_717_171_717_000L;
    final String nonce = "android-canonical-nonce";

    final Map<String, String> headers =
        CanonicalRequestSigner.buildHeaders(
            "get", uri, body, "sorauﾛ1Npﾃﾕヱﾇq11pｳﾘ2ｱ5ﾇｦiCJKjRﾔzｷNMNﾆｹﾕPCｳﾙFvｵE9LBLB", keyPair.getPrivate(), timestampMs, nonce);
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

  private static void unsignedBodyAuthJsonRemovesOnlyTopLevelProofFields() {
    final Map<String, Object> nested = new LinkedHashMap<>();
    nested.put(CanonicalRequestSigner.BODY_SIGNATURE_BASE64, "keep");
    final Map<String, Object> body = new LinkedHashMap<>();
    body.put("z", "last");
    body.put(CanonicalRequestSigner.BODY_SIGNATURE_BASE64, "remove");
    body.put("nested", nested);
    body.put(CanonicalRequestSigner.BODY_WITNESS_BASE64, "remove-too");
    body.put(CanonicalRequestSigner.BODY_ACCOUNT_ID, "alice");
    body.put(CanonicalRequestSigner.BODY_TIMESTAMP_MS, 7L);
    body.put(CanonicalRequestSigner.BODY_NONCE, "n");

    final String unsigned =
        new String(CanonicalRequestSigner.unsignedBodyAuthJson(body), StandardCharsets.UTF_8);

    assert "{\"account_id\":\"alice\",\"nested\":{\"signature_base64\":\"keep\"},\"nonce\":\"n\",\"timestamp_ms\":7,\"z\":\"last\"}"
            .equals(unsigned)
        : "unsigned body auth JSON mismatch: " + unsigned;
  }

  private static void bodySignatureFieldsCarryVerifiableSignature() throws Exception {
    final KeyPairGenerator generator = KeyPairGenerator.getInstance("Ed25519");
    final KeyPair keyPair = generator.generateKeyPair();
    final URI uri =
        new URI("https://torii.example/v1/offline/v2/keys/refill?b=2&a=1");
    final long timestampMs = 1_717_171_717_000L;
    final String nonce = "offline-body-nonce";
    final Map<String, Object> body = new LinkedHashMap<>();
    body.put("operation_id", "operation-1");

    final Map<String, Object> signed =
        CanonicalRequestSigner.withBodySignature(
            "post", uri, body, "alice", keyPair.getPrivate(), timestampMs, nonce);

    assert "alice".equals(signed.get(CanonicalRequestSigner.BODY_ACCOUNT_ID))
        : "account_id mismatch";
    assert Long.valueOf(timestampMs).equals(signed.get(CanonicalRequestSigner.BODY_TIMESTAMP_MS))
        : "timestamp mismatch";
    assert nonce.equals(signed.get(CanonicalRequestSigner.BODY_NONCE))
        : "nonce mismatch";
    assert !signed.containsKey(CanonicalRequestSigner.BODY_WITNESS_BASE64)
        : "witness proof must not be present for single-signature body auth";

    final byte[] signature =
        Base64.getDecoder()
            .decode((String) signed.get(CanonicalRequestSigner.BODY_SIGNATURE_BASE64));
    final byte[] message =
        CanonicalRequestSigner.canonicalBodyAuthSignatureMessage(
            "post", uri, signed, timestampMs, nonce);
    final Signature verifier = Signature.getInstance("Ed25519");
    verifier.initVerify(keyPair.getPublic());
    verifier.update(message);
    assert verifier.verify(signature) : "body auth signature verification failed";
  }
}

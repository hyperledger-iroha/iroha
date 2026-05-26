package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.concurrent.atomic.AtomicReference;

public final class CanonicalRequestSignerTests {

  private CanonicalRequestSignerTests() {}

  public static void main(final String[] args) throws Exception {
    canonicalQuerySortsPairs();
    callbackHeadersReceiveCanonicalMessage();
    unsignedBodyAuthJsonRemovesOnlyTopLevelProofFields();
    callbackBodySignatureReceivesCanonicalMessage();
    System.out.println("[IrohaAndroid] Canonical request signer tests passed.");
  }

  private static void canonicalQuerySortsPairs() {
    final String rendered =
        CanonicalRequestSigner.canonicalQueryString("b=2&a=3&b=1&space=a+b");
    assert "a=3&b=1&b=2&space=a+b".equals(rendered)
        : "canonical query mismatch: " + rendered;
  }

  private static void callbackHeadersReceiveCanonicalMessage() throws Exception {
    final URI uri = new URI("https://torii.example/v1/offline/keys/refill?b=2&a=1");
    final byte[] body = "{\"operation_id\":\"operation-1\"}".getBytes(StandardCharsets.UTF_8);
    final long timestampMs = 1_717_171_717_001L;
    final String nonce = "callback-header-nonce";
    final byte[] expectedMessage =
        CanonicalRequestSigner.canonicalRequestSignatureMessage(
            "post", uri, body, timestampMs, nonce);
    final byte[] expectedSignature = fakeSignature(expectedMessage);
    final AtomicReference<byte[]> signedMessage = new AtomicReference<>();
    final ToriiCanonicalRequestAuth auth =
        new ToriiCanonicalRequestAuth(
            "alice",
            message -> {
              signedMessage.set(Arrays.copyOf(message, message.length));
              return fakeSignature(message);
            },
            timestampMs,
            nonce);

    final Map<String, String> headers =
        CanonicalRequestSigner.buildHeaders("post", uri, body, auth, timestampMs, nonce);

    assert Arrays.equals(expectedMessage, signedMessage.get()) : "callback header message mismatch";
    assert "alice".equals(headers.get(CanonicalRequestSigner.HEADER_ACCOUNT))
        : "callback account header mismatch";
    assert Long.toString(timestampMs)
            .equals(headers.get(CanonicalRequestSigner.HEADER_TIMESTAMP_MS))
        : "callback timestamp header mismatch";
    assert nonce.equals(headers.get(CanonicalRequestSigner.HEADER_NONCE))
        : "callback nonce header mismatch";
    assert Base64.getEncoder()
            .encodeToString(expectedSignature)
            .equals(headers.get(CanonicalRequestSigner.HEADER_SIGNATURE))
        : "callback signature header mismatch";
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

  private static void callbackBodySignatureReceivesCanonicalMessage() throws Exception {
    final URI uri = new URI("https://torii.example/v1/offline/notes/issue");
    final long timestampMs = 1_717_171_717_002L;
    final String nonce = "callback-body-nonce";
    final AtomicReference<byte[]> signedMessage = new AtomicReference<>();
    final ToriiCanonicalRequestAuth auth =
        new ToriiCanonicalRequestAuth(
            "alice",
            message -> {
              signedMessage.set(Arrays.copyOf(message, message.length));
              return fakeSignature(message);
            },
            timestampMs,
            nonce);
    final Map<String, Object> body = new LinkedHashMap<>();
    body.put("operation_id", "operation-2");

    final Map<String, Object> signed =
        CanonicalRequestSigner.withBodySignature("post", uri, body, auth, timestampMs, nonce);
    final byte[] expectedMessage =
        CanonicalRequestSigner.canonicalBodyAuthSignatureMessage(
            "post", uri, signed, timestampMs, nonce);

    assert Arrays.equals(expectedMessage, signedMessage.get()) : "callback body message mismatch";
    assert "alice".equals(signed.get(CanonicalRequestSigner.BODY_ACCOUNT_ID))
        : "callback body account_id mismatch";
    assert Long.valueOf(timestampMs).equals(signed.get(CanonicalRequestSigner.BODY_TIMESTAMP_MS))
        : "callback body timestamp mismatch";
    assert nonce.equals(signed.get(CanonicalRequestSigner.BODY_NONCE))
        : "callback body nonce mismatch";
    assert Base64.getEncoder()
            .encodeToString(fakeSignature(expectedMessage))
            .equals(signed.get(CanonicalRequestSigner.BODY_SIGNATURE_BASE64))
        : "callback body signature mismatch";
    assert !signed.containsKey(CanonicalRequestSigner.BODY_WITNESS_BASE64)
        : "callback body auth must not include witness";
  }

  private static byte[] fakeSignature(final byte[] message) {
    final byte[] signature = new byte[64];
    for (int index = 0; index < signature.length; index++) {
      signature[index] = (byte) (message[index % message.length] ^ index);
    }
    return signature;
  }
}

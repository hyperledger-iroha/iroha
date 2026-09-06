package org.hyperledger.iroha.sdk.client;

import static org.junit.jupiter.api.Assertions.*;

import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.security.KeyPair;
import java.security.KeyPairGenerator;
import java.security.MessageDigest;
import java.security.Signature;
import java.util.Arrays;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.concurrent.atomic.AtomicReference;
import org.hyperledger.iroha.sdk.core.model.NetworkId;
import org.junit.jupiter.api.Test;

/** Java applications sign through the Kotlin-owned callback without exporting a key. */
final class RequestSigningJavaConsumerTest {
  private static final URI URI_VALUE = URI.create("https://torii.example/v1/accounts?b=2&a=1");
  private static final long TIMESTAMP = 1_717_171_717_001L;
  private static final String NONCE = "java-signer-1";

  private static NetworkId network() {
    final byte[] bytes = new byte[32];
    Arrays.fill(bytes, (byte) 1);
    return NetworkId.fromBytes(bytes);
  }

  private static Map<String, String> headers(RequestSigner signer, byte[] body) {
    final ToriiCanonicalRequestAuth auth =
        new ToriiCanonicalRequestAuth("alice@universal", signer, TIMESTAMP, NONCE);
    return CanonicalRequestSigner.buildHeaders(
        network(), "post", URI_VALUE, body, auth.accountId, auth.signer,
        auth.timestampMs, auth.nonce);
  }

  @Test
  void opaqueSignerReceivesExactCanonicalMessageAndReturnedBytesAreIsolated() throws Exception {
    final byte[] body = "{\"operation_id\":\"operation-1\"}".getBytes(StandardCharsets.UTF_8);
    final byte[] originalBody = body.clone();
    final byte[] returned = new byte[64];
    Arrays.fill(returned, (byte) 7);
    final AtomicReference<byte[]> captured = new AtomicReference<>();
    final Map<String, String> signed = headers(message -> {
      captured.set(message.clone());
      Arrays.fill(message, (byte) 0);
      return returned;
    }, body);

    final StringBuilder digest = new StringBuilder();
    for (byte value : MessageDigest.getInstance("SHA-256").digest(body)) {
      digest.append(String.format(java.util.Locale.ROOT, "%02x", value & 0xff));
    }
    final byte[] domain = "iroha.app.request.network.v1\0".getBytes(StandardCharsets.UTF_8);
    final byte[] request =
        ("POST\n/v1/accounts\na=1&b=2\n" + digest + "\n" + TIMESTAMP + "\n" + NONCE)
            .getBytes(StandardCharsets.UTF_8);
    final byte[] expected = new byte[domain.length + 32 + request.length];
    System.arraycopy(domain, 0, expected, 0, domain.length);
    System.arraycopy(network().bytes(), 0, expected, domain.length, 32);
    System.arraycopy(request, 0, expected, domain.length + 32, request.length);
    assertArrayEquals(expected, captured.get());
    assertArrayEquals(originalBody, body);
    final String signature = Base64.getEncoder().encodeToString(returned);
    Arrays.fill(returned, (byte) 0);
    assertEquals(signature, signed.get(CanonicalRequestSigner.HEADER_SIGNATURE));
    assertEquals("alice@universal", signed.get(CanonicalRequestSigner.HEADER_ACCOUNT));
  }

  @Test
  void signatureBoundsRejectNullEmptyZeroAndOversizedCallbackResults() {
    for (byte[] invalid : new byte[][] {
        null, new byte[0], new byte[64],
        new byte[CanonicalRequestSigner.CANONICAL_REQUEST_MAX_SIGNATURE_BYTES_V1 + 1]
    }) {
      assertThrows(IllegalStateException.class, () -> headers(message -> invalid, new byte[0]));
    }
    final byte[] maximum = new byte[CanonicalRequestSigner.CANONICAL_REQUEST_MAX_SIGNATURE_BYTES_V1];
    Arrays.fill(maximum, (byte) 1);
    assertArrayEquals(maximum, Base64.getDecoder().decode(
        headers(message -> maximum, new byte[0]).get(CanonicalRequestSigner.HEADER_SIGNATURE)));
  }

  @Test
  void providerFailurePropagatesWithoutASignatureOrSecondAttempt() {
    final IllegalStateException unavailable = new IllegalStateException("signer unavailable");
    final int[] attempts = {0};
    assertSame(unavailable, assertThrows(IllegalStateException.class, () -> headers(message -> {
      attempts[0]++;
      throw unavailable;
    }, new byte[0])));
    assertEquals(1, attempts[0]);
  }

  @Test
  void softwareSignerUsesTheSameCanonicalMessage() throws Exception {
    final KeyPair keys = KeyPairGenerator.getInstance("Ed25519").generateKeyPair();
    final byte[] body = "request".getBytes(StandardCharsets.UTF_8);
    final Map<String, String> signed = headers(RequestSigner.ed25519(keys.getPrivate()), body);
    final Signature verifier = Signature.getInstance("Ed25519");
    verifier.initVerify(keys.getPublic());
    verifier.update(CanonicalRequestSigner.canonicalRequestSignatureMessage(
        network(), "post", URI_VALUE, body, TIMESTAMP, NONCE));
    assertTrue(verifier.verify(Base64.getDecoder().decode(
        signed.get(CanonicalRequestSigner.HEADER_SIGNATURE))));
  }

  @Test
  void bodyAuthenticationUsesTheOpaqueSignerAndRejectsInvalidOutput() {
    final Map<String, Object> body = new LinkedHashMap<>();
    body.put("operation_id", "operation-1");
    final AtomicReference<byte[]> captured = new AtomicReference<>();
    final RequestSigner signer = message -> {
      captured.set(message.clone());
      return new byte[] {1, 2, 3};
    };
    final Map<String, Object> signed = CanonicalRequestSigner.withBodySignature(
        network(), "post", URI_VALUE, body, "alice@universal", signer, TIMESTAMP, NONCE);
    assertEquals("AQID", signed.get(CanonicalRequestSigner.BODY_SIGNATURE_BASE64));
    assertArrayEquals(CanonicalRequestSigner.canonicalBodyAuthSignatureMessage(
        network(), "post", URI_VALUE, signed, TIMESTAMP, NONCE), captured.get());
    assertEquals(1, body.size());
    assertThrows(IllegalStateException.class, () -> CanonicalRequestSigner.withBodySignature(
        network(), "post", URI_VALUE, body, "alice@universal", message -> new byte[64],
        TIMESTAMP, NONCE));
  }
}

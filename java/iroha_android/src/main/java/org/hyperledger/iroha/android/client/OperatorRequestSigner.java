package org.hyperledger.iroha.android.client;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.security.SecureRandom;
import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.Locale;
import java.util.Map;
import java.util.Set;

/** Exact-request header builder for operator-authenticated Torii APIs. */
public final class OperatorRequestSigner {
  public static final String HEADER_PUBLIC_KEY = "X-Iroha-Operator-Public-Key";
  public static final String HEADER_TIMESTAMP_MS = "X-Iroha-Operator-Timestamp-Ms";
  public static final String HEADER_NONCE = "X-Iroha-Operator-Nonce";
  public static final String HEADER_SIGNATURE = "X-Iroha-Operator-Signature";

  private static final byte[] DOMAIN =
      "iroha.operator.http-request.network.v1\0".getBytes(StandardCharsets.UTF_8);
  private static final SecureRandom NONCE_RANDOM = new SecureRandom();
  private static final Set<String> FORBIDDEN_HEADERS =
      Collections.unmodifiableSet(
          new LinkedHashSet<>(
              Arrays.asList(
                  "authorization",
                  "x-api-token",
                  "x-iroha-account",
                  "x-iroha-signature",
                  "x-iroha-timestamp-ms",
                  "x-iroha-nonce",
                  "x-iroha-witness",
                  HEADER_PUBLIC_KEY.toLowerCase(Locale.ROOT),
                  HEADER_TIMESTAMP_MS.toLowerCase(Locale.ROOT),
                  HEADER_NONCE.toLowerCase(Locale.ROOT),
                  HEADER_SIGNATURE.toLowerCase(Locale.ROOT))));

  private OperatorRequestSigner() {}

  /** Rejects token, account, witness, and precomputed operator fallback headers. */
  public static void requireGeneratedAuth(final Map<String, ?> headers) {
    if (headers == null) {
      return;
    }
    for (final String name : headers.keySet()) {
      if (name != null && FORBIDDEN_HEADERS.contains(name.toLowerCase(Locale.ROOT))) {
        throw new IllegalArgumentException(
            "operator GET requires generated signing; header " + name + " is not accepted");
      }
    }
  }

  /** Builds the exact NetworkId-bound message for deterministic tests and signers. */
  public static byte[] signatureMessage(
      final OperatorSigningContext context,
      final String method,
      final URI uri,
      final byte[] body,
      final long timestampMs,
      final String nonce) {
    if (timestampMs < 0L) {
      throw new IllegalArgumentException("operator timestamp must be non-negative");
    }
    if (nonce == null || nonce.isEmpty() || !nonce.equals(nonce.trim())) {
      throw new IllegalArgumentException("operator nonce must be exact and non-empty");
    }
    try {
      final ByteArrayOutputStream output = new ByteArrayOutputStream();
      output.write(DOMAIN);
      output.write(context.networkId().bytes());
      output.write(CanonicalRequestSigner.canonicalRequestMessage(method, uri, body));
      output.write(("\n" + timestampMs + "\n" + nonce).getBytes(StandardCharsets.UTF_8));
      return output.toByteArray();
    } catch (final IOException ex) {
      throw new IllegalStateException("failed to build operator signature message", ex);
    }
  }

  /** Builds a fresh operator signature quartet for one finalized request target. */
  public static Map<String, String> buildHeaders(
      final OperatorSigningContext context,
      final String method,
      final URI uri,
      final byte[] body) {
    final long timestampMs = System.currentTimeMillis();
    final byte[] nonceBytes = new byte[16];
    NONCE_RANDOM.nextBytes(nonceBytes);
    final String nonce = Base64.getUrlEncoder().withoutPadding().encodeToString(nonceBytes);
    final byte[] signature =
        context.sign(signatureMessage(context, method, uri, body, timestampMs, nonce));
    final Map<String, String> headers = new LinkedHashMap<>();
    headers.put(HEADER_PUBLIC_KEY, context.publicKey());
    headers.put(HEADER_TIMESTAMP_MS, Long.toString(timestampMs));
    headers.put(HEADER_NONCE, nonce);
    headers.put(HEADER_SIGNATURE, Base64.getEncoder().encodeToString(signature));
    return Collections.unmodifiableMap(headers);
  }
}

package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.net.URLDecoder;
import java.net.URLEncoder;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.SecureRandom;
import java.util.AbstractMap;
import java.util.ArrayList;
import java.util.Base64;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;

/**
 * Builds canonical request signatures for Torii app endpoints.
 */
public final class CanonicalRequestSigner {

  public static final String HEADER_ACCOUNT = "X-Iroha-Account";
  public static final String HEADER_SIGNATURE = "X-Iroha-Signature";
  public static final String HEADER_TIMESTAMP_MS = "X-Iroha-Timestamp-Ms";
  public static final String HEADER_NONCE = "X-Iroha-Nonce";
  public static final String BODY_ACCOUNT_ID = "account_id";
  public static final String BODY_TIMESTAMP_MS = "timestamp_ms";
  public static final String BODY_NONCE = "nonce";
  public static final String BODY_SIGNATURE_BASE64 = "signature_base64";
  public static final String BODY_WITNESS_BASE64 = "witness_base64";
  private static final SecureRandom NONCE_RANDOM = new SecureRandom();

  private CanonicalRequestSigner() {}

  /**
   * Canonicalise a raw query string by decoding, sorting, and re-encoding.
   */
  public static String canonicalQueryString(final String raw) {
    if (raw == null || raw.isEmpty()) {
      return "";
    }
    final List<Map.Entry<String, String>> pairs = new ArrayList<>();
    for (final String component : raw.split("&", -1)) {
      final String[] kv = component.split("=", 2);
      final String key = kv.length > 0 ? kv[0] : "";
      final String value = kv.length > 1 ? kv[1] : "";
      pairs.add(
          new AbstractMap.SimpleEntry<>(urlDecode(key), urlDecode(value)));
    }
    pairs.sort(
        (a, b) -> {
          final int keyCompare = a.getKey().compareTo(b.getKey());
          if (keyCompare != 0) {
            return keyCompare;
          }
          return a.getValue().compareTo(b.getValue());
        });
    final StringBuilder builder = new StringBuilder();
    for (int i = 0; i < pairs.size(); i++) {
      final Map.Entry<String, String> pair = pairs.get(i);
      if (i > 0) {
        builder.append('&');
      }
      builder.append(urlEncode(pair.getKey()));
      builder.append('=');
      builder.append(urlEncode(pair.getValue()));
    }
    return builder.toString();
  }

  /**
   * Build canonical request bytes for signing.
   */
  public static byte[] canonicalRequestMessage(
      final String method, final URI uri, final byte[] body) {
    final String query = canonicalQueryString(uri.getRawQuery());
    final String path = uri.getRawPath() == null ? "" : uri.getRawPath();
    final byte[] bodyBytes = body == null ? new byte[0] : body;
    final byte[] digest;
    try {
      digest = MessageDigest.getInstance("SHA-256").digest(bodyBytes);
    } catch (Exception ex) {
      throw new IllegalStateException("sha256 unavailable", ex);
    }
    final String rendered =
        method.toUpperCase(Locale.ROOT)
            + "\n"
            + path
            + "\n"
            + query
            + "\n"
            + hex(digest);
    return rendered.getBytes(StandardCharsets.UTF_8);
  }

  /**
   * Build canonical request bytes for signing with freshness metadata.
   */
  public static byte[] canonicalRequestSignatureMessage(
      final String method,
      final URI uri,
      final byte[] body,
      final long timestampMs,
      final String nonce) {
    if (nonce == null || nonce.trim().isEmpty()) {
      throw new IllegalArgumentException("nonce is required");
    }
    final String rendered =
        new String(canonicalRequestMessage(method, uri, body), StandardCharsets.UTF_8)
            + "\n"
            + timestampMs
            + "\n"
            + nonce;
    return rendered.getBytes(StandardCharsets.UTF_8);
  }

  /**
   * Build unsigned canonical JSON bytes for body-auth endpoints.
   */
  public static byte[] unsignedBodyAuthJson(final Map<String, Object> bodyFields) {
    final Map<String, Object> unsigned = new LinkedHashMap<>(bodyFields);
    unsigned.remove(BODY_SIGNATURE_BASE64);
    unsigned.remove(BODY_WITNESS_BASE64);
    return JsonEncoder.encode(unsigned).getBytes(StandardCharsets.UTF_8);
  }

  /**
   * Build body-auth canonical request bytes plus freshness metadata.
   */
  public static byte[] canonicalBodyAuthSignatureMessage(
      final String method,
      final URI uri,
      final Map<String, Object> bodyFields,
      final long timestampMs,
      final String nonce) {
    return canonicalRequestSignatureMessage(
        method, uri, unsignedBodyAuthJson(bodyFields), timestampMs, nonce);
  }

  /**
   * Build the top-level fields required for single-signature body auth with callback signing.
   */
  public static Map<String, Object> buildBodySignatureFields(
      final String method,
      final URI uri,
      final Map<String, Object> bodyFields,
      final ToriiCanonicalRequestAuth canonicalAuth,
      final long timestampMs,
      final String nonce) {
    if (canonicalAuth == null) {
      throw new IllegalArgumentException("canonicalAuth is required");
    }
    return buildBodySignatureFields(
        method,
        uri,
        bodyFields,
        canonicalAuth.accountId(),
        canonicalAuth::sign,
        timestampMs,
        nonce);
  }

  private static Map<String, Object> buildBodySignatureFields(
      final String method,
      final URI uri,
      final Map<String, Object> bodyFields,
      final String accountId,
      final CanonicalRequestSignatureProvider signatureProvider,
      final long timestampMs,
      final String nonce) {
    final Map<String, Object> unsigned =
        bodyWithBodyAuthFreshness(bodyFields, accountId, timestampMs, nonce);
    final byte[] message =
        canonicalBodyAuthSignatureMessage(method, uri, unsigned, timestampMs, nonce);
    final byte[] signatureBytes = signCanonicalMessage(signatureProvider, message);
    final Map<String, Object> fields = new LinkedHashMap<>();
    fields.put(BODY_ACCOUNT_ID, accountId);
    fields.put(BODY_TIMESTAMP_MS, timestampMs);
    fields.put(BODY_NONCE, nonce);
    fields.put(BODY_SIGNATURE_BASE64, Base64.getEncoder().encodeToString(signatureBytes));
    return fields;
  }

  /**
   * Return a copy of {@code bodyFields} carrying single-signature body auth.
   */
  public static Map<String, Object> withBodySignature(
      final String method,
      final URI uri,
      final Map<String, Object> bodyFields,
      final ToriiCanonicalRequestAuth canonicalAuth,
      final long timestampMs,
      final String nonce) {
    final Map<String, Object> body = new LinkedHashMap<>(bodyFields);
    body.remove(BODY_WITNESS_BASE64);
    body.putAll(buildBodySignatureFields(method, uri, body, canonicalAuth, timestampMs, nonce));
    return body;
  }

  /**
   * Return a copy of {@code bodyFields} carrying a prebuilt multisig witness body auth proof.
   */
  public static Map<String, Object> withBodyWitness(
      final Map<String, Object> bodyFields,
      final String accountId,
      final long timestampMs,
      final String nonce,
      final String witnessBase64) {
    if (witnessBase64 == null || witnessBase64.trim().isEmpty()) {
      throw new IllegalArgumentException("witnessBase64 is required");
    }
    final Map<String, Object> body =
        bodyWithBodyAuthFreshness(bodyFields, accountId, timestampMs, nonce);
    body.put(BODY_WITNESS_BASE64, witnessBase64);
    return body;
  }

  /**
   * Build canonical signing headers including freshness metadata.
   */
  public static Map<String, String> buildHeaders(
      final String method,
      final URI uri,
      final byte[] body,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    return buildHeaders(
        method,
        uri,
        body,
        canonicalAuth,
        System.currentTimeMillis(),
        randomNonce());
  }

  /**
   * Build canonical signing headers with explicit freshness metadata.
   */
  public static Map<String, String> buildHeaders(
      final String method,
      final URI uri,
      final byte[] body,
      final ToriiCanonicalRequestAuth canonicalAuth,
      final long timestampMs,
      final String nonce) {
    if (canonicalAuth == null) {
      throw new IllegalArgumentException("canonicalAuth is required");
    }
    return buildHeaders(
        method,
        uri,
        body,
        canonicalAuth.accountId(),
        canonicalAuth::sign,
        timestampMs,
        nonce);
  }

  private static Map<String, String> buildHeaders(
      final String method,
      final URI uri,
      final byte[] body,
      final String accountId,
      final CanonicalRequestSignatureProvider signatureProvider,
      final long timestampMs,
      final String nonce) {
    if (accountId == null || accountId.trim().isEmpty()) {
      throw new IllegalArgumentException("accountId is required");
    }
    if (nonce == null || nonce.trim().isEmpty()) {
      throw new IllegalArgumentException("nonce is required");
    }
    final byte[] message =
        canonicalRequestSignatureMessage(method, uri, body, timestampMs, nonce);
    final byte[] signatureBytes = signCanonicalMessage(signatureProvider, message);
    final Map<String, String> headers = new HashMap<>();
    headers.put(HEADER_ACCOUNT, accountId);
    headers.put(HEADER_SIGNATURE, Base64.getEncoder().encodeToString(signatureBytes));
    headers.put(HEADER_TIMESTAMP_MS, Long.toString(timestampMs));
    headers.put(HEADER_NONCE, nonce);
    return headers;
  }

  private static Map<String, Object> bodyWithBodyAuthFreshness(
      final Map<String, Object> bodyFields,
      final String accountId,
      final long timestampMs,
      final String nonce) {
    if (accountId == null || accountId.trim().isEmpty()) {
      throw new IllegalArgumentException("accountId is required");
    }
    if (nonce == null || nonce.trim().isEmpty()) {
      throw new IllegalArgumentException("nonce is required");
    }
    final Map<String, Object> body = new LinkedHashMap<>(bodyFields);
    body.put(BODY_ACCOUNT_ID, accountId);
    body.put(BODY_TIMESTAMP_MS, timestampMs);
    body.put(BODY_NONCE, nonce);
    body.remove(BODY_SIGNATURE_BASE64);
    body.remove(BODY_WITNESS_BASE64);
    return body;
  }

  private static byte[] signCanonicalMessage(
      final CanonicalRequestSignatureProvider signatureProvider, final byte[] message) {
    if (signatureProvider == null) {
      throw new IllegalArgumentException("signatureProvider is required");
    }
    final byte[] signature = signatureProvider.sign(message);
    if (signature == null || signature.length == 0) {
      throw new IllegalStateException("canonical request signature is empty");
    }
    return signature;
  }

  private static String randomNonce() {
    final byte[] bytes = new byte[16];
    NONCE_RANDOM.nextBytes(bytes);
    return hex(bytes);
  }

  private static String urlEncode(final String value) {
    try {
      return URLEncoder.encode(value, StandardCharsets.UTF_8.toString());
    } catch (Exception ex) {
      throw new IllegalStateException("failed to encode query component", ex);
    }
  }

  private static String urlDecode(final String value) {
    try {
      return URLDecoder.decode(value, StandardCharsets.UTF_8.toString());
    } catch (Exception ex) {
      throw new IllegalArgumentException("failed to decode query component", ex);
    }
  }

  private static String hex(final byte[] bytes) {
    final char[] digits = "0123456789abcdef".toCharArray();
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      final int value = b & 0xff;
      builder.append(digits[value >>> 4]);
      builder.append(digits[value & 0x0f]);
    }
    return builder.toString();
  }
}

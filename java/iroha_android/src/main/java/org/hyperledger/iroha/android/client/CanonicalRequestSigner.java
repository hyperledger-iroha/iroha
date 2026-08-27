package org.hyperledger.iroha.android.client;

import java.io.ByteArrayOutputStream;
import java.net.URI;
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
import java.util.Objects;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.model.NetworkId;

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
  public static final int CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1 = 64;
  public static final int CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1 = 64 * 1024;
  public static final int CANONICAL_REQUEST_MAX_METHOD_BYTES_V1 = 32;
  public static final int CANONICAL_REQUEST_MAX_PATH_BYTES_V1 = 64 * 1024;
  public static final int CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1 = 36 * 1024;
  public static final int CANONICAL_REQUEST_MAX_SIGNATURE_BYTES_V1 = 3309;
  private static final String BODY_WITNESS_BASE64 = "witness_base64";
  private static final SecureRandom NONCE_RANDOM = new SecureRandom();
  private static final byte[] NETWORK_DOMAIN =
      "iroha.app.request.network.v1\0".getBytes(StandardCharsets.UTF_8);

  private CanonicalRequestSigner() {}

  /**
   * Canonicalise a raw query string by decoding, sorting, and re-encoding.
   */
  public static String canonicalQueryString(final String raw) {
    if (raw == null || raw.isEmpty()) {
      return "";
    }
    if (raw.getBytes(StandardCharsets.UTF_8).length
        > CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1) {
      throw new IllegalArgumentException(
          "canonical request query exceeds "
              + CANONICAL_REQUEST_MAX_RAW_QUERY_BYTES_V1
              + " raw UTF-8 bytes");
    }
    final List<Map.Entry<String, String>> pairs = new ArrayList<>();
    for (final String component : raw.split("&", -1)) {
      if (component.isEmpty()) {
        continue;
      }
      if (pairs.size() >= CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1) {
        throw new IllegalArgumentException(
            "canonical request query exceeds "
                + CANONICAL_REQUEST_MAX_QUERY_PAIRS_V1
                + " pairs");
      }
      final String[] kv = component.split("=", 2);
      final String key = kv.length > 0 ? kv[0] : "";
      final String value = kv.length > 1 ? kv[1] : "";
      pairs.add(
          new AbstractMap.SimpleEntry<>(urlDecode(key), urlDecode(value)));
    }
    pairs.sort(
        (a, b) -> {
          final int keyCompare = compareUtf8(a.getKey(), b.getKey());
          if (keyCompare != 0) {
            return keyCompare;
          }
          return compareUtf8(a.getValue(), b.getValue());
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
    final String exactMethod = requireHttpMethodToken(method);
    final String path = requireCanonicalRawPath(uri);
    final String query = canonicalQueryString(uri.getRawQuery());
    final byte[] bodyBytes = body == null ? new byte[0] : body;
    final byte[] digest;
    try {
      digest = MessageDigest.getInstance("SHA-256").digest(bodyBytes);
    } catch (Exception ex) {
      throw new IllegalStateException("sha256 unavailable", ex);
    }
    final String rendered =
        exactMethod.toUpperCase(Locale.ROOT)
            + "\n"
            + path
            + "\n"
            + query
            + "\n"
            + hex(digest);
    return rendered.getBytes(StandardCharsets.UTF_8);
  }

  /**
   * Build canonical request bytes bound to an exact network and freshness metadata.
   */
  public static byte[] canonicalRequestSignatureMessage(
      final NetworkId networkId,
      final String method,
      final URI uri,
      final byte[] body,
      final long timestampMs,
      final String nonce) {
    if (timestampMs < 0) {
      throw new IllegalArgumentException("timestampMs must be non-negative");
    }
    requireExactNonBlank(nonce, "nonce");
    Objects.requireNonNull(networkId, "networkId");
    return concat(
        NETWORK_DOMAIN,
        networkId.bytes(),
        canonicalRequestMessage(method, uri, body),
        ("\n" + timestampMs + "\n" + nonce).getBytes(StandardCharsets.UTF_8));
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
      final NetworkId networkId,
      final String method,
      final URI uri,
      final Map<String, Object> bodyFields,
      final long timestampMs,
      final String nonce) {
    return canonicalRequestSignatureMessage(
        networkId, method, uri, unsignedBodyAuthJson(bodyFields), timestampMs, nonce);
  }

  /**
   * Build the top-level fields required for single-signature body auth with callback signing.
   */
  public static Map<String, Object> buildBodySignatureFields(
      final NetworkId networkId,
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
        networkId,
        method,
        uri,
        bodyFields,
        canonicalAuth.accountId(),
        canonicalAuth::sign,
        timestampMs,
        nonce);
  }

  private static Map<String, Object> buildBodySignatureFields(
      final NetworkId networkId,
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
        canonicalBodyAuthSignatureMessage(networkId, method, uri, unsigned, timestampMs, nonce);
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
      final NetworkId networkId,
      final String method,
      final URI uri,
      final Map<String, Object> bodyFields,
      final ToriiCanonicalRequestAuth canonicalAuth,
      final long timestampMs,
      final String nonce) {
    final Map<String, Object> body = new LinkedHashMap<>(bodyFields);
    body.remove(BODY_WITNESS_BASE64);
    body.putAll(
        buildBodySignatureFields(
            networkId, method, uri, body, canonicalAuth, timestampMs, nonce));
    return body;
  }

  /**
   * Build canonical signing headers including freshness metadata.
   *
   * <p>Canonical I105 identities are emitted as lowercase canonical hex in {@link
   * #HEADER_ACCOUNT}; canonical lowercase ASCII aliases are emitted unchanged.
   */
  public static Map<String, String> buildHeaders(
      final NetworkId networkId,
      final String method,
      final URI uri,
      final byte[] body,
      final ToriiCanonicalRequestAuth canonicalAuth) {
    return buildHeaders(
        networkId,
        method,
        uri,
        body,
        canonicalAuth,
        System.currentTimeMillis(),
        randomNonce());
  }

  /**
   * Build canonical signing headers with explicit freshness metadata.
   *
   * <p>Canonical I105 identities are emitted as lowercase canonical hex in {@link
   * #HEADER_ACCOUNT}; canonical lowercase ASCII aliases are emitted unchanged.
   */
  public static Map<String, String> buildHeaders(
      final NetworkId networkId,
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
        networkId,
        method,
        uri,
        body,
        canonicalAuth.accountId(),
        canonicalAuth::sign,
        timestampMs,
        nonce);
  }

  private static Map<String, String> buildHeaders(
      final NetworkId networkId,
      final String method,
      final URI uri,
      final byte[] body,
      final String accountId,
      final CanonicalRequestSignatureProvider signatureProvider,
      final long timestampMs,
      final String nonce) {
    final String canonicalAccountId = requireCanonicalAccountId(accountId);
    requireExactNonBlank(nonce, "nonce");
    final String accountHeader = canonicalAccountHeaderValue(canonicalAccountId);
    final byte[] message =
        canonicalRequestSignatureMessage(networkId, method, uri, body, timestampMs, nonce);
    final byte[] signatureBytes = signCanonicalMessage(signatureProvider, message);
    final Map<String, String> headers = new HashMap<>();
    headers.put(HEADER_ACCOUNT, accountHeader);
    headers.put(HEADER_SIGNATURE, Base64.getEncoder().encodeToString(signatureBytes));
    headers.put(HEADER_TIMESTAMP_MS, Long.toString(timestampMs));
    headers.put(HEADER_NONCE, nonce);
    return headers;
  }

  private static String canonicalAccountHeaderValue(final String accountId) {
    try {
      return AccountAddress.parseEncodedIgnoringCurveSupport(accountId, null)
          .canonicalHex();
    } catch (AccountAddress.AccountAddressException ignored) {
      return accountId;
    }
  }

  private static Map<String, Object> bodyWithBodyAuthFreshness(
      final Map<String, Object> bodyFields,
      final String accountId,
      final long timestampMs,
      final String nonce) {
    requireCanonicalAccountId(accountId);
    requireExactNonBlank(nonce, "nonce");
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
    if (signature.length > CANONICAL_REQUEST_MAX_SIGNATURE_BYTES_V1) {
      throw new IllegalStateException(
          "canonical request signature exceeds "
              + CANONICAL_REQUEST_MAX_SIGNATURE_BYTES_V1
              + " bytes");
    }
    boolean allZero = true;
    for (final byte value : signature) {
      if (value != 0) {
        allZero = false;
        break;
      }
    }
    if (allZero) {
      throw new IllegalStateException("canonical request signature must not be all zero");
    }
    return signature;
  }

  private static String requireHttpMethodToken(final String method) {
    if (method == null || method.isEmpty()) {
      throw new IllegalArgumentException("canonical request method is required");
    }
    if (method.length() > CANONICAL_REQUEST_MAX_METHOD_BYTES_V1) {
      throw new IllegalArgumentException(
          "canonical request method exceeds "
              + CANONICAL_REQUEST_MAX_METHOD_BYTES_V1
              + " ASCII bytes");
    }
    for (int index = 0; index < method.length(); index++) {
      if (!isHttpTokenCharacter(method.charAt(index))) {
        throw new IllegalArgumentException("canonical request method must be an ASCII HTTP token");
      }
    }
    return method;
  }

  private static boolean isHttpTokenCharacter(final char value) {
    return (value >= 'A' && value <= 'Z')
        || (value >= 'a' && value <= 'z')
        || (value >= '0' && value <= '9')
        || value == '!'
        || value == '#'
        || value == '$'
        || value == '%'
        || value == '&'
        || value == '\''
        || value == '*'
        || value == '+'
        || value == '-'
        || value == '.'
        || value == '^'
        || value == '_'
        || value == '`'
        || value == '|'
        || value == '~';
  }

  private static String requireCanonicalRawPath(final URI uri) {
    Objects.requireNonNull(uri, "uri");
    if (uri.isOpaque()) {
      throw new IllegalArgumentException("canonical request URI must be hierarchical");
    }
    if (uri.getRawFragment() != null) {
      throw new IllegalArgumentException("canonical request URI must not contain a fragment");
    }
    final String scheme = uri.getScheme();
    final String authority = uri.getRawAuthority();
    if (scheme == null) {
      if (authority != null) {
        throw new IllegalArgumentException("canonical request URI must not be scheme-relative");
      }
    } else if (authority == null
        || !(scheme.equalsIgnoreCase("http") || scheme.equalsIgnoreCase("https"))) {
      throw new IllegalArgumentException(
          "canonical request absolute URI must use HTTP(S) with an authority");
    }
    final String rawPath = uri.getRawPath() == null ? "" : uri.getRawPath();
    final String path = rawPath.isEmpty() && scheme != null ? "/" : rawPath;
    if (path.length() > CANONICAL_REQUEST_MAX_PATH_BYTES_V1) {
      throw new IllegalArgumentException(
          "canonical request path exceeds "
              + CANONICAL_REQUEST_MAX_PATH_BYTES_V1
              + " ASCII bytes");
    }
    if (path.isEmpty()
        || path.charAt(0) != '/'
        || (path.length() > 1 && path.charAt(1) == '/')) {
      throw new IllegalArgumentException(
          "canonical request path must be an exact root-relative path");
    }
    for (int index = 0; index < path.length(); index++) {
      final char character = path.charAt(index);
      if (character < 0x21 || character > 0x7e) {
        throw new IllegalArgumentException(
            "canonical request path must use its exact ASCII wire spelling");
      }
    }
    if (!hasSafeCanonicalPathSegments(path)) {
      throw new IllegalArgumentException(
          "canonical request path must use well-formed escapes without dot segments");
    }
    return path;
  }

  private static boolean hasSafeCanonicalPathSegments(final String path) {
    final StringBuilder structuralPath = new StringBuilder(path.length());
    int index = 0;
    while (index < path.length()) {
      if (path.charAt(index) != '%') {
        structuralPath.append(path.charAt(index++));
        continue;
      }
      if (index + 2 >= path.length()) {
        return false;
      }
      final int high = hexValue(path.charAt(index + 1));
      final int low = hexValue(path.charAt(index + 2));
      if (high < 0 || low < 0) {
        return false;
      }
      final int decoded = (high << 4) | low;
      structuralPath.append(decoded == '.' ? (char) decoded : '\0');
      index += 3;
    }
    for (final String segment : structuralPath.toString().split("/", -1)) {
      if (segment.equals(".") || segment.equals("..")) {
        return false;
      }
    }
    return true;
  }

  private static String requireCanonicalAccountId(final String accountId) {
    requireExactNonBlank(accountId, "accountId");
    try {
      AccountAddress.parseEncodedIgnoringCurveSupport(accountId, null);
      return accountId;
    } catch (AccountAddress.AccountAddressException ignored) {
      if (isCanonicalAsciiAccountAlias(accountId)) {
        return accountId;
      }
      throw new IllegalArgumentException(
          "accountId must be a canonical I105 account or canonical ASCII account alias");
    }
  }

  // This is wire-safe structural admission only. Torii owns UTS-46 and alias resolution.
  static boolean isCanonicalAsciiAccountAlias(final String value) {
    if (value.startsWith("0x")) {
      return false;
    }
    final int separator = value.indexOf('@');
    if (separator <= 0
        || separator != value.lastIndexOf('@')
        || separator == value.length() - 1) {
      return false;
    }
    final String[] scope = value.substring(separator + 1).split("\\.", -1);
    if (scope.length < 1 || scope.length > 2
        || !isCanonicalAsciiAliasSegment(value.substring(0, separator))) {
      return false;
    }
    for (final String segment : scope) {
      if (!isCanonicalAsciiAliasSegment(segment)) {
        return false;
      }
    }
    return true;
  }

  private static boolean isCanonicalAsciiAliasSegment(final String value) {
    if (value.isEmpty()
        || value.length() > 63
        || value.charAt(0) == '-'
        || value.charAt(value.length() - 1) == '-') {
      return false;
    }
    if (value.length() >= 4
        && value.charAt(2) == '-'
        && value.charAt(3) == '-'
        && !value.startsWith("xn--")) {
      return false;
    }
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (!((character >= 'a' && character <= 'z')
          || (character >= '0' && character <= '9')
          || character == '-'
          || character == '_')) {
        return false;
      }
    }
    return true;
  }

  private static void requireExactNonBlank(final String value, final String field) {
    if (value == null || value.isEmpty() || isAllWhitespace(value)) {
      throw new IllegalArgumentException(field + " is required");
    }
    if (Character.isWhitespace(value.charAt(0))
        || Character.isWhitespace(value.charAt(value.length() - 1))) {
      throw new IllegalArgumentException(field + " must not contain surrounding whitespace");
    }
    if ("accountId".equals(field)
        && value.getBytes(StandardCharsets.UTF_8).length
            > CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1) {
      throw new IllegalArgumentException(
          "accountId exceeds "
              + CANONICAL_REQUEST_MAX_ACCOUNT_LITERAL_BYTES_V1
              + " UTF-8 bytes");
    }
    if ("nonce".equals(field)) {
      if (value.getBytes(StandardCharsets.UTF_8).length > 256) {
        throw new IllegalArgumentException(
            "nonce must contain 1...256 non-whitespace ASCII bytes");
      }
      for (int index = 0; index < value.length(); index++) {
        final char character = value.charAt(index);
        if (character < 0x21 || character > 0x7e) {
          throw new IllegalArgumentException(
              "nonce must contain 1...256 non-whitespace ASCII bytes");
        }
      }
    }
  }

  private static byte[] concat(final byte[]... parts) {
    int length = 0;
    for (final byte[] part : parts) {
      length = Math.addExact(length, part.length);
    }
    final byte[] joined = new byte[length];
    int offset = 0;
    for (final byte[] part : parts) {
      System.arraycopy(part, 0, joined, offset, part.length);
      offset += part.length;
    }
    return joined;
  }

  private static boolean isAllWhitespace(final String value) {
    for (int index = 0; index < value.length(); index++) {
      if (!Character.isWhitespace(value.charAt(index))) {
        return false;
      }
    }
    return true;
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
    final byte[] raw = value.getBytes(StandardCharsets.UTF_8);
    final ByteArrayOutputStream decoded = new ByteArrayOutputStream(raw.length);
    int index = 0;
    while (index < raw.length) {
      final int current = raw[index] & 0xff;
      if (current == '+') {
        decoded.write(' ');
        index++;
      } else if (current == '%' && index + 2 < raw.length) {
        final int high = hexValue(raw[index + 1] & 0xff);
        final int low = hexValue(raw[index + 2] & 0xff);
        if (high >= 0 && low >= 0) {
          decoded.write((high << 4) | low);
          index += 3;
        } else {
          decoded.write(current);
          index++;
        }
      } else {
        decoded.write(current);
        index++;
      }
    }
    return decodeUtf8LossyLikeRust(decoded.toByteArray());
  }

  /**
   * Decodes UTF-8 with the same malformed-sequence boundaries as Rust's
   * {@code String::from_utf8_lossy}. The JVM decoder consumes an encoded surrogate such as
   * {@code ED A0 80} as one malformed unit, while Rust replaces each byte; canonical request
   * signatures must preserve the Rust/Torii grouping.
   */
  private static String decodeUtf8LossyLikeRust(final byte[] bytes) {
    final StringBuilder decoded = new StringBuilder(bytes.length);
    int index = 0;
    while (index < bytes.length) {
      final int first = bytes[index] & 0xff;
      if (first < 0x80) {
        decoded.append((char) first);
        index++;
      } else if (first >= 0xc2 && first <= 0xdf) {
        if (index + 1 >= bytes.length) {
          decoded.append('\uFFFD');
          index = bytes.length;
          continue;
        }
        final int second = bytes[index + 1] & 0xff;
        if (!isUtf8Continuation(second)) {
          decoded.append('\uFFFD');
          index++;
          continue;
        }
        decoded.append((char) (((first & 0x1f) << 6) | (second & 0x3f)));
        index += 2;
      } else if (first >= 0xe0 && first <= 0xef) {
        if (index + 1 >= bytes.length) {
          decoded.append('\uFFFD');
          index = bytes.length;
          continue;
        }
        final int second = bytes[index + 1] & 0xff;
        final boolean validSecond =
            first == 0xe0
                ? second >= 0xa0 && second <= 0xbf
                : first == 0xed
                    ? second >= 0x80 && second <= 0x9f
                    : isUtf8Continuation(second);
        if (!validSecond) {
          decoded.append('\uFFFD');
          index++;
          continue;
        }
        if (index + 2 >= bytes.length) {
          decoded.append('\uFFFD');
          index = bytes.length;
          continue;
        }
        final int third = bytes[index + 2] & 0xff;
        if (!isUtf8Continuation(third)) {
          decoded.append('\uFFFD');
          index += 2;
          continue;
        }
        final int codePoint =
            ((first & 0x0f) << 12) | ((second & 0x3f) << 6) | (third & 0x3f);
        decoded.append((char) codePoint);
        index += 3;
      } else if (first >= 0xf0 && first <= 0xf4) {
        if (index + 1 >= bytes.length) {
          decoded.append('\uFFFD');
          index = bytes.length;
          continue;
        }
        final int second = bytes[index + 1] & 0xff;
        final boolean validSecond =
            first == 0xf0
                ? second >= 0x90 && second <= 0xbf
                : first == 0xf4
                    ? second >= 0x80 && second <= 0x8f
                    : isUtf8Continuation(second);
        if (!validSecond) {
          decoded.append('\uFFFD');
          index++;
          continue;
        }
        if (index + 2 >= bytes.length) {
          decoded.append('\uFFFD');
          index = bytes.length;
          continue;
        }
        final int third = bytes[index + 2] & 0xff;
        if (!isUtf8Continuation(third)) {
          decoded.append('\uFFFD');
          index += 2;
          continue;
        }
        if (index + 3 >= bytes.length) {
          decoded.append('\uFFFD');
          index = bytes.length;
          continue;
        }
        final int fourth = bytes[index + 3] & 0xff;
        if (!isUtf8Continuation(fourth)) {
          decoded.append('\uFFFD');
          index += 3;
          continue;
        }
        final int codePoint =
            ((first & 0x07) << 18)
                | ((second & 0x3f) << 12)
                | ((third & 0x3f) << 6)
                | (fourth & 0x3f);
        decoded.append(Character.toChars(codePoint));
        index += 4;
      } else {
        decoded.append('\uFFFD');
        index++;
      }
    }
    return decoded.toString();
  }

  private static boolean isUtf8Continuation(final int value) {
    return value >= 0x80 && value <= 0xbf;
  }

  private static int hexValue(final int value) {
    if (value >= '0' && value <= '9') {
      return value - '0';
    }
    if (value >= 'A' && value <= 'F') {
      return value - 'A' + 10;
    }
    if (value >= 'a' && value <= 'f') {
      return value - 'a' + 10;
    }
    return -1;
  }

  private static int compareUtf8(final String left, final String right) {
    final byte[] leftBytes = left.getBytes(StandardCharsets.UTF_8);
    final byte[] rightBytes = right.getBytes(StandardCharsets.UTF_8);
    final int sharedLength = Math.min(leftBytes.length, rightBytes.length);
    for (int index = 0; index < sharedLength; index++) {
      final int difference = (leftBytes[index] & 0xff) - (rightBytes[index] & 0xff);
      if (difference != 0) {
        return difference;
      }
    }
    return leftBytes.length - rightBytes.length;
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

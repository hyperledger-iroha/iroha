package org.hyperledger.iroha.android.sorafs;

import java.net.InetAddress;
import java.net.URI;
import java.net.URISyntaxException;
import java.net.UnknownHostException;
import java.util.Base64;

final class SorafsInputValidator {

  private static final int MAX_PROVIDER_NAME_BYTES = 128;
  private static final int MAX_STREAM_TOKEN_ENCODED_BYTES = 90 * 1024;
  private static final int MAX_STREAM_TOKEN_DECODED_BYTES = 64 * 1024;

  private SorafsInputValidator() {}

  static String requireExactNonEmpty(final String value, final String field) {
    if (value == null || value.isEmpty()) {
      throw new IllegalArgumentException(field + " must not be empty");
    }
    if (isBoundaryWhitespace(value.charAt(0))
        || isBoundaryWhitespace(value.charAt(value.length() - 1))) {
      throw new IllegalArgumentException(
          field + " must not contain leading or trailing whitespace");
    }
    for (int i = 0; i < value.length(); i++) {
      if (Character.isISOControl(value.charAt(i))) {
        throw new IllegalArgumentException(field + " must not contain control characters");
      }
    }
    return value;
  }

  static String requireCanonicalHex(final String value, final String field) {
    requireExactNonEmpty(value, field);
    if ((value.length() & 1) == 1) {
      throw new IllegalArgumentException(field + " must contain an even number of hex characters");
    }
    for (int i = 0; i < value.length(); i++) {
      final char c = value.charAt(i);
      if (!isLowerHexDigit(c)) {
        throw new IllegalArgumentException(
            field + " must be canonical lowercase hex without a prefix");
      }
    }
    return value;
  }

  static String requireCanonicalHexBytes(
      final String value, final String field, final int expectedBytes) {
    if (expectedBytes <= 0) {
      throw new IllegalArgumentException("expectedBytes must be positive");
    }
    if (expectedBytes > Integer.MAX_VALUE / 2) {
      throw new IllegalArgumentException("expectedBytes is too large");
    }
    final String canonical = requireCanonicalHex(value, field);
    final int expectedLength = expectedBytes * 2;
    if (canonical.length() != expectedLength) {
      throw new IllegalArgumentException(
          field + " must be a " + expectedBytes + "-byte lowercase hex string");
    }
    boolean nonZero = false;
    for (int i = 0; i < canonical.length(); i++) {
      if (canonical.charAt(i) != '0') {
        nonZero = true;
        break;
      }
    }
    if (!nonZero) {
      throw new IllegalArgumentException(field + " must not be all zero");
    }
    return canonical;
  }

  static String requireCanonicalBase64(final String value, final String field) {
    requireExactNonEmpty(value, field);
    final byte[] decoded;
    try {
      decoded = Base64.getDecoder().decode(value);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(field + " must be canonical standard base64", ex);
    }
    if (decoded.length == 0) {
      throw new IllegalArgumentException(field + " must encode at least one byte");
    }
    if (!Base64.getEncoder().encodeToString(decoded).equals(value)) {
      throw new IllegalArgumentException(field + " must be canonical standard base64");
    }
    return value;
  }

  static String requireCanonicalStreamTokenBase64(final String value, final String field) {
    requireExactNonEmpty(value, field);
    if (value.length() > MAX_STREAM_TOKEN_ENCODED_BYTES) {
      throw new IllegalArgumentException(
          field + " must not exceed " + MAX_STREAM_TOKEN_ENCODED_BYTES + " encoded bytes");
    }
    final byte[] decoded;
    try {
      decoded = Base64.getDecoder().decode(value);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(field + " must be canonical standard base64", ex);
    }
    if (decoded.length == 0 || decoded.length > MAX_STREAM_TOKEN_DECODED_BYTES) {
      throw new IllegalArgumentException(
          field
              + " must encode between 1 and "
              + MAX_STREAM_TOKEN_DECODED_BYTES
              + " bytes");
    }
    if (!Base64.getEncoder().encodeToString(decoded).equals(value)) {
      throw new IllegalArgumentException(field + " must be canonical standard base64");
    }
    return value;
  }

  static String requireCanonicalProviderName(final String value, final String field) {
    requireExactNonEmpty(value, field);
    if (value.length() > MAX_PROVIDER_NAME_BYTES) {
      throw new IllegalArgumentException(
          field + " must be 1-" + MAX_PROVIDER_NAME_BYTES + " canonical ASCII bytes");
    }
    for (int i = 0; i < value.length(); i++) {
      final char c = value.charAt(i);
      if (!((c >= 'a' && c <= 'z')
          || (c >= 'A' && c <= 'Z')
          || (c >= '0' && c <= '9')
          || c == '.'
          || c == '_'
          || c == ':'
          || c == '-')) {
        throw new IllegalArgumentException(
            field + " must be 1-" + MAX_PROVIDER_NAME_BYTES + " canonical ASCII bytes");
      }
    }
    return value;
  }

  static String requireCanonicalGatewayBaseUrl(final String value, final String field) {
    requireExactNonEmpty(value, field);
    if (value.length() > 2048) {
      throw new IllegalArgumentException(field + " must not exceed 2048 characters");
    }
    final URI uri;
    try {
      uri = new URI(value);
    } catch (final URISyntaxException ex) {
      throw new IllegalArgumentException(field + " must be a canonical HTTPS origin URL", ex);
    }
    if (!uri.isAbsolute() || !"https".equals(uri.getScheme())) {
      throw new IllegalArgumentException(field + " must use HTTPS");
    }
    if (uri.getRawUserInfo() != null) {
      throw new IllegalArgumentException(field + " must not contain credentials");
    }
    if (uri.getRawQuery() != null || uri.getRawFragment() != null) {
      throw new IllegalArgumentException(field + " must not contain a query or fragment");
    }
    if (uri.getHost() == null) {
      throw new IllegalArgumentException(field + " must contain a canonical host");
    }
    if (!uri.getHost().equals(uri.getHost().toLowerCase(java.util.Locale.ROOT))) {
      throw new IllegalArgumentException(field + " host must use canonical lowercase");
    }
    if (uri.getPort() != -1) {
      throw new IllegalArgumentException(field + " must omit the default HTTPS port");
    }
    final String path = uri.getRawPath();
    if (path != null && !path.isEmpty() && !"/".equals(path)) {
      throw new IllegalArgumentException(field + " must use the origin root path");
    }
    if (!uri.toASCIIString().equals(value)) {
      throw new IllegalArgumentException(field + " must use exact canonical ASCII URL syntax");
    }
    if (!isPublicGatewayHost(uri.getHost())) {
      throw new IllegalArgumentException(field + " must target a canonical public host");
    }
    return value;
  }

  static URI requireCanonicalGatewayBaseUri(final URI value, final String field) {
    if (value == null) {
      throw new IllegalArgumentException(field + " must not be null");
    }
    requireCanonicalGatewayBaseUrl(value.toString(), field);
    return value;
  }

  static String requireCanonicalGatewayFetchPath(final String value, final String field) {
    requireExactNonEmpty(value, field);
    if (value.length() > 1024) {
      throw new IllegalArgumentException(field + " must not exceed 1024 characters");
    }
    final URI uri;
    try {
      uri = new URI(value);
    } catch (final URISyntaxException ex) {
      throw new IllegalArgumentException(field + " must be a canonical relative path", ex);
    }
    if (uri.isAbsolute() || uri.getRawAuthority() != null) {
      throw new IllegalArgumentException(
          field + " must be relative to the configured gateway origin");
    }
    if (uri.getRawQuery() != null || uri.getRawFragment() != null) {
      throw new IllegalArgumentException(field + " must not contain a query or fragment");
    }
    if (!value.equals(uri.getRawPath()) || !value.startsWith("/") || value.length() == 1) {
      throw new IllegalArgumentException(field + " must be an absolute-path reference");
    }
    if (value.indexOf('%') >= 0 || value.contains("//") || value.endsWith("/")) {
      throw new IllegalArgumentException(
          field + " must not contain encoding ambiguity or empty path segments");
    }
    final String[] segments = value.substring(1).split("/", -1);
    for (final String segment : segments) {
      if (segment.isEmpty() || ".".equals(segment) || "..".equals(segment)) {
        throw new IllegalArgumentException(field + " must not contain dot or empty path segments");
      }
    }
    for (int i = 0; i < value.length(); i++) {
      final char c = value.charAt(i);
      if (!(c == '/' || c == '-' || c == '_' || (c >= 'a' && c <= 'z')
          || (c >= '0' && c <= '9'))) {
        throw new IllegalArgumentException(
            field + " must use canonical lowercase ASCII path characters");
      }
    }
    return value;
  }

  static String requireCanonicalRolloutPhase(final String value, final String field) {
    requireExactNonEmpty(value, field);
    if (!"canary".equals(value) && !"ramp".equals(value) && !"default".equals(value)) {
      throw new IllegalArgumentException(field + " must be one of canary, ramp, or default");
    }
    return value;
  }

  static String requireCanonicalChunkerHandle(final String value, final String field) {
    requireExactNonEmpty(value, field);
    final int at = value.indexOf('@');
    if (at <= 0 || at != value.lastIndexOf('@') || at == value.length() - 1) {
      throw new IllegalArgumentException(chunkerHandleLabel(field));
    }
    final String identity = value.substring(0, at);
    final int separator = identity.indexOf('.');
    if (separator <= 0
        || separator != identity.lastIndexOf('.')
        || separator == identity.length() - 1) {
      throw new IllegalArgumentException(chunkerHandleLabel(field));
    }
    if (!isCanonicalHandleToken(identity.substring(0, separator))
        || !isCanonicalHandleToken(identity.substring(separator + 1))) {
      throw new IllegalArgumentException(chunkerHandleLabel(field));
    }
    final String[] versionParts = value.substring(at + 1).split("\\.", -1);
    if (versionParts.length != 3) {
      throw new IllegalArgumentException(chunkerHandleLabel(field));
    }
    for (final String part : versionParts) {
      if (!isCanonicalDecimalComponent(part)) {
        throw new IllegalArgumentException(chunkerHandleLabel(field));
      }
    }
    return value;
  }

  private static boolean isCanonicalHandleToken(final String value) {
    if (value.isEmpty()
        || value.charAt(0) < 'a'
        || value.charAt(0) > 'z'
        || value.charAt(value.length() - 1) == '-') {
      return false;
    }
    for (int i = 0; i < value.length(); i++) {
      final char c = value.charAt(i);
      if (!((c >= 'a' && c <= 'z') || (c >= '0' && c <= '9') || c == '-')) {
        return false;
      }
    }
    return true;
  }

  private static boolean isCanonicalDecimalComponent(final String value) {
    if (value.isEmpty() || (value.length() > 1 && value.charAt(0) == '0')) {
      return false;
    }
    for (int i = 0; i < value.length(); i++) {
      final char c = value.charAt(i);
      if (c < '0' || c > '9') {
        return false;
      }
    }
    return true;
  }

  private static boolean isBoundaryWhitespace(final char c) {
    return Character.isWhitespace(c) || Character.isSpaceChar(c);
  }

  private static boolean isLowerHexDigit(final char c) {
    return (c >= '0' && c <= '9') || (c >= 'a' && c <= 'f');
  }

  private static boolean isPublicGatewayHost(final String rawHost) {
    String host = rawHost;
    if (host.startsWith("[") && host.endsWith("]")) {
      host = host.substring(1, host.length() - 1);
    }
    if (host.isEmpty() || !host.equals(host.toLowerCase(java.util.Locale.ROOT))) {
      return false;
    }

    final int[] ipv4 = parseCanonicalIpv4(host);
    if (ipv4 != null) {
      return isPublicIpv4(ipv4);
    }
    boolean numericIpv4Like = true;
    for (int i = 0; i < host.length(); i++) {
      final char c = host.charAt(i);
      if (!((c >= '0' && c <= '9') || c == '.')) {
        numericIpv4Like = false;
        break;
      }
    }
    if (numericIpv4Like) {
      return false;
    }

    if (host.indexOf(':') >= 0) {
      final byte[] bytes;
      try {
        bytes = InetAddress.getByName(host).getAddress();
      } catch (final UnknownHostException | SecurityException ex) {
        return false;
      }
      if (bytes.length == 4) {
        final int[] mapped = new int[4];
        for (int i = 0; i < mapped.length; i++) {
          mapped[i] = bytes[i] & 0xff;
        }
        return isPublicIpv4(mapped);
      }
      return bytes.length == 16 && isPublicIpv6(bytes);
    }

    if ("localhost".equals(host)
        || host.endsWith(".localhost")
        || host.endsWith(".local")
        || host.endsWith(".internal")
        || host.endsWith(".lan")
        || host.length() > 253
        || host.endsWith(".")) {
      return false;
    }
    final String[] labels = host.split("\\.", -1);
    for (final String label : labels) {
      if (label.isEmpty()
          || label.length() > 63
          || !isAsciiLowerAlphanumeric(label.charAt(0))
          || !isAsciiLowerAlphanumeric(label.charAt(label.length() - 1))) {
        return false;
      }
      for (int i = 0; i < label.length(); i++) {
        final char c = label.charAt(i);
        if (!isAsciiLowerAlphanumeric(c) && c != '-') {
          return false;
        }
      }
    }
    return true;
  }

  private static int[] parseCanonicalIpv4(final String host) {
    final String[] parts = host.split("\\.", -1);
    if (parts.length != 4) {
      return null;
    }
    final int[] octets = new int[4];
    for (int i = 0; i < parts.length; i++) {
      final String part = parts[i];
      if (part.isEmpty() || (part.length() > 1 && part.charAt(0) == '0')) {
        return null;
      }
      int value = 0;
      for (int j = 0; j < part.length(); j++) {
        final char c = part.charAt(j);
        if (c < '0' || c > '9') {
          return null;
        }
        value = value * 10 + c - '0';
        if (value > 255) {
          return null;
        }
      }
      octets[i] = value;
    }
    return octets;
  }

  private static boolean isPublicIpv4(final int[] octets) {
    if (octets.length != 4) {
      return false;
    }
    final int first = octets[0];
    final int second = octets[1];
    final int third = octets[2];
    final int fourth = octets[3];
    return first != 0
        && first != 10
        && first != 127
        && first < 224
        && !(first == 100 && second >= 64 && second <= 127)
        && !(first == 169 && second == 254)
        && !(first == 172 && second >= 16 && second <= 31)
        && !(first == 192 && second == 0 && third == 0)
        && !(first == 192 && second == 0 && third == 2)
        && !(first == 192 && second == 88 && third == 99)
        && !(first == 192 && second == 168)
        && !(first == 198 && (second == 18 || second == 19))
        && !(first == 198 && second == 51 && third == 100)
        && !(first == 203 && second == 0 && third == 113)
        && !(first == 255 && second == 255 && third == 255 && fourth == 255);
  }

  private static boolean isPublicIpv6(final byte[] bytes) {
    final int first = ((bytes[0] & 0xff) << 8) | (bytes[1] & 0xff);
    final int second = ((bytes[2] & 0xff) << 8) | (bytes[3] & 0xff);
    final boolean globalUnicast = (first & 0xe000) == 0x2000;
    final boolean documentation =
        (first == 0x2001 && second == 0x0db8)
            || (first == 0x3fff && (second & 0xf000) == 0);
    final boolean specialPurpose = first == 0x2001 && second <= 0x01ff;
    return globalUnicast && !documentation && !specialPurpose && first != 0x2002;
  }

  private static boolean isAsciiLowerAlphanumeric(final char c) {
    return (c >= 'a' && c <= 'z') || (c >= '0' && c <= '9');
  }

  private static String chunkerHandleLabel(final String field) {
    return field + " must be a canonical chunker handle (namespace.name@major.minor.patch)";
  }
}

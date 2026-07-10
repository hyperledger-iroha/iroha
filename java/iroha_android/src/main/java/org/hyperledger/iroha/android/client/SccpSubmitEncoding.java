package org.hyperledger.iroha.android.client;

import java.util.Arrays;
import java.util.Base64;
import org.hyperledger.iroha.norito.NoritoHeader;

/** Shared strict encoding checks for SCCP bridge submit DTOs. */
final class SccpSubmitEncoding {
  static final int MAX_ARTIFACT_BYTES = 16 * 1024 * 1024;

  private SccpSubmitEncoding() {}

  static byte[] validateCanonicalNoritoBase64(
      final String value, final String field, final int maximum) {
    if (value == null || value.isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(field + " must be canonical padded base64");
    }
    final byte[] decoded;
    try {
      decoded = Base64.getDecoder().decode(value);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(field + " must be valid base64", ex);
    }
    if (decoded.length == 0 || decoded.length > maximum) {
      throw new IllegalArgumentException(field + " exceeds its canonical size bound");
    }
    if (!Base64.getEncoder().encodeToString(decoded).equals(value)) {
      throw new IllegalArgumentException(field + " must be canonical padded base64");
    }
    final NoritoHeader.DecodeResult result;
    try {
      result = NoritoHeader.decode(decoded, null);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(field + " must contain a canonical Norito envelope", ex);
    }
    final NoritoHeader header = result.header();
    if (header.compression() != NoritoHeader.COMPRESSION_NONE) {
      throw new IllegalArgumentException(field + " must use uncompressed canonical Norito");
    }
    final int headerPadding =
        decoded.length - NoritoHeader.HEADER_LENGTH - header.payloadLength();
    if (headerPadding != 0 && headerPadding != 8) {
      throw new IllegalArgumentException(
          field + " must use canonical Norito header alignment padding");
    }
    if (allZero(header.schemaHash())) {
      throw new IllegalArgumentException(field + " must advertise a nonzero Norito schema");
    }
    if (!Arrays.equals(
        header.encode(), Arrays.copyOfRange(decoded, 0, NoritoHeader.HEADER_LENGTH))) {
      throw new IllegalArgumentException(field + " contains a non-canonical Norito header");
    }
    header.validateChecksum(result.payload());
    return decoded;
  }

  static String requireCanonicalNonBlank(final String value, final String field) {
    if (value == null || value.isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(field + " is required and must be canonical");
    }
    return value;
  }

  static String normalizeOptional(final String value) {
    if (value == null) return null;
    if (!value.equals(value.trim())) {
      throw new IllegalArgumentException(
          "optional string fields must not contain surrounding whitespace");
    }
    return value.isEmpty() ? null : value;
  }

  static String normalizeOptionalHex(final String value, final int bytes, final String field) {
    final String normalized = normalizeOptional(value);
    if (normalized == null) return null;
    if (!normalized.startsWith("0x") || normalized.length() != 2 + bytes * 2) {
      throw new IllegalArgumentException(
          field + " must be canonical lowercase 0x-prefixed " + bytes + "-byte hex");
    }
    boolean nonzero = false;
    for (int i = 2; i < normalized.length(); i++) {
      final char item = normalized.charAt(i);
      if (!((item >= '0' && item <= '9') || (item >= 'a' && item <= 'f'))) {
        throw new IllegalArgumentException(
            field + " must be canonical lowercase 0x-prefixed " + bytes + "-byte hex");
      }
      nonzero |= item != '0';
    }
    if (!nonzero) throw new IllegalArgumentException(field + " must be nonzero");
    return normalized;
  }

  static String normalizeOptionalPublicKeyHex(final String value) {
    final String normalized = normalizeOptional(value);
    if (normalized == null) return null;
    if (normalized.length() != 64) {
      throw new IllegalArgumentException(
          "publicKeyHex must be exactly 32 nonzero lowercase hexadecimal bytes");
    }
    boolean nonzero = false;
    for (int i = 0; i < normalized.length(); i++) {
      final char item = normalized.charAt(i);
      if (!((item >= '0' && item <= '9') || (item >= 'a' && item <= 'f'))) {
        throw new IllegalArgumentException(
            "publicKeyHex must be exactly 32 nonzero lowercase hexadecimal bytes");
      }
      nonzero |= item != '0';
    }
    if (!nonzero) {
      throw new IllegalArgumentException(
          "publicKeyHex must be exactly 32 nonzero lowercase hexadecimal bytes");
    }
    return normalized;
  }

  static Long normalizeOptionalCreationTimeMs(final Long value) {
    if (value != null && value <= 0) {
      throw new IllegalArgumentException("creationTimeMs must be positive");
    }
    return value;
  }

  static String normalizeOptionalExactBase64(final String value, final String field) {
    if (value == null) return null;
    if (value.isEmpty() || !value.equals(value.trim())) {
      throw new IllegalArgumentException(field + " must be exact standard-base64");
    }
    final byte[] decoded;
    try {
      decoded = Base64.getDecoder().decode(value);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(field + " must be valid base64", ex);
    }
    if (decoded.length == 0 || !Base64.getEncoder().encodeToString(decoded).equals(value)) {
      throw new IllegalArgumentException(field + " must be exact standard-base64");
    }
    if ("signatureB64".equals(field) && decoded.length != 64) {
      throw new IllegalArgumentException(field + " must contain a 64-byte Ed25519 signature");
    }
    return value;
  }

  private static boolean allZero(final byte[] value) {
    for (final byte item : value) {
      if (item != 0) return false;
    }
    return true;
  }
}

package org.hyperledger.iroha.android.address;

import java.util.Arrays;
import java.util.Locale;
import org.hyperledger.iroha.android.crypto.Ed25519PublicKeyAdmission;
import org.hyperledger.iroha.norito.Varint;

/** Utilities for encoding and decoding canonical public key literals. */
public final class PublicKeyCodec {

  private PublicKeyCodec() {}

  public static final class PublicKeyPayload {
    private final int curveId;
    private final byte[] keyBytes;

    private PublicKeyPayload(final int curveId, final byte[] keyBytes) {
      this.curveId = curveId;
      this.keyBytes = Arrays.copyOf(keyBytes, keyBytes.length);
    }

    public int curveId() {
      return curveId;
    }

    public byte[] keyBytes() {
      return Arrays.copyOf(keyBytes, keyBytes.length);
    }
  }

  /**
   * Decodes a multihash public key literal into its curve id and payload bytes.
   * The literal may be bare or use one exact canonical algorithm prefix; surrounding whitespace,
   * unknown prefixes, and prefixes that do not match the encoded curve are rejected. Returns
   * {@code null} when the literal is not a valid multihash key.
   */
  public static PublicKeyPayload decodePublicKeyLiteral(final String literal) {
    if (literal == null || literal.isEmpty()) {
      return null;
    }
    if (isPublicKeyLiteralWhitespace(literal.charAt(0))
        || isPublicKeyLiteralWhitespace(literal.charAt(literal.length() - 1))) {
      return null;
    }
    String encoded = literal;
    String algorithmPrefix = null;
    final int colonIndex = encoded.indexOf(':');
    if (colonIndex >= 0) {
      if (colonIndex == 0
          || colonIndex == encoded.length() - 1
          || colonIndex != encoded.lastIndexOf(':')) {
        return null;
      }
      algorithmPrefix = encoded.substring(0, colonIndex);
      encoded = encoded.substring(colonIndex + 1);
    }
    if (encoded.startsWith("0x") || encoded.startsWith("0X")) {
      return null;
    }
    if ((encoded.length() & 1) == 1) {
      return null;
    }
    if (!encoded.matches("(?i)[0-9a-f]+")) {
      return null;
    }
    final byte[] bytes = hexToBytes(encoded);
    final Varint.DecodeResult code = Varint.decode(bytes, 0);
    final Varint.DecodeResult len = Varint.decode(bytes, code.nextOffset());
    if (len.value() > Integer.MAX_VALUE) {
      return null;
    }
    final int payloadOffset = len.nextOffset();
    final int payloadLength = (int) len.value();
    if (payloadOffset + payloadLength != bytes.length) {
      return null;
    }
    final int curveId = curveIdForMultihashCode(code.value());
    if (curveId < 0) {
      return null;
    }
    if (algorithmPrefix != null && !algorithmPrefix.equals(algorithmForCurveId(curveId))) {
      return null;
    }
    final byte[] keyBytes = Arrays.copyOfRange(bytes, payloadOffset, payloadOffset + payloadLength);
    if (curveId == 0x01 && !Ed25519PublicKeyAdmission.isValid(keyBytes)) {
      return null;
    }
    return new PublicKeyPayload(curveId, keyBytes);
  }

  private static boolean isPublicKeyLiteralWhitespace(final char character) {
    return Character.isWhitespace(character) || Character.isSpaceChar(character);
  }

  /** Encodes the multihash public key literal from the given curve id and key bytes. */
  public static String encodePublicKeyMultihash(final int curveId, final byte[] keyBytes) {
    final long code = multihashCodeForCurveId(curveId);
    requireValidPublicKeyForEncoding(curveId, keyBytes);
    final byte[] codeVarint = Varint.encode(code);
    final byte[] lenVarint = Varint.encode(keyBytes.length);
    final StringBuilder builder =
        new StringBuilder((codeVarint.length + lenVarint.length + keyBytes.length) * 2);
    appendHexLower(builder, codeVarint);
    appendHexLower(builder, lenVarint);
    appendHexUpper(builder, keyBytes);
    return builder.toString();
  }

  /** Encodes a public key as Iroha's compact Norito payload: algorithm tag plus key bytes. */
  public static byte[] compactPublicKeyPayload(final int curveId, final byte[] keyBytes) {
    final int tag = compactAlgorithmTagForCurveId(curveId);
    requireValidPublicKeyForEncoding(curveId, keyBytes);
    final byte[] payload = new byte[1 + keyBytes.length];
    payload[0] = (byte) tag;
    System.arraycopy(keyBytes, 0, payload, 1, keyBytes.length);
    return payload;
  }

  /** Decodes Iroha's compact Norito public-key payload. */
  public static PublicKeyPayload decodeCompactPublicKeyPayload(final byte[] payload) {
    if (payload == null || payload.length == 0) {
      return null;
    }
    final int curveId = curveIdForCompactAlgorithmTag(payload[0] & 0xFF);
    if (curveId < 0) {
      return null;
    }
    final byte[] keyBytes = Arrays.copyOfRange(payload, 1, payload.length);
    if (curveId == 0x01 && !Ed25519PublicKeyAdmission.isValid(keyBytes)) {
      return null;
    }
    return new PublicKeyPayload(curveId, keyBytes);
  }

  /** Returns the canonical algorithm label for the curve id, or {@code null} when unknown. */
  public static String algorithmForCurveId(final int curveId) {
    switch (curveId) {
      case 0x01:
        return "ed25519";
      case 0x02:
        return "ml-dsa";
      case 0x03:
        return "bls_normal";
      case 0x04:
        return "secp256k1";
      case 0x05:
        return "bls_small";
      case 0x0A:
        return "gost256a";
      case 0x0B:
        return "gost256b";
      case 0x0C:
        return "gost256c";
      case 0x0D:
        return "gost512a";
      case 0x0E:
        return "gost512b";
      case 0x0F:
        return "sm2";
      default:
        return null;
    }
  }

  private static void requireValidPublicKeyForEncoding(
      final int curveId, final byte[] keyBytes) {
    if (curveId == 0x01 && !Ed25519PublicKeyAdmission.isValid(keyBytes)) {
      throw new IllegalArgumentException(
          "invalid Ed25519 public key: expected a canonical point in the prime-order subgroup");
    }
  }

  private static int compactAlgorithmTagForCurveId(final int curveId) {
    switch (curveId) {
      case 0x01:
        return 0;
      case 0x04:
        return 1;
      case 0x03:
        return 2;
      case 0x05:
        return 3;
      case 0x02:
        return 4;
      case 0x0A:
        return 5;
      case 0x0B:
        return 6;
      case 0x0C:
        return 7;
      case 0x0D:
        return 8;
      case 0x0E:
        return 9;
      case 0x0F:
        return 10;
      default:
        throw new IllegalArgumentException("Unsupported curve id: " + curveId);
    }
  }

  private static int curveIdForCompactAlgorithmTag(final int tag) {
    switch (tag) {
      case 0:
        return 0x01;
      case 1:
        return 0x04;
      case 2:
        return 0x03;
      case 3:
        return 0x05;
      case 4:
        return 0x02;
      case 5:
        return 0x0A;
      case 6:
        return 0x0B;
      case 7:
        return 0x0C;
      case 8:
        return 0x0D;
      case 9:
        return 0x0E;
      case 10:
        return 0x0F;
      default:
        return -1;
    }
  }

  private static int curveIdForMultihashCode(final long code) {
    if (code == 0xedL) {
      return 0x01;
    }
    if (code == 0xeaL) {
      return 0x03;
    }
    if (code == 0xe7L) {
      return 0x04;
    }
    if (code == 0xebL) {
      return 0x05;
    }
    if (code == 0xeeL) {
      return 0x02;
    }
    if (code == 0x1200L) {
      return 0x0A;
    }
    if (code == 0x1201L) {
      return 0x0B;
    }
    if (code == 0x1202L) {
      return 0x0C;
    }
    if (code == 0x1203L) {
      return 0x0D;
    }
    if (code == 0x1204L) {
      return 0x0E;
    }
    if (code == 0x1306L) {
      return 0x0F;
    }
    return -1;
  }

  private static long multihashCodeForCurveId(final int curveId) {
    switch (curveId) {
      case 0x01:
        return 0xedL;
      case 0x02:
        return 0xeeL;
      case 0x03:
        return 0xeaL;
      case 0x04:
        return 0xe7L;
      case 0x05:
        return 0xebL;
      case 0x0A:
        return 0x1200L;
      case 0x0B:
        return 0x1201L;
      case 0x0C:
        return 0x1202L;
      case 0x0D:
        return 0x1203L;
      case 0x0E:
        return 0x1204L;
      case 0x0F:
        return 0x1306L;
      default:
        throw new IllegalArgumentException("Unsupported curve id: " + curveId);
    }
  }

  private static byte[] hexToBytes(final String hex) {
    final int len = hex.length();
    final byte[] out = new byte[len / 2];
    for (int i = 0; i < len; i += 2) {
      final int high = Character.digit(hex.charAt(i), 16);
      final int low = Character.digit(hex.charAt(i + 1), 16);
      if (high < 0 || low < 0) {
        throw new IllegalArgumentException("Invalid hex literal");
      }
      out[i / 2] = (byte) ((high << 4) + low);
    }
    return out;
  }

  private static void appendHexLower(final StringBuilder builder, final byte[] bytes) {
    for (final byte b : bytes) {
      builder.append(String.format(Locale.ROOT, "%02x", b & 0xFF));
    }
  }

  private static void appendHexUpper(final StringBuilder builder, final byte[] bytes) {
    for (final byte b : bytes) {
      builder.append(String.format(Locale.ROOT, "%02X", b & 0xFF));
    }
  }
}

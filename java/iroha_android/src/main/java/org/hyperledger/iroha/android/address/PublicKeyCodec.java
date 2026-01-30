package org.hyperledger.iroha.android.address;

import java.util.Arrays;
import java.util.Locale;
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
   * Returns {@code null} when the literal is not a valid multihash key.
   */
  public static PublicKeyPayload decodePublicKeyLiteral(final String literal) {
    if (literal == null || literal.isBlank()) {
      return null;
    }
    String trimmed = literal.trim();
    final int colonIndex = trimmed.indexOf(':');
    if (colonIndex > 0) {
      trimmed = trimmed.substring(colonIndex + 1);
    }
    if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
      return null;
    }
    if ((trimmed.length() & 1) == 1) {
      return null;
    }
    if (!trimmed.matches("(?i)[0-9a-f]+")) {
      return null;
    }
    final byte[] bytes = hexToBytes(trimmed);
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
    final byte[] keyBytes = Arrays.copyOfRange(bytes, payloadOffset, payloadOffset + payloadLength);
    return new PublicKeyPayload(curveId, keyBytes);
  }

  /** Encodes the multihash public key literal from the given curve id and key bytes. */
  public static String encodePublicKeyMultihash(final int curveId, final byte[] keyBytes) {
    final long code = multihashCodeForCurveId(curveId);
    final byte[] codeVarint = Varint.encode(code);
    final byte[] lenVarint = Varint.encode(keyBytes.length);
    final StringBuilder builder =
        new StringBuilder((codeVarint.length + lenVarint.length + keyBytes.length) * 2);
    appendHexLower(builder, codeVarint);
    appendHexLower(builder, lenVarint);
    appendHexUpper(builder, keyBytes);
    return builder.toString();
  }

  /** Returns the canonical algorithm label for the curve id, or {@code null} when unknown. */
  public static String algorithmForCurveId(final int curveId) {
    switch (curveId) {
      case 0x01:
        return "ed25519";
      case 0x02:
        return "ml-dsa";
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

  private static int curveIdForMultihashCode(final long code) {
    if (code == 0xedL) {
      return 0x01;
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

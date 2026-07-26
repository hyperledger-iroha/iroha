package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.Objects;

/** Canonical first-release Kagemusha peer identifiers and PKK2R/P/A text envelopes. */
public final class KagemushaPeerTransport {
  public static final String RECEIVE_REQUEST_TEXT_PREFIX = "PKK2R.";
  public static final String PAYMENT_TEXT_PREFIX = "PKK2P.";
  public static final String ACKNOWLEDGEMENT_TEXT_PREFIX = "PKK2A.";
  public static final String QR_STREAM_TEXT_PREFIX = "PKKQ1.";
  public static final String NFC_APPLICATION_IDENTIFIER_HEX =
      IrohaPeerNfcV1.APPLICATION_IDENTIFIER_HEX;
  public static final String NEARBY_SERVICE_NAME = "pk-kagemusha";
  public static final String NEARBY_BONJOUR_SERVICE = "_pk-kagemusha._tcp";
  public static final String RECEIVE_REQUEST_CONTENT_TYPE =
      "text/vnd.pk.kagemusha-v2.receive-request";
  public static final String PAYMENT_CONTENT_TYPE = "text/vnd.pk.kagemusha-v2.payment";
  public static final String ACKNOWLEDGEMENT_CONTENT_TYPE = "text/vnd.pk.kagemusha-v2.ack";
  public static final int MAXIMUM_ARCHIVE_BYTES_V2 =
      KagemushaRecursiveSpendProver.MAX_PEER_ARCHIVE_BYTES_V2;
  public static final int MAXIMUM_ARCHIVE_BYTES_V4 =
      KagemushaRecursiveSpendProver.MAX_PEER_ARCHIVE_BYTES_V4;
  public static final int MAXIMUM_ARCHIVE_BYTES = MAXIMUM_ARCHIVE_BYTES_V4;
  public static final int MAXIMUM_TEXT_ENVELOPE_BYTES =
      KagemushaRecursiveSpendProver.MAX_PEER_TEXT_ENVELOPE_BYTES;

  private static final Base64.Encoder ENCODER = Base64.getUrlEncoder().withoutPadding();
  private static final Base64.Decoder DECODER = Base64.getUrlDecoder();

  private KagemushaPeerTransport() {}

  public static String encode(final Payload payload) {
    Objects.requireNonNull(payload, "payload");
    final byte[] archive = payload.archive();
    try {
      requireArchiveBound(archive);
      final String value = payload.kind().textPrefix + ENCODER.encodeToString(archive);
      if (value.getBytes(StandardCharsets.UTF_8).length > MAXIMUM_TEXT_ENVELOPE_BYTES) {
        throw new IllegalArgumentException("Kagemusha peer text exceeds its bound");
      }
      return value;
    } finally {
      java.util.Arrays.fill(archive, (byte) 0);
    }
  }

  public static Payload decode(final String value) {
    return decode(value, null);
  }

  public static Payload decode(final String value, final Kind expectedKind) {
    Objects.requireNonNull(value, "value");
    if (value.getBytes(StandardCharsets.UTF_8).length > MAXIMUM_TEXT_ENVELOPE_BYTES) {
      throw new IllegalArgumentException("Kagemusha peer text exceeds its bound");
    }
    final Kind kind = kindOf(value);
    if (kind == null) throw new IllegalArgumentException("Kagemusha peer prefix is invalid");
    if (expectedKind != null && expectedKind != kind) {
      throw new IllegalArgumentException("Unexpected Kagemusha peer payload kind");
    }
    final String body = value.substring(kind.textPrefix.length());
    final byte[] archive = decodeBase64Url(body);
    try {
      if (!kind.textPrefix.concat(ENCODER.encodeToString(archive)).equals(value)) {
        throw new IllegalArgumentException("Kagemusha peer text is not canonical");
      }
      return Payload.decode(archive, kind);
    } finally {
      java.util.Arrays.fill(archive, (byte) 0);
    }
  }

  public static Payload decodeUserPresented(final String value, final Kind expectedKind) {
    Objects.requireNonNull(value, "value");
    if (value.getBytes(StandardCharsets.UTF_8).length > MAXIMUM_TEXT_ENVELOPE_BYTES) {
      throw new IllegalArgumentException("Kagemusha peer text exceeds its bound");
    }
    return decode(trimAsciiBoundary(value), expectedKind);
  }

  public static Kind kindOf(final String value) {
    for (final Kind kind : Kind.values()) {
      if (value.startsWith(kind.textPrefix)) return kind;
    }
    return null;
  }

  public static String base64UrlEncode(final byte[] value) {
    return ENCODER.encodeToString(Objects.requireNonNull(value, "value"));
  }

  public static byte[] base64UrlDecode(final String value) {
    return decodeBase64Url(value);
  }

  private static byte[] decodeBase64Url(final String value) {
    if (value == null || value.isEmpty() || value.length() % 4 == 1) {
      throw new IllegalArgumentException("Kagemusha peer text is not canonical Base64URL");
    }
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (!((character >= '0' && character <= '9')
          || (character >= 'A' && character <= 'Z')
          || (character >= 'a' && character <= 'z')
          || character == '-'
          || character == '_')) {
        throw new IllegalArgumentException("Kagemusha peer text is not canonical Base64URL");
      }
    }
    final byte[] decoded;
    try {
      decoded = DECODER.decode(value);
    } catch (IllegalArgumentException failure) {
      throw new IllegalArgumentException("Kagemusha peer text is not canonical Base64URL", failure);
    }
    if (decoded.length == 0 || !ENCODER.encodeToString(decoded).equals(value)) {
      java.util.Arrays.fill(decoded, (byte) 0);
      throw new IllegalArgumentException("Kagemusha peer text is not canonical Base64URL");
    }
    return decoded;
  }

  private static String trimAsciiBoundary(final String value) {
    int start = 0;
    int end = value.length();
    while (start < end && isBoundary(value.charAt(start))) start++;
    while (end > start && isBoundary(value.charAt(end - 1))) end--;
    return value.substring(start, end);
  }

  private static boolean isBoundary(final char value) {
    return value == ' ' || value == '\t' || value == '\r' || value == '\n';
  }

  private static void requireArchiveBound(final byte[] archive) {
    if (archive == null || archive.length == 0 || archive.length > MAXIMUM_ARCHIVE_BYTES) {
      throw new IllegalArgumentException("Kagemusha peer archive exceeds its bound");
    }
  }

  public enum Kind {
    RECEIVE_REQUEST(1, RECEIVE_REQUEST_TEXT_PREFIX, RECEIVE_REQUEST_CONTENT_TYPE),
    PAYMENT(2, PAYMENT_TEXT_PREFIX, PAYMENT_CONTENT_TYPE),
    ACKNOWLEDGEMENT(3, ACKNOWLEDGEMENT_TEXT_PREFIX, ACKNOWLEDGEMENT_CONTENT_TYPE);

    private final int code;
    private final String textPrefix;
    private final String contentType;

    Kind(final int code, final String textPrefix, final String contentType) {
      this.code = code;
      this.textPrefix = textPrefix;
      this.contentType = contentType;
    }

    public int code() { return code; }
    public String textPrefix() { return textPrefix; }
    public String contentType() { return contentType; }

    public static Kind fromCode(final int code) {
      for (final Kind value : values()) if (value.code == code) return value;
      return null;
    }

    public static Kind fromContentType(final String contentType) {
      for (final Kind value : values()) if (value.contentType.equals(contentType)) return value;
      return null;
    }
  }

  public static final class Payload {
    private final Kind kind;
    private final KagemushaRecursiveSpendProver.CanonicalArchive typedArchive;

    private Payload(
        final Kind kind, final KagemushaRecursiveSpendProver.CanonicalArchive typedArchive) {
      this.kind = Objects.requireNonNull(kind, "kind");
      this.typedArchive = Objects.requireNonNull(typedArchive, "typedArchive");
    }

    public Kind kind() { return kind; }
    public byte[] archive() { return typedArchive.noritoEncoded(); }
    public KagemushaRecursiveSpendProver.CanonicalArchive typedArchive() { return typedArchive; }

    public static Payload decode(final byte[] archive, final Kind kind) {
      requireArchiveBound(archive);
      try {
        return switch (Objects.requireNonNull(kind, "kind")) {
          case RECEIVE_REQUEST -> new Payload(
              kind, KagemushaRecursiveSpendProver.decodeRecipientReceiveOfferV2(archive));
          case PAYMENT -> new Payload(
              kind, KagemushaRecursiveSpendProver.decodePeerPayment(archive));
          case ACKNOWLEDGEMENT -> new Payload(
              kind, KagemushaRecursiveSpendProver.decodeReceiverAcknowledgement(archive));
        };
      } catch (RuntimeException failure) {
        throw new IllegalArgumentException("Invalid Kagemusha peer archive", failure);
      }
    }
  }
}

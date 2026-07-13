package org.hyperledger.iroha.android.offline;

import java.nio.ByteBuffer;
import java.nio.CharBuffer;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.CodingErrorAction;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Objects;
import java.util.regex.Matcher;
import java.util.regex.Pattern;

/** Canonical Kagemusha Nearby envelope and fail-closed authenticated-transport policy. */
public final class KagemushaNearby {
  public static final int MAXIMUM_ENVELOPE_BYTES = 20 * 1024;
  public static final String SERVICE_NAME = KagemushaPeerTransport.NEARBY_SERVICE_NAME;
  public static final String BONJOUR_SERVICE = KagemushaPeerTransport.NEARBY_BONJOUR_SERVICE;
  public static final String DISCOVERY_PROTOCOL = "kagemusha-v2";
  public static final boolean REQUIRES_CERTIFICATE_AUTHENTICATED_ECDH_TRANSCRIPT = true;
  public static final boolean HAS_AUDITED_AUTHENTICATED_TRANSCRIPT_BACKEND = false;
  public static final boolean IS_AVAILABLE = HAS_AUDITED_AUTHENTICATED_TRANSCRIPT_BACKEND;

  private static final String REJECTION_CONTENT_TYPE = "text/plain";
  private static final String REJECTION_TEXT = "rejected";
  private static final Pattern CANONICAL_ENVELOPE =
      Pattern.compile(
          "\\A\\{\"contentType\":\"([^\"\\\\]+)\",\"kind\":\"([^\"\\\\]+)\""
              + "(?:,\"pairingChallenge\":\"([^\"\\\\]+)\")?"
              + ",\"payload\":\"([A-Za-z0-9_-]+)\"\\}\\z");

  private KagemushaNearby() {}

  public static byte[] encode(final KagemushaPeerTransport.Payload payload) {
    return encode(payload, null);
  }

  public static byte[] encode(
      final KagemushaPeerTransport.Payload payload, final PairingSymbol pairingChallenge) {
    Objects.requireNonNull(payload, "payload");
    switch (payload.kind()) {
      case RECEIVE_REQUEST -> require(pairingChallenge != null,
          "Kagemusha receive request requires a pairing challenge");
      case PAYMENT, ACKNOWLEDGEMENT -> require(pairingChallenge == null,
          "Kagemusha payment and acknowledgement cannot carry a pairing challenge");
    }
    final byte[] text = KagemushaPeerTransport.encode(payload).getBytes(StandardCharsets.UTF_8);
    try {
      return encodeEnvelope(
          payload.kind().contentType(),
          MessageKind.fromPeerKind(payload.kind()),
          pairingChallenge,
          KagemushaPeerTransport.base64UrlEncode(text));
    } finally {
      Arrays.fill(text, (byte) 0);
    }
  }

  public static byte[] encodeRejection() {
    final byte[] text = REJECTION_TEXT.getBytes(StandardCharsets.UTF_8);
    try {
      return encodeEnvelope(
          REJECTION_CONTENT_TYPE,
          MessageKind.REJECTED,
          null,
          KagemushaPeerTransport.base64UrlEncode(text));
    } finally {
      Arrays.fill(text, (byte) 0);
    }
  }

  public static Decoded decode(final byte[] data) {
    Objects.requireNonNull(data, "data");
    require(data.length > 0 && data.length <= MAXIMUM_ENVELOPE_BYTES,
        "Invalid Kagemusha Nearby envelope");
    final String text = strictUtf8(data);
    final Matcher match = CANONICAL_ENVELOPE.matcher(text);
    require(match.matches(), "Invalid Kagemusha Nearby envelope");

    final String contentType = match.group(1);
    final MessageKind messageKind = MessageKind.fromWireValue(match.group(2));
    require(messageKind != null, "Invalid Kagemusha Nearby message kind");
    final PairingSymbol challenge = PairingSymbol.fromWireValue(match.group(3));
    require(match.group(3) == null || challenge != null,
        "Invalid Kagemusha Nearby pairing challenge");
    final String payloadText = match.group(4);

    final byte[] canonical = encodeEnvelope(contentType, messageKind, challenge, payloadText);
    try {
      require(Arrays.equals(canonical, data), "Kagemusha Nearby envelope is not canonical");
    } finally {
      Arrays.fill(canonical, (byte) 0);
    }

    final byte[] payloadBytes = KagemushaPeerTransport.base64UrlDecode(payloadText);
    try {
      require(payloadBytes.length <= KagemushaPeerTransport.MAXIMUM_TEXT_ENVELOPE_BYTES,
          "Invalid Kagemusha Nearby payload");
      if (messageKind == MessageKind.REJECTED) {
        require(
            REJECTION_CONTENT_TYPE.equals(contentType)
                && challenge == null
                && Arrays.equals(payloadBytes, REJECTION_TEXT.getBytes(StandardCharsets.UTF_8)),
            "Invalid Kagemusha Nearby rejection");
        return new Decoded(messageKind, null, null);
      }

      final KagemushaPeerTransport.Kind peerKind =
          KagemushaPeerTransport.Kind.fromContentType(contentType);
      require(peerKind != null && MessageKind.fromPeerKind(peerKind) == messageKind,
          "Invalid Kagemusha Nearby content type");
      final KagemushaPeerTransport.Payload payload =
          KagemushaPeerTransport.decode(strictUtf8(payloadBytes), peerKind);
      switch (peerKind) {
        case RECEIVE_REQUEST -> require(challenge != null,
            "Kagemusha receive request requires a pairing challenge");
        case PAYMENT, ACKNOWLEDGEMENT -> require(challenge == null,
            "Kagemusha payment and acknowledgement cannot carry a pairing challenge");
      }
      return new Decoded(messageKind, payload, challenge);
    } finally {
      Arrays.fill(payloadBytes, (byte) 0);
    }
  }

  private static byte[] encodeEnvelope(
      final String contentType,
      final MessageKind kind,
      final PairingSymbol pairingChallenge,
      final String payload) {
    require(isJsonAtom(contentType) && isJsonAtom(kind.wireValue)
            && (pairingChallenge == null || isJsonAtom(pairingChallenge.wireValue))
            && payload != null && !payload.isEmpty()
            && payload.chars().allMatch(KagemushaNearby::isBase64UrlCharacter),
        "Invalid Kagemusha Nearby envelope");
    final StringBuilder value = new StringBuilder(128 + payload.length());
    value.append("{\"contentType\":\"").append(contentType)
        .append("\",\"kind\":\"").append(kind.wireValue).append('"');
    if (pairingChallenge != null) {
      value.append(",\"pairingChallenge\":\"")
          .append(pairingChallenge.wireValue).append('"');
    }
    value.append(",\"payload\":\"").append(payload).append("\"}");
    final byte[] encoded = value.toString().getBytes(StandardCharsets.UTF_8);
    require(encoded.length <= MAXIMUM_ENVELOPE_BYTES,
        "Kagemusha Nearby envelope is too large");
    return encoded;
  }

  private static String strictUtf8(final byte[] data) {
    try {
      final CharBuffer value = StandardCharsets.UTF_8.newDecoder()
          .onMalformedInput(CodingErrorAction.REPORT)
          .onUnmappableCharacter(CodingErrorAction.REPORT)
          .decode(ByteBuffer.wrap(data));
      return value.toString();
    } catch (CharacterCodingException failure) {
      throw new IllegalArgumentException("Invalid Kagemusha Nearby UTF-8", failure);
    }
  }

  private static boolean isJsonAtom(final String value) {
    if (value == null || value.isEmpty()) return false;
    for (int index = 0; index < value.length(); index++) {
      final char character = value.charAt(index);
      if (character < 0x20 || character == '"' || character == '\\') return false;
    }
    return true;
  }

  private static boolean isBase64UrlCharacter(final int value) {
    return (value >= '0' && value <= '9')
        || (value >= 'A' && value <= 'Z')
        || (value >= 'a' && value <= 'z')
        || value == '-'
        || value == '_';
  }

  private static void require(final boolean condition, final String message) {
    if (!condition) throw new IllegalArgumentException(message);
  }

  public enum PairingSymbol {
    STARS("nearby_pairing_stars"),
    BIRD("nearby_pairing_bird"),
    MASK("nearby_pairing_mask");

    private final String wireValue;

    PairingSymbol(final String wireValue) {
      this.wireValue = wireValue;
    }

    public String wireValue() {
      return wireValue;
    }

    private static PairingSymbol fromWireValue(final String wireValue) {
      if (wireValue == null) return null;
      for (final PairingSymbol value : values()) {
        if (value.wireValue.equals(wireValue)) return value;
      }
      return null;
    }
  }

  public enum MessageKind {
    RECEIVE_REQUEST("receive_request"),
    PAYMENT("payment"),
    ACKNOWLEDGEMENT("acknowledgement"),
    REJECTED("rejected");

    private final String wireValue;

    MessageKind(final String wireValue) {
      this.wireValue = wireValue;
    }

    public String wireValue() {
      return wireValue;
    }

    private static MessageKind fromPeerKind(final KagemushaPeerTransport.Kind kind) {
      return switch (kind) {
        case RECEIVE_REQUEST -> RECEIVE_REQUEST;
        case PAYMENT -> PAYMENT;
        case ACKNOWLEDGEMENT -> ACKNOWLEDGEMENT;
      };
    }

    private static MessageKind fromWireValue(final String wireValue) {
      for (final MessageKind value : values()) {
        if (value.wireValue.equals(wireValue)) return value;
      }
      return null;
    }
  }

  /** Immutable decoded Nearby message. */
  public static final class Decoded {
    private final MessageKind messageKind;
    private final KagemushaPeerTransport.Payload payload;
    private final PairingSymbol pairingChallenge;

    private Decoded(
        final MessageKind messageKind,
        final KagemushaPeerTransport.Payload payload,
        final PairingSymbol pairingChallenge) {
      this.messageKind = Objects.requireNonNull(messageKind, "messageKind");
      this.payload = payload;
      this.pairingChallenge = pairingChallenge;
    }

    public MessageKind messageKind() {
      return messageKind;
    }

    public KagemushaPeerTransport.Payload payload() {
      return payload;
    }

    public PairingSymbol pairingChallenge() {
      return pairingChallenge;
    }
  }
}

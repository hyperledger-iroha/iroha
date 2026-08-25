package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Objects;

/** Exact PKNB1 Kagemusha Nearby envelope and fail-closed authenticated-transport policy. */
final class KagemushaNearby {
  public static final int MAXIMUM_ENVELOPE_BYTES = 32_704;
  public static final int HEADER_LENGTH = 12;
  public static final String SERVICE_NAME = KagemushaPeerTransport.NEARBY_SERVICE_NAME;
  public static final String BONJOUR_SERVICE = KagemushaPeerTransport.NEARBY_BONJOUR_SERVICE;
  public static final String DISCOVERY_PROTOCOL = "kagemusha-v2";
  public static final boolean REQUIRES_CERTIFICATE_AUTHENTICATED_ECDH_TRANSCRIPT = true;
  public static final boolean HAS_AUDITED_AUTHENTICATED_TRANSCRIPT_BACKEND = false;
  public static final boolean IS_AVAILABLE = HAS_AUDITED_AUTHENTICATED_TRANSCRIPT_BACKEND;

  private static final byte[] MAGIC = "PKNB1".getBytes(StandardCharsets.US_ASCII);

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
    final byte[] message = IrohaPeerKagemushaAdapterV1.wrap(payload).encode();
    try {
      return encodeEnvelope(
          MessageKind.fromPeerKind(payload.kind()),
          pairingChallenge == null ? 0 : pairingChallenge.code,
          message);
    } finally {
      Arrays.fill(message, (byte) 0);
    }
  }

  public static byte[] encodeRejection() {
    return encodeEnvelope(MessageKind.REJECTED, 0, new byte[0]);
  }

  public static Decoded decode(final byte[] data) {
    Objects.requireNonNull(data, "data");
    require(data.length >= HEADER_LENGTH && data.length <= MAXIMUM_ENVELOPE_BYTES
            && rangeEquals(data, 0, MAGIC),
        "Invalid Kagemusha Nearby envelope");
    final MessageKind messageKind = MessageKind.fromCode(data[5] & 0xff);
    require(messageKind != null, "Invalid Kagemusha Nearby message kind");
    final int pairingCode = data[6] & 0xff;
    require(data[7] == 0, "Invalid Kagemusha Nearby envelope flags");
    final int payloadLength = checkedLength(readU32(data, 8));
    require(payloadLength <= MAXIMUM_ENVELOPE_BYTES - HEADER_LENGTH
            && data.length == HEADER_LENGTH + payloadLength,
        "Kagemusha Nearby envelope length mismatch");

    if (messageKind == MessageKind.REJECTED) {
      require(pairingCode == 0 && payloadLength == 0,
          "Invalid Kagemusha Nearby rejection");
      return new Decoded(messageKind, null, null);
    }

    final PairingSymbol challenge = pairingCode == 0 ? null : PairingSymbol.fromCode(pairingCode);
    require(pairingCode == 0 || challenge != null,
        "Invalid Kagemusha Nearby pairing challenge");
    final KagemushaPeerTransport.Kind peerKind = messageKind.toPeerKind();
    switch (peerKind) {
      case RECEIVE_REQUEST -> require(challenge != null,
          "Kagemusha receive request requires a pairing challenge");
      case PAYMENT, ACKNOWLEDGEMENT -> require(challenge == null,
          "Kagemusha payment and acknowledgement cannot carry a pairing challenge");
    }

    final byte[] payloadBytes = Arrays.copyOfRange(data, HEADER_LENGTH, data.length);
    try {
      final IrohaPeerWireMessageV1 message = IrohaPeerWireMessageV1.decode(
          payloadBytes,
          IrohaPeerPayloadProfile.KAGEMUSHA_RECURSIVE_SPEND,
          toWireKind(peerKind));
      return new Decoded(
          messageKind,
          IrohaPeerKagemushaAdapterV1.decode(message),
          challenge);
    } catch (RuntimeException failure) {
      throw new IllegalArgumentException("Invalid Kagemusha Nearby payload", failure);
    } finally {
      Arrays.fill(payloadBytes, (byte) 0);
    }
  }

  private static byte[] encodeEnvelope(
      final MessageKind kind, final int pairingCode, final byte[] payload) {
    Objects.requireNonNull(kind, "kind");
    Objects.requireNonNull(payload, "payload");
    require(pairingCode >= 0 && pairingCode <= 3
            && payload.length <= MAXIMUM_ENVELOPE_BYTES - HEADER_LENGTH,
        "Kagemusha Nearby envelope is too large");
    final byte[] encoded = new byte[HEADER_LENGTH + payload.length];
    System.arraycopy(MAGIC, 0, encoded, 0, MAGIC.length);
    encoded[5] = (byte) kind.code;
    encoded[6] = (byte) pairingCode;
    encoded[7] = 0;
    writeU32(encoded, 8, payload.length);
    System.arraycopy(payload, 0, encoded, HEADER_LENGTH, payload.length);
    return encoded;
  }

  private static IrohaPeerPayloadKind toWireKind(final KagemushaPeerTransport.Kind kind) {
    return switch (kind) {
      case RECEIVE_REQUEST -> IrohaPeerPayloadKind.RECEIVE_REQUEST;
      case PAYMENT -> IrohaPeerPayloadKind.PAYMENT;
      case ACKNOWLEDGEMENT -> IrohaPeerPayloadKind.ACKNOWLEDGEMENT;
    };
  }

  private static boolean rangeEquals(final byte[] data, final int offset, final byte[] expected) {
    if (offset < 0 || data.length - offset < expected.length) return false;
    for (int index = 0; index < expected.length; index++) {
      if (data[offset + index] != expected[index]) return false;
    }
    return true;
  }

  private static long readU32(final byte[] data, final int offset) {
    return ((long) (data[offset] & 0xff) << 24)
        | ((long) (data[offset + 1] & 0xff) << 16)
        | ((long) (data[offset + 2] & 0xff) << 8)
        | (long) (data[offset + 3] & 0xff);
  }

  private static int checkedLength(final long value) {
    require(value <= Integer.MAX_VALUE, "Kagemusha Nearby envelope length is invalid");
    return (int) value;
  }

  private static void writeU32(final byte[] data, final int offset, final int value) {
    data[offset] = (byte) (value >>> 24);
    data[offset + 1] = (byte) (value >>> 16);
    data[offset + 2] = (byte) (value >>> 8);
    data[offset + 3] = (byte) value;
  }

  private static void require(final boolean condition, final String message) {
    if (!condition) throw new IllegalArgumentException(message);
  }

  public enum PairingSymbol {
    STARS(1, "nearby_pairing_stars"),
    BIRD(2, "nearby_pairing_bird"),
    MASK(3, "nearby_pairing_mask");

    private final int code;
    private final String wireValue;

    PairingSymbol(final int code, final String wireValue) {
      this.code = code;
      this.wireValue = wireValue;
    }

    public int code() { return code; }
    public String wireValue() { return wireValue; }

    private static PairingSymbol fromCode(final int code) {
      for (final PairingSymbol value : values()) if (value.code == code) return value;
      return null;
    }
  }

  public enum MessageKind {
    RECEIVE_REQUEST(1, "receive_request"),
    PAYMENT(2, "payment"),
    ACKNOWLEDGEMENT(3, "acknowledgement"),
    REJECTED(4, "rejected");

    private final int code;
    private final String wireValue;

    MessageKind(final int code, final String wireValue) {
      this.code = code;
      this.wireValue = wireValue;
    }

    public int code() { return code; }
    public String wireValue() { return wireValue; }

    private static MessageKind fromPeerKind(final KagemushaPeerTransport.Kind kind) {
      return switch (kind) {
        case RECEIVE_REQUEST -> RECEIVE_REQUEST;
        case PAYMENT -> PAYMENT;
        case ACKNOWLEDGEMENT -> ACKNOWLEDGEMENT;
      };
    }

    private static MessageKind fromCode(final int code) {
      for (final MessageKind value : values()) if (value.code == code) return value;
      return null;
    }

    private KagemushaPeerTransport.Kind toPeerKind() {
      return switch (this) {
        case RECEIVE_REQUEST -> KagemushaPeerTransport.Kind.RECEIVE_REQUEST;
        case PAYMENT -> KagemushaPeerTransport.Kind.PAYMENT;
        case ACKNOWLEDGEMENT -> KagemushaPeerTransport.Kind.ACKNOWLEDGEMENT;
        case REJECTED -> throw new IllegalStateException("A rejection has no peer payload");
      };
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

    public MessageKind messageKind() { return messageKind; }
    public KagemushaPeerTransport.Payload payload() { return payload; }
    public PairingSymbol pairingChallenge() { return pairingChallenge; }
  }
}

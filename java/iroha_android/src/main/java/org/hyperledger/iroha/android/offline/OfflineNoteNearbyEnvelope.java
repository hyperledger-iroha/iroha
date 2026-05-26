package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.ThreadLocalRandom;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;

/** JSON envelope for Nearby byte transports; apps bind it to Nearby Connections or Multipeer. */
public final class OfflineNoteNearbyEnvelope {
  private final Kind kind;
  private final byte[] payload;
  private final String contentType;
  private final PairingChallenge pairingChallenge;

  public OfflineNoteNearbyEnvelope(
      final Kind kind, final byte[] payload, final String contentType) {
    this(kind, payload, contentType, null);
  }

  public OfflineNoteNearbyEnvelope(
      final Kind kind,
      final byte[] payload,
      final String contentType,
      final PairingChallenge pairingChallenge) {
    this.kind = Objects.requireNonNull(kind, "kind");
    this.payload = Objects.requireNonNull(payload, "payload").clone();
    this.contentType = Objects.requireNonNull(contentType, "contentType");
    this.pairingChallenge = pairingChallenge;
    validateForTransport(kind, this.payload, contentType, pairingChallenge);
  }

  public Kind kind() {
    return kind;
  }

  public byte[] payload() {
    return payload.clone();
  }

  public String textPayload() {
    return new String(payload, StandardCharsets.UTF_8);
  }

  public String contentType() {
    return contentType;
  }

  public PairingChallenge pairingChallenge() {
    return pairingChallenge;
  }

  public byte[] encoded() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("kind", kind.wireName());
    map.put("payload", base64UrlEncode(payload));
    map.put("contentType", contentType);
    if (pairingChallenge != null) {
      map.put("pairingChallenge", pairingChallenge.assetName());
    }
    return JsonEncoder.encode(map).getBytes(StandardCharsets.UTF_8);
  }

  public OfflineNotePaymentToken paymentToken() {
    if (kind != Kind.PAYMENT) {
      throw new IllegalArgumentException("Nearby envelope is not a payment");
    }
    if (OfflineNoteTransferHandoff.PAYMENT_TOKEN_CONTENT_TYPE.equals(contentType)) {
      return OfflineNotePaymentTokenCodec.decodeNorito(payload);
    }
    if (OfflineNoteTransferHandoff.TEXT_PAYMENT_TOKEN_CONTENT_TYPE.equals(contentType)) {
      return OfflineNotePaymentTokenCodec.decodeText(textPayload());
    }
    throw new IllegalArgumentException("Nearby envelope content type is not a payment token");
  }

  public OfflineNoteReceiveRequest receiveRequest() {
    if (kind != Kind.RECEIVE_REQUEST) {
      throw new IllegalArgumentException("Nearby envelope is not a receive request");
    }
    if (OfflineNoteTransferHandoff.RECEIVE_REQUEST_CONTENT_TYPE.equals(contentType)) {
      return OfflineNoteReceiveRequestCodec.decodeNorito(payload);
    }
    if (OfflineNoteTransferHandoff.TEXT_RECEIVE_REQUEST_CONTENT_TYPE.equals(contentType)) {
      return OfflineNoteReceiveRequestCodec.decodeText(textPayload());
    }
    throw new IllegalArgumentException("Nearby envelope content type is not a receive request");
  }

  public OfflineNoteReceiptAck receiptAck() {
    if (kind != Kind.RECEIPT_ACK) {
      throw new IllegalArgumentException("Nearby envelope is not a receipt ACK");
    }
    if (OfflineNoteTransferHandoff.RECEIPT_ACK_CONTENT_TYPE.equals(contentType)) {
      return OfflineNoteReceiptAckCodec.decodeNorito(payload);
    }
    if (OfflineNoteTransferHandoff.TEXT_RECEIPT_ACK_CONTENT_TYPE.equals(contentType)) {
      return OfflineNoteReceiptAckCodec.decodeText(textPayload());
    }
    throw new IllegalArgumentException("Nearby envelope content type is not a receipt ACK");
  }

  public static OfflineNoteNearbyEnvelope decode(final byte[] bytes) {
    Objects.requireNonNull(bytes, "bytes");
    final Object parsed;
    try {
      parsed = JsonParser.parse(new String(bytes, StandardCharsets.UTF_8));
    } catch (RuntimeException ex) {
      throw new IllegalArgumentException("Invalid nearby envelope JSON", ex);
    }
    if (!(parsed instanceof Map<?, ?> root)) {
      throw new IllegalArgumentException("Nearby envelope must be a JSON object");
    }
    final List<String> allowedKeys =
        java.util.Arrays.asList("kind", "payload", "contentType", "pairingChallenge");
    for (final Object key : root.keySet()) {
      if (!(key instanceof String stringKey) || !allowedKeys.contains(stringKey)) {
        throw new IllegalArgumentException("Nearby envelope contains unknown fields");
      }
    }
    final Kind kind = Kind.fromWireName(root.get("kind") instanceof String s ? s : "");
    if (kind == null) {
      throw new IllegalArgumentException("Nearby envelope kind is invalid");
    }
    final byte[] payload = base64UrlDecode(root.get("payload") instanceof String s ? s : "");
    if (payload == null) {
      throw new IllegalArgumentException("Nearby envelope payload is invalid");
    }
    if (!(root.get("contentType") instanceof String contentType)) {
      throw new IllegalArgumentException("Nearby envelope content type is missing");
    }
    final PairingChallenge pairingChallenge = decodePairingChallenge(root.get("pairingChallenge"));
    return new OfflineNoteNearbyEnvelope(kind, payload, contentType, pairingChallenge);
  }

  private static void validateForTransport(
      final Kind kind,
      final byte[] payload,
      final String contentType,
      final PairingChallenge pairingChallenge) {
    if (payload.length == 0) {
      throw new IllegalArgumentException("Nearby envelope payload is blank");
    }
    if (payload.length > OfflineNoteNfcApduProtocol.MAX_INCOMING_PAYLOAD_BYTES) {
      throw new IllegalArgumentException("Nearby envelope payload is too large");
    }
    if (contentType.trim().isEmpty()) {
      throw new IllegalArgumentException("Nearby envelope content type is blank");
    }
    switch (kind) {
      case RECEIVE_REQUEST -> {
        if (pairingChallenge == null) {
          throw new IllegalArgumentException("Receive request envelope requires pairing challenge");
        }
        if (!isOneOf(
            contentType,
            OfflineNoteTransferHandoff.RECEIVE_REQUEST_CONTENT_TYPE,
            OfflineNoteTransferHandoff.TEXT_RECEIVE_REQUEST_CONTENT_TYPE)) {
          throw new IllegalArgumentException("Receive request envelope content type mismatch");
        }
        validateReceiveRequestPayload(payload, contentType);
      }
      case PAYMENT -> {
        if (pairingChallenge != null) {
          throw new IllegalArgumentException("Payment envelope must not include pairing challenge");
        }
        if (!isOneOf(
            contentType,
            OfflineNoteTransferHandoff.PAYMENT_TOKEN_CONTENT_TYPE,
            OfflineNoteTransferHandoff.TEXT_PAYMENT_TOKEN_CONTENT_TYPE)) {
          throw new IllegalArgumentException("Payment envelope content type mismatch");
        }
        validatePaymentPayload(payload, contentType);
      }
      case RECEIPT_ACK -> {
        if (pairingChallenge != null) {
          throw new IllegalArgumentException("Envelope must not include pairing challenge");
        }
        if (!isOneOf(
            contentType,
            OfflineNoteTransferHandoff.RECEIPT_ACK_CONTENT_TYPE,
            OfflineNoteTransferHandoff.TEXT_RECEIPT_ACK_CONTENT_TYPE)) {
          throw new IllegalArgumentException("Receipt ACK envelope content type mismatch");
        }
        validateReceiptAckPayload(payload, contentType);
      }
      case REJECTED -> {
        if (pairingChallenge != null) {
          throw new IllegalArgumentException("Envelope must not include pairing challenge");
        }
      }
    }
  }

  private static boolean isOneOf(final String value, final String first, final String second) {
    return first.equals(value) || second.equals(value);
  }

  private static void validateReceiveRequestPayload(
      final byte[] payload, final String contentType) {
    try {
      if (OfflineNoteTransferHandoff.RECEIVE_REQUEST_CONTENT_TYPE.equals(contentType)) {
        OfflineNoteReceiveRequestCodec.decodeNorito(payload);
      } else {
        OfflineNoteReceiveRequestCodec.decodeText(new String(payload, StandardCharsets.UTF_8));
      }
    } catch (RuntimeException ex) {
      throw new IllegalArgumentException("Receive request envelope payload is invalid", ex);
    }
  }

  private static void validatePaymentPayload(final byte[] payload, final String contentType) {
    try {
      if (OfflineNoteTransferHandoff.PAYMENT_TOKEN_CONTENT_TYPE.equals(contentType)) {
        OfflineNotePaymentTokenCodec.decodeNorito(payload);
      } else {
        OfflineNotePaymentTokenCodec.decodeText(new String(payload, StandardCharsets.UTF_8));
      }
    } catch (RuntimeException ex) {
      throw new IllegalArgumentException("Payment envelope payload is invalid", ex);
    }
  }

  private static void validateReceiptAckPayload(final byte[] payload, final String contentType) {
    try {
      if (OfflineNoteTransferHandoff.RECEIPT_ACK_CONTENT_TYPE.equals(contentType)) {
        OfflineNoteReceiptAckCodec.decodeNorito(payload);
      } else {
        OfflineNoteReceiptAckCodec.decodeText(new String(payload, StandardCharsets.UTF_8));
      }
    } catch (RuntimeException ex) {
      throw new IllegalArgumentException("Receipt ACK envelope payload is invalid", ex);
    }
  }

  private static PairingChallenge decodePairingChallenge(final Object value) {
    if (value == null) {
      return null;
    }
    if (value instanceof String assetName) {
      return PairingChallenge.fromAssetName(assetName);
    }
    if (value instanceof Map<?, ?> map) {
      for (final Object key : map.keySet()) {
        if (!(key instanceof String stringKey) || !"assetName".equals(stringKey)) {
          throw new IllegalArgumentException("Nearby pairing challenge contains unknown fields");
        }
      }
      final Object assetName = map.get("assetName");
      if (!(assetName instanceof String assetNameString)) {
        throw new IllegalArgumentException("Nearby pairing challenge asset name is missing");
      }
      return PairingChallenge.fromAssetName(assetNameString);
    }
    throw new IllegalArgumentException("Nearby pairing challenge is invalid");
  }

  private static String base64UrlEncode(final byte[] bytes) {
    return Base64.getUrlEncoder().withoutPadding().encodeToString(bytes);
  }

  private static byte[] base64UrlDecode(final String value) {
    if (value == null || value.trim().isEmpty() || value.contains("=")) {
      return null;
    }
    for (int i = 0; i < value.length(); i++) {
      final char c = value.charAt(i);
      final boolean valid =
          (c >= 'A' && c <= 'Z')
              || (c >= 'a' && c <= 'z')
              || (c >= '0' && c <= '9')
              || c == '-'
              || c == '_';
      if (!valid) {
        return null;
      }
    }
    try {
      return Base64.getUrlDecoder().decode(value);
    } catch (IllegalArgumentException ex) {
      return null;
    }
  }

  @Override
  public boolean equals(final Object other) {
    if (!(other instanceof OfflineNoteNearbyEnvelope that)) {
      return false;
    }
    return kind == that.kind
        && Arrays.equals(payload, that.payload)
        && contentType.equals(that.contentType)
        && Objects.equals(pairingChallenge, that.pairingChallenge);
  }

  @Override
  public int hashCode() {
    int result = kind.hashCode();
    result = 31 * result + Arrays.hashCode(payload);
    result = 31 * result + contentType.hashCode();
    result = 31 * result + Objects.hashCode(pairingChallenge);
    return result;
  }

  /** Nearby envelope message kind. */
  public enum Kind {
    RECEIVE_REQUEST("receive_request"),
    PAYMENT("payment"),
    RECEIPT_ACK("receipt_ack"),
    REJECTED("rejected");

    private final String wireName;

    Kind(final String wireName) {
      this.wireName = wireName;
    }

    public String wireName() {
      return wireName;
    }

    public static Kind fromWireName(final String value) {
      for (final Kind kind : values()) {
        if (kind.wireName.equals(value)) {
          return kind;
        }
      }
      return null;
    }
  }

  /** Human-verifiable Nearby pairing challenge. */
  public static final class PairingChallenge {
    public static final List<String> ASSET_NAMES =
        java.util.Collections.unmodifiableList(
            java.util.Arrays.asList(
                "nearby_pairing_stars", "nearby_pairing_bird", "nearby_pairing_mask"));
    public static final List<PairingChallenge> ALL_CHOICES =
        java.util.Collections.unmodifiableList(
            java.util.Arrays.asList(
                new PairingChallenge("nearby_pairing_stars"),
                new PairingChallenge("nearby_pairing_bird"),
                new PairingChallenge("nearby_pairing_mask")));

    private final String assetName;

    public PairingChallenge(final String assetName) {
      final String trimmed = Objects.requireNonNull(assetName, "assetName").trim();
      if (!ASSET_NAMES.contains(trimmed)) {
        throw new IllegalArgumentException("Unsupported nearby pairing challenge");
      }
      this.assetName = trimmed;
    }

    public static PairingChallenge fromAssetName(final String value) {
      return new PairingChallenge(value);
    }

    public static PairingChallenge random() {
      return ALL_CHOICES.get(ThreadLocalRandom.current().nextInt(ALL_CHOICES.size()));
    }

    public String assetName() {
      return assetName;
    }

    @Override
    public boolean equals(final Object other) {
      return other instanceof PairingChallenge that && assetName.equals(that.assetName);
    }

    @Override
    public int hashCode() {
      return assetName.hashCode();
    }

    @Override
    public String toString() {
      return assetName;
    }
  }
}

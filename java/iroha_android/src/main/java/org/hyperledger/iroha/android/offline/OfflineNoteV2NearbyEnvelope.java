package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;

/** JSON envelope for Nearby byte transports; apps bind it to Nearby Connections or Multipeer. */
public final class OfflineNoteV2NearbyEnvelope {
  public static final int VERSION = 1;

  private final int version;
  private final Kind kind;
  private final byte[] payload;
  private final String contentType;
  private final PairingChallenge pairingChallenge;

  public OfflineNoteV2NearbyEnvelope(
      final Kind kind, final byte[] payload, final String contentType) {
    this(kind, payload, contentType, null, VERSION);
  }

  public OfflineNoteV2NearbyEnvelope(
      final Kind kind,
      final byte[] payload,
      final String contentType,
      final PairingChallenge pairingChallenge) {
    this(kind, payload, contentType, pairingChallenge, VERSION);
  }

  public OfflineNoteV2NearbyEnvelope(
      final Kind kind,
      final byte[] payload,
      final String contentType,
      final PairingChallenge pairingChallenge,
      final int version) {
    this.version = version;
    this.kind = Objects.requireNonNull(kind, "kind");
    this.payload = Objects.requireNonNull(payload, "payload").clone();
    this.contentType = Objects.requireNonNull(contentType, "contentType");
    this.pairingChallenge = pairingChallenge;
    if (version != VERSION) {
      throw new IllegalArgumentException("Unsupported nearby envelope version");
    }
    validateForTransport(kind, this.payload, contentType, pairingChallenge);
  }

  public int version() {
    return version;
  }

  public Kind kind() {
    return kind;
  }

  public byte[] payload() {
    return payload.clone();
  }

  public String contentType() {
    return contentType;
  }

  public PairingChallenge pairingChallenge() {
    return pairingChallenge;
  }

  public byte[] encoded() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("version", version);
    map.put("kind", kind.wireName());
    map.put("payload", base64UrlEncode(payload));
    map.put("contentType", contentType);
    if (pairingChallenge != null) {
      map.put("pairingChallenge", pairingChallenge.assetName());
    }
    return JsonEncoder.encode(map).getBytes(StandardCharsets.UTF_8);
  }

  public OfflineNoteV2PaymentToken paymentToken() {
    if (kind != Kind.PAYMENT) {
      throw new IllegalArgumentException("Nearby envelope is not a payment");
    }
    if (!OfflineNoteV2TransferHandoff.PAYMENT_TOKEN_CONTENT_TYPE.equals(contentType)) {
      throw new IllegalArgumentException("Nearby envelope content type is not a payment token");
    }
    return OfflineNoteV2PaymentTokenCodec.decodeNorito(payload);
  }

  public static OfflineNoteV2NearbyEnvelope decode(final byte[] bytes) {
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
        java.util.Arrays.asList("version", "kind", "payload", "contentType", "pairingChallenge");
    for (final Object key : root.keySet()) {
      if (!(key instanceof String stringKey) || !allowedKeys.contains(stringKey)) {
        throw new IllegalArgumentException("Nearby envelope contains unknown fields");
      }
    }
    final int version = decodeIntegerVersion(root.get("version"));
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
    return new OfflineNoteV2NearbyEnvelope(
        kind, payload, contentType, pairingChallenge, version);
  }

  private static int decodeIntegerVersion(final Object value) {
    if (!(value instanceof Number number)) {
      throw new IllegalArgumentException("Nearby envelope version is missing");
    }
    final long longValue;
    if (number instanceof Byte
        || number instanceof Short
        || number instanceof Integer
        || number instanceof Long) {
      longValue = number.longValue();
    } else {
      throw new IllegalArgumentException("Nearby envelope version must be an integer");
    }
    if (longValue < Integer.MIN_VALUE || longValue > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("Nearby envelope version is out of bounds");
    }
    return (int) longValue;
  }

  private static void validateForTransport(
      final Kind kind,
      final byte[] payload,
      final String contentType,
      final PairingChallenge pairingChallenge) {
    if (payload.length == 0) {
      throw new IllegalArgumentException("Nearby envelope payload is blank");
    }
    if (payload.length > OfflineNoteV2NfcApduProtocol.MAX_INCOMING_PAYLOAD_BYTES) {
      throw new IllegalArgumentException("Nearby envelope payload is too large");
    }
    if (contentType.trim().isEmpty()) {
      throw new IllegalArgumentException("Nearby envelope content type is blank");
    }
    switch (kind) {
      case CHALLENGE -> {
        if (pairingChallenge == null) {
          throw new IllegalArgumentException("Challenge envelope requires pairing challenge");
        }
        if (!OfflineNoteV2TransferHandoff.RECEIVE_CHALLENGE_CONTENT_TYPE.equals(contentType)) {
          throw new IllegalArgumentException("Challenge envelope content type mismatch");
        }
      }
      case PAYMENT -> {
        if (pairingChallenge != null) {
          throw new IllegalArgumentException("Payment envelope must not include pairing challenge");
        }
        if (!OfflineNoteV2TransferHandoff.PAYMENT_TOKEN_CONTENT_TYPE.equals(contentType)) {
          throw new IllegalArgumentException("Payment envelope content type mismatch");
        }
        try {
          OfflineNoteV2PaymentTokenCodec.decodeNorito(payload);
        } catch (RuntimeException ex) {
          throw new IllegalArgumentException("Payment envelope payload is invalid", ex);
        }
      }
      case RECEIPT_ACK -> {
        if (pairingChallenge != null) {
          throw new IllegalArgumentException("Envelope must not include pairing challenge");
        }
        if (!OfflineNoteV2TransferHandoff.RECEIPT_ACK_CONTENT_TYPE.equals(contentType)) {
          throw new IllegalArgumentException("Receipt ACK envelope content type mismatch");
        }
      }
      case REJECTED -> {
        if (pairingChallenge != null) {
          throw new IllegalArgumentException("Envelope must not include pairing challenge");
        }
      }
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
    if (!(other instanceof OfflineNoteV2NearbyEnvelope that)) {
      return false;
    }
    return version == that.version
        && kind == that.kind
        && Arrays.equals(payload, that.payload)
        && contentType.equals(that.contentType)
        && Objects.equals(pairingChallenge, that.pairingChallenge);
  }

  @Override
  public int hashCode() {
    int result = version;
    result = 31 * result + kind.hashCode();
    result = 31 * result + Arrays.hashCode(payload);
    result = 31 * result + contentType.hashCode();
    result = 31 * result + Objects.hashCode(pairingChallenge);
    return result;
  }

  /** Nearby envelope message kind. */
  public enum Kind {
    CHALLENGE("challenge"),
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

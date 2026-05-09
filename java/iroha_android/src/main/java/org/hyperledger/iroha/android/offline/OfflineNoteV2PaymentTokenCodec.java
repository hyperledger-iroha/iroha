package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.Base64;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;

/** QR/JSON handoff codec for Offline Note V2 payment tokens. */
public final class OfflineNoteV2PaymentTokenCodec {
  public static final String TYPE = "offline_payment_token_v2";
  public static final long VERSION = 2L;
  public static final String TEXT_PREFIX = "wallet-offline-payment-v2:";

  private OfflineNoteV2PaymentTokenCodec() {}

  public static byte[] encodeJson(final OfflineNoteV2PaymentToken token) {
    Objects.requireNonNull(token, "token");
    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("version", VERSION);
    payload.put("type", TYPE);
    payload.put("invoice_id", token.paymentRequestId());
    payload.put("token_id", token.tokenIdHex());
    payload.put(
        "audit_norito_base64",
        Base64.getEncoder().encodeToString(token.audit().noritoEncoded()));
    payload.put("created_at_ms", token.createdAtMs());
    return JsonEncoder.encode(payload).getBytes(StandardCharsets.UTF_8);
  }

  public static OfflineNoteV2PaymentToken decodeJson(final byte[] payload) {
    final Map<String, Object> object = parseObject(payload);
    final long version = asLong(object.get("version"), "version");
    if (version != VERSION) {
      throw new IllegalArgumentException(
          "Offline Note V2 payment token JSON version must be " + VERSION);
    }
    if (!TYPE.equals(asString(object.get("type"), "type"))) {
      throw new IllegalArgumentException("Offline Note V2 payment token JSON type mismatch");
    }
    final String paymentRequestId =
        object.containsKey("invoice_id")
            ? asString(object.get("invoice_id"), "invoice_id")
            : asString(object.get("payment_request_id"), "payment_request_id");
    final byte[] tokenId = hexBytes(asString(object.get("token_id"), "token_id"), "token_id");
    final byte[] auditBytes =
        Base64.getDecoder()
            .decode(asString(object.get("audit_norito_base64"), "audit_norito_base64"));
    final OfflineNoteV2.AuditBundleV2 audit = OfflineNoteV2.decodeAudit(auditBytes);
    if (!java.util.Arrays.equals(audit.tokenId(), tokenId)) {
      throw new IllegalArgumentException(
          "Offline Note V2 payment token id does not match audit bundle");
    }
    return new OfflineNoteV2PaymentToken(
        paymentRequestId, tokenId, audit, asLong(object.get("created_at_ms"), "created_at_ms"));
  }

  public static String encodeText(final OfflineNoteV2PaymentToken token) {
    return TEXT_PREFIX + Base64.getEncoder().encodeToString(encodeJson(token));
  }

  public static OfflineNoteV2PaymentToken decodeText(final String text) {
    final String trimmed = Objects.requireNonNull(text, "text").trim();
    if (!trimmed.startsWith(TEXT_PREFIX)) {
      throw new IllegalArgumentException("Offline Note V2 payment token prefix missing");
    }
    return decodeJson(Base64.getDecoder().decode(trimmed.substring(TEXT_PREFIX.length())));
  }

  public static List<byte[]> encodeQrFrameBytes(final OfflineNoteV2PaymentToken token) {
    return encodeQrFrameBytes(token, new OfflineQrStream.Options());
  }

  public static List<byte[]> encodeQrFrameBytes(
      final OfflineNoteV2PaymentToken token, final OfflineQrStream.Options options) {
    return OfflineQrStream.Encoder.encodeFrameBytes(
        encodeJson(token), OfflineQrStream.PayloadKind.OFFLINE_PAYMENT_TOKEN_V2, options);
  }

  public static OfflineNoteV2PaymentToken decodeQrPayload(final byte[] payload) {
    return decodeJson(payload);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> parseObject(final byte[] payload) {
    final Object parsed =
        JsonParser.parse(new String(Objects.requireNonNull(payload, "payload"), StandardCharsets.UTF_8));
    if (!(parsed instanceof Map<?, ?>)) {
      throw new IllegalArgumentException("Offline Note V2 payment token JSON root must be an object");
    }
    return (Map<String, Object>) parsed;
  }

  private static String asString(final Object value, final String field) {
    if (!(value instanceof String)) {
      throw new IllegalArgumentException(field + " must be a non-empty string");
    }
    final String string = (String) value;
    if (string.trim().isEmpty()) {
      throw new IllegalArgumentException(field + " must be a non-empty string");
    }
    return string;
  }

  private static long asLong(final Object value, final String field) {
    if (value instanceof Number) {
      return ((Number) value).longValue();
    }
    if (value instanceof String) {
      return Long.parseLong((String) value);
    }
    throw new IllegalArgumentException(field + " must be an integer");
  }

  private static byte[] hexBytes(final String value, final String field) {
    final String normalized = value.toLowerCase(Locale.ROOT);
    if ((normalized.length() & 1) != 0) {
      throw new IllegalArgumentException(field + " must have an even hex length");
    }
    final byte[] out = new byte[normalized.length() / 2];
    for (int i = 0; i < out.length; i++) {
      final int hi = Character.digit(normalized.charAt(i * 2), 16);
      final int lo = Character.digit(normalized.charAt(i * 2 + 1), 16);
      if (hi < 0 || lo < 0) {
        throw new IllegalArgumentException(field + " must be hex");
      }
      out[i] = (byte) ((hi << 4) | lo);
    }
    return out;
  }
}

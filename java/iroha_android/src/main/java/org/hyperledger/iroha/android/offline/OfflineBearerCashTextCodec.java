package org.hyperledger.iroha.android.offline;

import java.util.Objects;

/** Bearer Cash v1 text codec over the ZK Offline Note wire payloads. */
public final class OfflineBearerCashTextCodec {
  public static final String RECEIVE_REQUEST_TEXT_PREFIX =
      "wallet-offline-bearer-cash-receive:";
  public static final String PAYMENT_TEXT_PREFIX =
      "wallet-offline-bearer-cash-payment:";
  public static final String ACK_TEXT_PREFIX = "wallet-offline-bearer-cash-ack:";

  private OfflineBearerCashTextCodec() {}

  public static String encodeReceiveRequestText(
      final OfflineBearerCashReceiveRequestV1 request) {
    return OfflineNoteReceiveRequestCodec.encodeText(
        Objects.requireNonNull(request, "request").unwrap());
  }

  public static String encodeReceiveRequestText(final OfflineNoteReceiveRequest request) {
    return OfflineNoteReceiveRequestCodec.encodeText(Objects.requireNonNull(request, "request"));
  }

  public static OfflineBearerCashReceiveRequestV1 decodeReceiveRequestText(final String text) {
    return new OfflineBearerCashReceiveRequestV1(OfflineNoteReceiveRequestCodec.decodeText(text));
  }

  public static String encodePaymentText(final OfflineBearerCashPaymentTokenV1 token) {
    return OfflineNotePaymentTokenCodec.encodeText(Objects.requireNonNull(token, "token").unwrap());
  }

  public static String encodePaymentText(final OfflineNotePaymentToken token) {
    return OfflineNotePaymentTokenCodec.encodeText(Objects.requireNonNull(token, "token"));
  }

  public static OfflineBearerCashPaymentTokenV1 decodePaymentText(final String text) {
    return new OfflineBearerCashPaymentTokenV1(OfflineNotePaymentTokenCodec.decodeText(text));
  }

  public static String encodeAckText(final OfflineBearerCashAckV1 ack) {
    return OfflineNoteReceiptAckCodec.encodeText(Objects.requireNonNull(ack, "ack").unwrap());
  }

  public static String encodeAckText(final OfflineNoteReceiptAck ack) {
    return OfflineNoteReceiptAckCodec.encodeText(Objects.requireNonNull(ack, "ack"));
  }

  public static OfflineBearerCashAckV1 decodeAckText(final String text) {
    return new OfflineBearerCashAckV1(OfflineNoteReceiptAckCodec.decodeText(text));
  }

  public static OfflineBearerCashPayloadKindV1 payloadKind(final String text) {
    final String value = Objects.requireNonNull(text, "text");
    if (hasExactUnpaddedBase64UrlPayload(value, RECEIVE_REQUEST_TEXT_PREFIX)) {
      return OfflineBearerCashPayloadKindV1.RECEIVE_REQUEST;
    }
    if (hasExactUnpaddedBase64UrlPayload(value, PAYMENT_TEXT_PREFIX)) {
      return OfflineBearerCashPayloadKindV1.PAYMENT;
    }
    if (hasExactUnpaddedBase64UrlPayload(value, ACK_TEXT_PREFIX)) {
      return OfflineBearerCashPayloadKindV1.ACK;
    }
    return null;
  }

  private static boolean hasExactUnpaddedBase64UrlPayload(
      final String text, final String prefix) {
    if (!text.startsWith(prefix)) {
      return false;
    }
    final String payload = text.substring(prefix.length());
    if (payload.isEmpty() || !payload.trim().equals(payload) || payload.indexOf('=') >= 0) {
      return false;
    }
    for (int index = 0; index < payload.length(); index++) {
      if (!isBase64UrlCharacter(payload.charAt(index))) {
        return false;
      }
    }
    return true;
  }

  private static boolean isBase64UrlCharacter(final char ch) {
    return (ch >= 'A' && ch <= 'Z')
        || (ch >= 'a' && ch <= 'z')
        || (ch >= '0' && ch <= '9')
        || ch == '-'
        || ch == '_';
  }
}

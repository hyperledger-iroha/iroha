package org.hyperledger.iroha.android.offline;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;

/** Canonical Offline Note V2 payment-token handoff helpers for QR, NFC, and nearby transports. */
public final class OfflineNoteV2TransferHandoff {
  public static final String PAYMENT_TOKEN_CONTENT_TYPE =
      "application/vnd.iroha.offline.payment-token-v2+norito";
  public static final String RECEIVE_REQUEST_CONTENT_TYPE =
      "application/vnd.iroha.offline.receive-request-v2+norito";
  public static final String RECEIPT_ACK_CONTENT_TYPE =
      "application/vnd.iroha.offline.receipt-ack-v2+norito";
  public static final String TEXT_PAYMENT_TOKEN_CONTENT_TYPE =
      "text/vnd.iroha.offline.payment-token-v2";
  public static final String TEXT_RECEIVE_REQUEST_CONTENT_TYPE =
      "text/vnd.iroha.offline.receive-request-v2";
  public static final String TEXT_RECEIPT_ACK_CONTENT_TYPE =
      "text/vnd.iroha.offline.receipt-ack-v2";
  public static final String NEARBY_SERVICE_NAME = "iroha-pay-v2";
  public static final String NFC_EXTERNAL_TYPE = "org.hyperledger.iroha:offline-payment-v2";
  public static final String DEFAULT_NFC_AID_HEX = OfflineNoteV2NfcApduProtocol.AID_HEX;
  public static final int QR_FRAME_CADENCE_MS = 500;

  public static final OfflineQrStream.Options QR_STREAMING_OPTIONS =
      new OfflineQrStream.Options(180, 2);
  public static final OfflineQrStream.Options NFC_STREAMING_OPTIONS =
      new OfflineQrStream.Options(OfflineNoteV2NfcApduProtocol.ANDROID_SAFE_CHUNK_BYTES - 20, 0);
  public static final OfflineQrStream.Options NEARBY_STREAMING_OPTIONS =
      new OfflineQrStream.Options(4096, 0);

  private OfflineNoteV2TransferHandoff() {}

  public static byte[] rawPaymentTokenBytes(final OfflineNoteV2PaymentToken token) {
    return OfflineNoteV2PaymentTokenCodec.encodeNorito(Objects.requireNonNull(token, "token"));
  }

  public static OfflineNoteV2TransferPayload paymentTokenPayload(
      final OfflineNoteV2PaymentToken token, final OfflineNoteV2TransferModality modality) {
    return new OfflineNoteV2TransferPayload(
        Objects.requireNonNull(modality, "modality"),
        PAYMENT_TOKEN_CONTENT_TYPE,
        rawPaymentTokenBytes(token));
  }

  public static OfflineNoteV2PaymentToken decodePaymentToken(
      final OfflineNoteV2TransferPayload payload) {
    final OfflineNoteV2TransferPayload checkedPayload = Objects.requireNonNull(payload, "payload");
    if (!PAYMENT_TOKEN_CONTENT_TYPE.equals(checkedPayload.contentType())) {
      throw new IllegalArgumentException("Transfer payload content type is not a payment token");
    }
    return OfflineNoteV2PaymentTokenCodec.decodeNorito(checkedPayload.payload());
  }

  public static OfflineNoteV2PaymentToken decodePaymentToken(final byte[] rawPayload) {
    return OfflineNoteV2PaymentTokenCodec.decodeNorito(
        Objects.requireNonNull(rawPayload, "rawPayload"));
  }

  public static byte[] rawReceiveRequestBytes(final OfflineNoteV2ReceiveRequest request) {
    return OfflineNoteV2ReceiveRequestCodec.encodeNorito(
        Objects.requireNonNull(request, "request"));
  }

  public static OfflineNoteV2TransferPayload receiveRequestPayload(
      final OfflineNoteV2ReceiveRequest request, final OfflineNoteV2TransferModality modality) {
    return new OfflineNoteV2TransferPayload(
        Objects.requireNonNull(modality, "modality"),
        RECEIVE_REQUEST_CONTENT_TYPE,
        rawReceiveRequestBytes(request));
  }

  public static OfflineNoteV2ReceiveRequest decodeReceiveRequest(
      final OfflineNoteV2TransferPayload payload) {
    final OfflineNoteV2TransferPayload checkedPayload = Objects.requireNonNull(payload, "payload");
    if (!RECEIVE_REQUEST_CONTENT_TYPE.equals(checkedPayload.contentType())) {
      throw new IllegalArgumentException("Transfer payload content type is not a receive request");
    }
    return OfflineNoteV2ReceiveRequestCodec.decodeNorito(checkedPayload.payload());
  }

  public static OfflineNoteV2ReceiveRequest decodeReceiveRequest(final byte[] rawPayload) {
    return OfflineNoteV2ReceiveRequestCodec.decodeNorito(
        Objects.requireNonNull(rawPayload, "rawPayload"));
  }

  public static byte[] rawReceiptAckBytes(final OfflineNoteV2ReceiptAck ack) {
    return OfflineNoteV2ReceiptAckCodec.encodeNorito(Objects.requireNonNull(ack, "ack"));
  }

  public static OfflineNoteV2TransferPayload receiptAckPayload(
      final OfflineNoteV2ReceiptAck ack, final OfflineNoteV2TransferModality modality) {
    return new OfflineNoteV2TransferPayload(
        Objects.requireNonNull(modality, "modality"),
        RECEIPT_ACK_CONTENT_TYPE,
        rawReceiptAckBytes(ack));
  }

  public static OfflineNoteV2ReceiptAck decodeReceiptAck(
      final OfflineNoteV2TransferPayload payload) {
    final OfflineNoteV2TransferPayload checkedPayload = Objects.requireNonNull(payload, "payload");
    if (!RECEIPT_ACK_CONTENT_TYPE.equals(checkedPayload.contentType())) {
      throw new IllegalArgumentException("Transfer payload content type is not a receipt ACK");
    }
    return OfflineNoteV2ReceiptAckCodec.decodeNorito(checkedPayload.payload());
  }

  public static OfflineNoteV2ReceiptAck decodeReceiptAck(final byte[] rawPayload) {
    return OfflineNoteV2ReceiptAckCodec.decodeNorito(
        Objects.requireNonNull(rawPayload, "rawPayload"));
  }

  public static List<byte[]> qrStreamingFrameBytes(final OfflineNoteV2PaymentToken token) {
    return qrStreamingFrameBytes(token, QR_STREAMING_OPTIONS);
  }

  public static List<byte[]> qrStreamingFrameBytes(
      final OfflineNoteV2PaymentToken token, final OfflineQrStream.Options options) {
    return OfflineNoteV2PaymentTokenCodec.encodeQrFrameBytes(
        Objects.requireNonNull(token, "token"), Objects.requireNonNull(options, "options"));
  }

  public static List<byte[]> qrStreamingFrameBytes(final OfflineNoteV2ReceiveRequest request) {
    return qrStreamingFrameBytes(request, QR_STREAMING_OPTIONS);
  }

  public static List<byte[]> qrStreamingFrameBytes(
      final OfflineNoteV2ReceiveRequest request, final OfflineQrStream.Options options) {
    return OfflineNoteV2ReceiveRequestCodec.encodeQrFrameBytes(
        Objects.requireNonNull(request, "request"), Objects.requireNonNull(options, "options"));
  }

  public static List<byte[]> qrStreamingFrameBytes(final OfflineNoteV2ReceiptAck ack) {
    return qrStreamingFrameBytes(ack, QR_STREAMING_OPTIONS);
  }

  public static List<byte[]> qrStreamingFrameBytes(
      final OfflineNoteV2ReceiptAck ack, final OfflineQrStream.Options options) {
    return OfflineNoteV2ReceiptAckCodec.encodeQrFrameBytes(
        Objects.requireNonNull(ack, "ack"), Objects.requireNonNull(options, "options"));
  }

  public static List<byte[]> nfcFrameBytes(final OfflineNoteV2PaymentToken token) {
    return nfcFrameBytes(token, NFC_STREAMING_OPTIONS);
  }

  public static List<byte[]> nfcFrameBytes(
      final OfflineNoteV2PaymentToken token, final OfflineQrStream.Options options) {
    return streamFrameBytes(token, options);
  }

  public static List<byte[]> nfcPaymentTokenWriteApdus(final OfflineNoteV2PaymentToken token) {
    return nfcPaymentTokenWriteApdus(token, OfflineNoteV2NfcApduProtocol.ANDROID_SAFE_CHUNK_BYTES);
  }

  public static List<byte[]> nfcPaymentTokenWriteApdus(
      final OfflineNoteV2PaymentToken token, final int maxChunkLength) {
    return OfflineNoteV2NfcApduProtocol.writePayloadApdus(
        OfflineNoteV2NfcApduProtocol.PayloadKind.PAYMENT_TOKEN,
        rawPaymentTokenBytes(token),
        maxChunkLength);
  }

  public static List<byte[]> nfcReceiptAckWriteApdus(final OfflineNoteV2ReceiptAck ack) {
    return nfcReceiptAckWriteApdus(ack, OfflineNoteV2NfcApduProtocol.ANDROID_SAFE_CHUNK_BYTES);
  }

  public static List<byte[]> nfcReceiptAckWriteApdus(
      final OfflineNoteV2ReceiptAck ack, final int maxChunkLength) {
    return OfflineNoteV2NfcApduProtocol.writePayloadApdus(
        OfflineNoteV2NfcApduProtocol.PayloadKind.RECEIPT_ACK,
        rawReceiptAckBytes(ack),
        maxChunkLength);
  }

  public static OfflineNoteV2TransferPayload nearbyPayload(final OfflineNoteV2PaymentToken token) {
    return paymentTokenPayload(token, OfflineNoteV2TransferModality.NEARBY);
  }

  public static byte[] nearbyPaymentEnvelopeBytes(final OfflineNoteV2PaymentToken token) {
    return new OfflineNoteV2NearbyEnvelope(
            OfflineNoteV2NearbyEnvelope.Kind.PAYMENT,
            rawPaymentTokenBytes(token),
            PAYMENT_TOKEN_CONTENT_TYPE)
        .encoded();
  }

  public static OfflineNoteV2PaymentToken decodeNearbyPaymentToken(final byte[] envelopeBytes) {
    return OfflineNoteV2NearbyEnvelope.decode(Objects.requireNonNull(envelopeBytes, "envelopeBytes"))
        .paymentToken();
  }

  public static byte[] nearbyReceiptAckEnvelopeBytes(final OfflineNoteV2ReceiptAck ack) {
    return new OfflineNoteV2NearbyEnvelope(
            OfflineNoteV2NearbyEnvelope.Kind.RECEIPT_ACK,
            rawReceiptAckBytes(ack),
            RECEIPT_ACK_CONTENT_TYPE)
        .encoded();
  }

  public static OfflineNoteV2ReceiptAck decodeNearbyReceiptAck(final byte[] envelopeBytes) {
    return OfflineNoteV2NearbyEnvelope.decode(Objects.requireNonNull(envelopeBytes, "envelopeBytes"))
        .receiptAck();
  }

  public static List<byte[]> nearbyFrameBytes(final OfflineNoteV2PaymentToken token) {
    return nearbyFrameBytes(token, NEARBY_STREAMING_OPTIONS);
  }

  public static List<byte[]> nearbyFrameBytes(
      final OfflineNoteV2PaymentToken token, final OfflineQrStream.Options options) {
    return streamFrameBytes(token, options);
  }

  private static List<byte[]> streamFrameBytes(
      final OfflineNoteV2PaymentToken token, final OfflineQrStream.Options options) {
    return OfflineQrStream.Encoder.encodeFrameBytes(
        rawPaymentTokenBytes(token),
        OfflineQrStream.PayloadKind.OFFLINE_PAYMENT_TOKEN_V2,
        Objects.requireNonNull(options, "options"));
  }

  /** App-facing transfer modality for Offline Note V2 payment-token handoff. */
  public enum OfflineNoteV2TransferModality {
    QR_STREAMING,
    NFC,
    NEARBY
  }

  /** NFC availability hint; apps still own platform permission and entitlement checks. */
  public static final class OfflineNoteV2NfcCapability {
    private final boolean supported;
    private final String reason;

    private OfflineNoteV2NfcCapability(final boolean supported, final String reason) {
      this.supported = supported;
      this.reason = reason;
    }

    public static OfflineNoteV2NfcCapability supported() {
      return new OfflineNoteV2NfcCapability(true, null);
    }

    public static OfflineNoteV2NfcCapability unavailable(final String reason) {
      return new OfflineNoteV2NfcCapability(false, Objects.requireNonNull(reason, "reason"));
    }

    public boolean supportedFlag() {
      return supported;
    }

    public String reason() {
      return reason;
    }
  }

  /** Capability hints for choosing a local transfer modality in app UI. */
  public static final class OfflineNoteV2TransferCapabilities {
    private final boolean qrStreaming;
    private final OfflineNoteV2NfcCapability nfc;
    private final boolean nearby;

    public OfflineNoteV2TransferCapabilities(
        final boolean qrStreaming, final OfflineNoteV2NfcCapability nfc, final boolean nearby) {
      this.qrStreaming = qrStreaming;
      this.nfc = Objects.requireNonNull(nfc, "nfc");
      this.nearby = nearby;
    }

    public static OfflineNoteV2TransferCapabilities current(
        final boolean androidHceSupported, final boolean nearbyAvailable) {
      final OfflineNoteV2NfcCapability nfc =
          androidHceSupported
              ? OfflineNoteV2NfcCapability.supported()
              : OfflineNoteV2NfcCapability.unavailable(
                  "Android NFC payment-token transfer requires device HCE support and an app HostApduService.");
      return new OfflineNoteV2TransferCapabilities(true, nfc, nearbyAvailable);
    }

    public boolean qrStreaming() {
      return qrStreaming;
    }

    public OfflineNoteV2NfcCapability nfc() {
      return nfc;
    }

    public boolean nearby() {
      return nearby;
    }

    public List<OfflineNoteV2TransferModality> supportedModalities() {
      final List<OfflineNoteV2TransferModality> modalities = new ArrayList<>();
      if (qrStreaming) {
        modalities.add(OfflineNoteV2TransferModality.QR_STREAMING);
      }
      if (nfc.supportedFlag()) {
        modalities.add(OfflineNoteV2TransferModality.NFC);
      }
      if (nearby) {
        modalities.add(OfflineNoteV2TransferModality.NEARBY);
      }
      return Collections.unmodifiableList(modalities);
    }
  }

  /** Canonical payment-token bytes plus modality metadata for framework-specific transports. */
  public static final class OfflineNoteV2TransferPayload {
    private final OfflineNoteV2TransferModality modality;
    private final String contentType;
    private final byte[] payload;

    public OfflineNoteV2TransferPayload(
        final OfflineNoteV2TransferModality modality,
        final String contentType,
        final byte[] payload) {
      this.modality = Objects.requireNonNull(modality, "modality");
      this.contentType = Objects.requireNonNull(contentType, "contentType");
      this.payload = Objects.requireNonNull(payload, "payload").clone();
    }

    public OfflineNoteV2TransferModality modality() {
      return modality;
    }

    public String contentType() {
      return contentType;
    }

    public byte[] payload() {
      return payload.clone();
    }
  }

  /** Result of ingesting a streamed QR/NFC/Nearby frame. */
  public static final class OfflineNoteV2TransferStreamResult {
    private final byte[] payload;
    private final OfflineNoteV2PaymentToken token;
    private final OfflineNoteV2ReceiveRequest receiveRequest;
    private final OfflineNoteV2ReceiptAck receiptAck;
    private final int receivedChunks;
    private final int totalChunks;
    private final int recoveredChunks;

    private OfflineNoteV2TransferStreamResult(
        final byte[] payload,
        final OfflineNoteV2PaymentToken token,
        final OfflineNoteV2ReceiveRequest receiveRequest,
        final OfflineNoteV2ReceiptAck receiptAck,
        final int receivedChunks,
        final int totalChunks,
        final int recoveredChunks) {
      this.payload = payload == null ? null : payload.clone();
      this.token = token;
      this.receiveRequest = receiveRequest;
      this.receiptAck = receiptAck;
      this.receivedChunks = receivedChunks;
      this.totalChunks = totalChunks;
      this.recoveredChunks = recoveredChunks;
    }

    public byte[] payload() {
      return payload == null ? null : payload.clone();
    }

    public OfflineNoteV2PaymentToken token() {
      return token;
    }

    public OfflineNoteV2ReceiveRequest receiveRequest() {
      return receiveRequest;
    }

    public OfflineNoteV2ReceiptAck receiptAck() {
      return receiptAck;
    }

    public boolean isComplete() {
      return payload != null;
    }

    public int receivedChunks() {
      return receivedChunks;
    }

    public int totalChunks() {
      return totalChunks;
    }

    public int recoveredChunks() {
      return recoveredChunks;
    }

    public double progress() {
      return totalChunks == 0 ? 0.0 : receivedChunks / (double) totalChunks;
    }
  }

  /** Receiver for QR-compatible stream frames carried over camera, NFC APDUs, or nearby byte channels. */
  public static final class OfflineNoteV2TransferStreamReceiver {
    private final OfflineQrStream.Decoder decoder = new OfflineQrStream.Decoder();

    public OfflineNoteV2TransferStreamResult ingestFrame(final byte[] frameBytes) {
      final OfflineQrStream.DecodeResult result =
          decoder.ingest(Objects.requireNonNull(frameBytes, "frameBytes"));
      final StreamPayload streamPayload =
          result.payload() == null ? StreamPayload.empty() : decodeStreamPayload(result);
      return new OfflineNoteV2TransferStreamResult(
          result.payload(),
          streamPayload.token,
          streamPayload.receiveRequest,
          streamPayload.receiptAck,
          result.receivedChunks(),
          result.totalChunks(),
          result.recoveredChunks());
    }

    private static StreamPayload decodeStreamPayload(final OfflineQrStream.DecodeResult result) {
      if (result.payloadKind() == OfflineQrStream.PayloadKind.OFFLINE_PAYMENT_TOKEN_V2) {
        return new StreamPayload(
            OfflineNoteV2PaymentTokenCodec.decodeQrPayload(result.payload()), null, null);
      }
      if (result.payloadKind() == OfflineQrStream.PayloadKind.OFFLINE_RECEIVE_REQUEST_V2) {
        return new StreamPayload(
            null, OfflineNoteV2ReceiveRequestCodec.decodeQrPayload(result.payload()), null);
      }
      if (result.payloadKind() == OfflineQrStream.PayloadKind.OFFLINE_RECEIPT_ACK_V2) {
        return new StreamPayload(
            null, null, OfflineNoteV2ReceiptAckCodec.decodeQrPayload(result.payload()));
      }
      throw new IllegalArgumentException("QR stream payload kind is not an Offline Note V2 payload");
    }

    private static final class StreamPayload {
      private final OfflineNoteV2PaymentToken token;
      private final OfflineNoteV2ReceiveRequest receiveRequest;
      private final OfflineNoteV2ReceiptAck receiptAck;

      private StreamPayload(
          final OfflineNoteV2PaymentToken token,
          final OfflineNoteV2ReceiveRequest receiveRequest,
          final OfflineNoteV2ReceiptAck receiptAck) {
        this.token = token;
        this.receiveRequest = receiveRequest;
        this.receiptAck = receiptAck;
      }

      static StreamPayload empty() {
        return new StreamPayload(null, null, null);
      }
    }
  }
}

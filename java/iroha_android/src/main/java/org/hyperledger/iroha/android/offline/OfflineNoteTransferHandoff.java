package org.hyperledger.iroha.android.offline;

import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Objects;

/** Canonical Offline Note payment-token handoff helpers for QR, NFC, and nearby transports. */
public final class OfflineNoteTransferHandoff {
  public static final String PAYMENT_TOKEN_CONTENT_TYPE =
      "application/vnd.iroha.offline.payment-token+norito";
  public static final String RECEIVE_REQUEST_CONTENT_TYPE =
      "application/vnd.iroha.offline.receive-request+norito";
  public static final String RECEIPT_ACK_CONTENT_TYPE =
      "application/vnd.iroha.offline.receipt-ack+norito";
  public static final String TEXT_PAYMENT_TOKEN_CONTENT_TYPE =
      "text/vnd.iroha.offline.payment-token";
  public static final String TEXT_RECEIVE_REQUEST_CONTENT_TYPE =
      "text/vnd.iroha.offline.receive-request";
  public static final String TEXT_RECEIPT_ACK_CONTENT_TYPE =
      "text/vnd.iroha.offline.receipt-ack";
  public static final String NEARBY_SERVICE_NAME = "iroha-pay";
  public static final String NFC_EXTERNAL_TYPE = "org.hyperledger.iroha:offline-payment";
  public static final String DEFAULT_NFC_AID_HEX = OfflineNoteNfcApduProtocol.AID_HEX;
  public static final int QR_FRAME_CADENCE_MS = 500;

  public static final OfflineQrStream.Options QR_STREAMING_OPTIONS =
      new OfflineQrStream.Options(180, 2);
  public static final OfflineQrStream.Options NFC_STREAMING_OPTIONS =
      new OfflineQrStream.Options(OfflineNoteNfcApduProtocol.ANDROID_SAFE_CHUNK_BYTES - 20, 0);
  public static final OfflineQrStream.Options NEARBY_STREAMING_OPTIONS =
      new OfflineQrStream.Options(4096, 0);

  private OfflineNoteTransferHandoff() {}

  public static byte[] rawPaymentTokenBytes(final OfflineNotePaymentToken token) {
    return OfflineNotePaymentTokenCodec.encodeNorito(Objects.requireNonNull(token, "token"));
  }

  public static OfflineNoteTransferPayload paymentTokenPayload(
      final OfflineNotePaymentToken token, final OfflineNoteTransferModality modality) {
    return new OfflineNoteTransferPayload(
        Objects.requireNonNull(modality, "modality"),
        PAYMENT_TOKEN_CONTENT_TYPE,
        rawPaymentTokenBytes(token));
  }

  public static OfflineNotePaymentToken decodePaymentToken(
      final OfflineNoteTransferPayload payload) {
    final OfflineNoteTransferPayload checkedPayload = Objects.requireNonNull(payload, "payload");
    if (!PAYMENT_TOKEN_CONTENT_TYPE.equals(checkedPayload.contentType())) {
      throw new IllegalArgumentException("Transfer payload content type is not a payment token");
    }
    return OfflineNotePaymentTokenCodec.decodeNorito(checkedPayload.payload());
  }

  public static OfflineNotePaymentToken decodePaymentToken(final byte[] rawPayload) {
    return OfflineNotePaymentTokenCodec.decodeNorito(
        Objects.requireNonNull(rawPayload, "rawPayload"));
  }

  public static byte[] rawReceiveRequestBytes(final OfflineNoteReceiveRequest request) {
    return OfflineNoteReceiveRequestCodec.encodeNorito(
        Objects.requireNonNull(request, "request"));
  }

  public static OfflineNoteTransferPayload receiveRequestPayload(
      final OfflineNoteReceiveRequest request, final OfflineNoteTransferModality modality) {
    return new OfflineNoteTransferPayload(
        Objects.requireNonNull(modality, "modality"),
        RECEIVE_REQUEST_CONTENT_TYPE,
        rawReceiveRequestBytes(request));
  }

  public static OfflineNoteReceiveRequest decodeReceiveRequest(
      final OfflineNoteTransferPayload payload) {
    final OfflineNoteTransferPayload checkedPayload = Objects.requireNonNull(payload, "payload");
    if (!RECEIVE_REQUEST_CONTENT_TYPE.equals(checkedPayload.contentType())) {
      throw new IllegalArgumentException("Transfer payload content type is not a receive request");
    }
    return OfflineNoteReceiveRequestCodec.decodeNorito(checkedPayload.payload());
  }

  public static OfflineNoteReceiveRequest decodeReceiveRequest(final byte[] rawPayload) {
    return OfflineNoteReceiveRequestCodec.decodeNorito(
        Objects.requireNonNull(rawPayload, "rawPayload"));
  }

  public static byte[] rawReceiptAckBytes(final OfflineNoteReceiptAck ack) {
    return OfflineNoteReceiptAckCodec.encodeNorito(Objects.requireNonNull(ack, "ack"));
  }

  public static OfflineNoteTransferPayload receiptAckPayload(
      final OfflineNoteReceiptAck ack, final OfflineNoteTransferModality modality) {
    return new OfflineNoteTransferPayload(
        Objects.requireNonNull(modality, "modality"),
        RECEIPT_ACK_CONTENT_TYPE,
        rawReceiptAckBytes(ack));
  }

  public static OfflineNoteReceiptAck decodeReceiptAck(
      final OfflineNoteTransferPayload payload) {
    final OfflineNoteTransferPayload checkedPayload = Objects.requireNonNull(payload, "payload");
    if (!RECEIPT_ACK_CONTENT_TYPE.equals(checkedPayload.contentType())) {
      throw new IllegalArgumentException("Transfer payload content type is not a receipt ACK");
    }
    return OfflineNoteReceiptAckCodec.decodeNorito(checkedPayload.payload());
  }

  public static OfflineNoteReceiptAck decodeReceiptAck(final byte[] rawPayload) {
    return OfflineNoteReceiptAckCodec.decodeNorito(
        Objects.requireNonNull(rawPayload, "rawPayload"));
  }

  public static List<byte[]> qrStreamingFrameBytes(final OfflineNotePaymentToken token) {
    return qrStreamingFrameBytes(token, QR_STREAMING_OPTIONS);
  }

  public static List<byte[]> qrStreamingFrameBytes(
      final OfflineNotePaymentToken token, final OfflineQrStream.Options options) {
    return OfflineNotePaymentTokenCodec.encodeQrFrameBytes(
        Objects.requireNonNull(token, "token"), Objects.requireNonNull(options, "options"));
  }

  public static List<byte[]> qrStreamingFrameBytes(final OfflineNoteReceiveRequest request) {
    return qrStreamingFrameBytes(request, QR_STREAMING_OPTIONS);
  }

  public static List<byte[]> qrStreamingFrameBytes(
      final OfflineNoteReceiveRequest request, final OfflineQrStream.Options options) {
    return OfflineNoteReceiveRequestCodec.encodeQrFrameBytes(
        Objects.requireNonNull(request, "request"), Objects.requireNonNull(options, "options"));
  }

  public static List<byte[]> qrStreamingFrameBytes(final OfflineNoteReceiptAck ack) {
    return qrStreamingFrameBytes(ack, QR_STREAMING_OPTIONS);
  }

  public static List<byte[]> qrStreamingFrameBytes(
      final OfflineNoteReceiptAck ack, final OfflineQrStream.Options options) {
    return OfflineNoteReceiptAckCodec.encodeQrFrameBytes(
        Objects.requireNonNull(ack, "ack"), Objects.requireNonNull(options, "options"));
  }

  public static List<byte[]> nfcFrameBytes(final OfflineNotePaymentToken token) {
    return nfcFrameBytes(token, NFC_STREAMING_OPTIONS);
  }

  public static List<byte[]> nfcFrameBytes(
      final OfflineNotePaymentToken token, final OfflineQrStream.Options options) {
    return streamFrameBytes(token, options);
  }

  public static List<byte[]> nfcPaymentTokenWriteApdus(final OfflineNotePaymentToken token) {
    return nfcPaymentTokenWriteApdus(token, OfflineNoteNfcApduProtocol.ANDROID_SAFE_CHUNK_BYTES);
  }

  public static List<byte[]> nfcPaymentTokenWriteApdus(
      final OfflineNotePaymentToken token, final int maxChunkLength) {
    return OfflineNoteNfcApduProtocol.writePayloadApdus(
        OfflineNoteNfcApduProtocol.PayloadKind.PAYMENT_TOKEN,
        rawPaymentTokenBytes(token),
        maxChunkLength);
  }

  public static List<byte[]> nfcReceiptAckWriteApdus(final OfflineNoteReceiptAck ack) {
    return nfcReceiptAckWriteApdus(ack, OfflineNoteNfcApduProtocol.ANDROID_SAFE_CHUNK_BYTES);
  }

  public static List<byte[]> nfcReceiptAckWriteApdus(
      final OfflineNoteReceiptAck ack, final int maxChunkLength) {
    return OfflineNoteNfcApduProtocol.writePayloadApdus(
        OfflineNoteNfcApduProtocol.PayloadKind.RECEIPT_ACK,
        rawReceiptAckBytes(ack),
        maxChunkLength);
  }

  public static OfflineNoteTransferPayload nearbyPayload(final OfflineNotePaymentToken token) {
    return paymentTokenPayload(token, OfflineNoteTransferModality.NEARBY);
  }

  public static byte[] nearbyPaymentEnvelopeBytes(final OfflineNotePaymentToken token) {
    return new OfflineNoteNearbyEnvelope(
            OfflineNoteNearbyEnvelope.Kind.PAYMENT,
            rawPaymentTokenBytes(token),
            PAYMENT_TOKEN_CONTENT_TYPE)
        .encoded();
  }

  public static OfflineNotePaymentToken decodeNearbyPaymentToken(final byte[] envelopeBytes) {
    return OfflineNoteNearbyEnvelope.decode(Objects.requireNonNull(envelopeBytes, "envelopeBytes"))
        .paymentToken();
  }

  public static byte[] nearbyReceiptAckEnvelopeBytes(final OfflineNoteReceiptAck ack) {
    return new OfflineNoteNearbyEnvelope(
            OfflineNoteNearbyEnvelope.Kind.RECEIPT_ACK,
            rawReceiptAckBytes(ack),
            RECEIPT_ACK_CONTENT_TYPE)
        .encoded();
  }

  public static OfflineNoteReceiptAck decodeNearbyReceiptAck(final byte[] envelopeBytes) {
    return OfflineNoteNearbyEnvelope.decode(Objects.requireNonNull(envelopeBytes, "envelopeBytes"))
        .receiptAck();
  }

  public static List<byte[]> nearbyFrameBytes(final OfflineNotePaymentToken token) {
    return nearbyFrameBytes(token, NEARBY_STREAMING_OPTIONS);
  }

  public static List<byte[]> nearbyFrameBytes(
      final OfflineNotePaymentToken token, final OfflineQrStream.Options options) {
    return streamFrameBytes(token, options);
  }

  private static List<byte[]> streamFrameBytes(
      final OfflineNotePaymentToken token, final OfflineQrStream.Options options) {
    return OfflineQrStream.Encoder.encodeFrameBytes(
        rawPaymentTokenBytes(token),
        OfflineQrStream.PayloadKind.OFFLINE_PAYMENT_TOKEN,
        Objects.requireNonNull(options, "options"));
  }

  /** App-facing transfer modality for Offline Note payment-token handoff. */
  public enum OfflineNoteTransferModality {
    QR_STREAMING,
    NFC,
    NEARBY
  }

  /** NFC availability hint; apps still own platform permission and entitlement checks. */
  public static final class OfflineNoteNfcCapability {
    private final boolean supported;
    private final String reason;

    private OfflineNoteNfcCapability(final boolean supported, final String reason) {
      this.supported = supported;
      this.reason = reason;
    }

    public static OfflineNoteNfcCapability supported() {
      return new OfflineNoteNfcCapability(true, null);
    }

    public static OfflineNoteNfcCapability unavailable(final String reason) {
      return new OfflineNoteNfcCapability(false, Objects.requireNonNull(reason, "reason"));
    }

    public boolean supportedFlag() {
      return supported;
    }

    public String reason() {
      return reason;
    }
  }

  /** Capability hints for choosing a local transfer modality in app UI. */
  public static final class OfflineNoteTransferCapabilities {
    private final boolean qrStreaming;
    private final OfflineNoteNfcCapability nfc;
    private final boolean nearby;

    public OfflineNoteTransferCapabilities(
        final boolean qrStreaming, final OfflineNoteNfcCapability nfc, final boolean nearby) {
      this.qrStreaming = qrStreaming;
      this.nfc = Objects.requireNonNull(nfc, "nfc");
      this.nearby = nearby;
    }

    public static OfflineNoteTransferCapabilities current(
        final boolean androidHceSupported, final boolean nearbyAvailable) {
      final OfflineNoteNfcCapability nfc =
          androidHceSupported
              ? OfflineNoteNfcCapability.supported()
              : OfflineNoteNfcCapability.unavailable(
                  "Android NFC payment-token transfer requires device HCE support and an app HostApduService.");
      return new OfflineNoteTransferCapabilities(true, nfc, nearbyAvailable);
    }

    public boolean qrStreaming() {
      return qrStreaming;
    }

    public OfflineNoteNfcCapability nfc() {
      return nfc;
    }

    public boolean nearby() {
      return nearby;
    }

    public List<OfflineNoteTransferModality> supportedModalities() {
      final List<OfflineNoteTransferModality> modalities = new ArrayList<>();
      if (qrStreaming) {
        modalities.add(OfflineNoteTransferModality.QR_STREAMING);
      }
      if (nfc.supportedFlag()) {
        modalities.add(OfflineNoteTransferModality.NFC);
      }
      if (nearby) {
        modalities.add(OfflineNoteTransferModality.NEARBY);
      }
      return Collections.unmodifiableList(modalities);
    }
  }

  /** Canonical payment-token bytes plus modality metadata for framework-specific transports. */
  public static final class OfflineNoteTransferPayload {
    private final OfflineNoteTransferModality modality;
    private final String contentType;
    private final byte[] payload;

    public OfflineNoteTransferPayload(
        final OfflineNoteTransferModality modality,
        final String contentType,
        final byte[] payload) {
      this.modality = Objects.requireNonNull(modality, "modality");
      this.contentType = Objects.requireNonNull(contentType, "contentType");
      this.payload = Objects.requireNonNull(payload, "payload").clone();
    }

    public OfflineNoteTransferModality modality() {
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
  public static final class OfflineNoteTransferStreamResult {
    private final byte[] payload;
    private final OfflineNotePaymentToken token;
    private final OfflineNoteReceiveRequest receiveRequest;
    private final OfflineNoteReceiptAck receiptAck;
    private final int receivedChunks;
    private final int totalChunks;
    private final int recoveredChunks;

    private OfflineNoteTransferStreamResult(
        final byte[] payload,
        final OfflineNotePaymentToken token,
        final OfflineNoteReceiveRequest receiveRequest,
        final OfflineNoteReceiptAck receiptAck,
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

    public OfflineNotePaymentToken token() {
      return token;
    }

    public OfflineNoteReceiveRequest receiveRequest() {
      return receiveRequest;
    }

    public OfflineNoteReceiptAck receiptAck() {
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
  public static final class OfflineNoteTransferStreamReceiver {
    private final OfflineQrStream.Decoder decoder = new OfflineQrStream.Decoder();

    public OfflineNoteTransferStreamResult ingestFrame(final byte[] frameBytes) {
      final OfflineQrStream.DecodeResult result =
          decoder.ingest(Objects.requireNonNull(frameBytes, "frameBytes"));
      final StreamPayload streamPayload =
          result.payload() == null ? StreamPayload.empty() : decodeStreamPayload(result);
      return new OfflineNoteTransferStreamResult(
          result.payload(),
          streamPayload.token,
          streamPayload.receiveRequest,
          streamPayload.receiptAck,
          result.receivedChunks(),
          result.totalChunks(),
          result.recoveredChunks());
    }

    private static StreamPayload decodeStreamPayload(final OfflineQrStream.DecodeResult result) {
      if (result.payloadKind() == OfflineQrStream.PayloadKind.OFFLINE_PAYMENT_TOKEN) {
        return new StreamPayload(
            OfflineNotePaymentTokenCodec.decodeQrPayload(result.payload()), null, null);
      }
      if (result.payloadKind() == OfflineQrStream.PayloadKind.OFFLINE_RECEIVE_REQUEST) {
        return new StreamPayload(
            null, OfflineNoteReceiveRequestCodec.decodeQrPayload(result.payload()), null);
      }
      if (result.payloadKind() == OfflineQrStream.PayloadKind.OFFLINE_RECEIPT_ACK) {
        return new StreamPayload(
            null, null, OfflineNoteReceiptAckCodec.decodeQrPayload(result.payload()));
      }
      throw new IllegalArgumentException("QR stream payload kind is not an Offline Note payload");
    }

    private static final class StreamPayload {
      private final OfflineNotePaymentToken token;
      private final OfflineNoteReceiveRequest receiveRequest;
      private final OfflineNoteReceiptAck receiptAck;

      private StreamPayload(
          final OfflineNotePaymentToken token,
          final OfflineNoteReceiveRequest receiveRequest,
          final OfflineNoteReceiptAck receiptAck) {
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

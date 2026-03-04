package org.hyperledger.iroha.android.connect;

import java.util.Objects;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/** Norito codec for encrypted Connect envelopes used by wallet-role flows. */
public final class ConnectEnvelopeCodec {

  private static final String ENVELOPE_SCHEMA_PATH = "iroha_torii_shared::connect::EnvelopeV1";
  private static final int CONNECT_LAYOUT_FLAGS = 0;

  private static final int PAYLOAD_CONTROL = 0;
  private static final int PAYLOAD_SIGN_REQUEST_RAW = 1;
  private static final int PAYLOAD_SIGN_REQUEST_TX = 2;
  private static final int PAYLOAD_SIGN_RESULT_OK = 3;
  private static final int PAYLOAD_SIGN_RESULT_ERR = 4;
  private static final int PAYLOAD_DISPLAY_REQUEST = 5;

  private static final int CONTROL_CLOSE = 0;
  private static final int CONTROL_REJECT = 1;

  private static final TypeAdapter<Long> UINT16 = NoritoAdapters.uint(16);
  private static final TypeAdapter<Long> UINT32 = NoritoAdapters.uint(32);
  private static final TypeAdapter<Long> UINT64 = NoritoAdapters.uint(64);
  private static final TypeAdapter<String> STRING = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<Boolean> BOOL = NoritoAdapters.boolAdapter();
  private static final TypeAdapter<byte[]> BYTE_VECTOR = NoritoAdapters.byteVecAdapter();
  private static final TypeAdapter<byte[]> RAW_BYTES = NoritoAdapters.rawByteVecAdapter();

  private ConnectEnvelopeCodec() {}

  public enum PayloadKind {
    CONTROL_CLOSE,
    CONTROL_REJECT,
    SIGN_REQUEST_RAW,
    SIGN_REQUEST_TX,
    SIGN_RESULT_OK,
    SIGN_RESULT_ERR,
    DISPLAY_REQUEST,
    UNKNOWN
  }

  public interface EnvelopePayload {
    PayloadKind kind();
  }

  public static final class UnknownPayload implements EnvelopePayload {
    private final int tag;

    UnknownPayload(final int tag) {
      this.tag = tag;
    }

    public int tag() {
      return tag;
    }

    @Override
    public PayloadKind kind() {
      return PayloadKind.UNKNOWN;
    }
  }

  public static final class ControlClosePayload implements EnvelopePayload {
    private final ConnectFrameCodec.ConnectRole role;
    private final int code;
    private final String reason;
    private final boolean retryable;

    ControlClosePayload(
        final ConnectFrameCodec.ConnectRole role,
        final int code,
        final String reason,
        final boolean retryable) {
      this.role = role;
      this.code = code;
      this.reason = reason;
      this.retryable = retryable;
    }

    public ConnectFrameCodec.ConnectRole role() {
      return role;
    }

    public int code() {
      return code;
    }

    public String reason() {
      return reason;
    }

    public boolean retryable() {
      return retryable;
    }

    @Override
    public PayloadKind kind() {
      return PayloadKind.CONTROL_CLOSE;
    }
  }

  public static final class ControlRejectPayload implements EnvelopePayload {
    private final int code;
    private final String codeId;
    private final String reason;

    ControlRejectPayload(final int code, final String codeId, final String reason) {
      this.code = code;
      this.codeId = codeId;
      this.reason = reason;
    }

    public int code() {
      return code;
    }

    public String codeId() {
      return codeId;
    }

    public String reason() {
      return reason;
    }

    @Override
    public PayloadKind kind() {
      return PayloadKind.CONTROL_REJECT;
    }
  }

  public static final class SignRequestRawPayload implements EnvelopePayload {
    private final String domainTag;
    private final byte[] bytes;

    SignRequestRawPayload(final String domainTag, final byte[] bytes) {
      this.domainTag = domainTag;
      this.bytes = bytes.clone();
    }

    public String domainTag() {
      return domainTag;
    }

    public byte[] bytes() {
      return bytes.clone();
    }

    @Override
    public PayloadKind kind() {
      return PayloadKind.SIGN_REQUEST_RAW;
    }
  }

  public static final class SignRequestTxPayload implements EnvelopePayload {
    private final byte[] txBytes;

    SignRequestTxPayload(final byte[] txBytes) {
      this.txBytes = txBytes.clone();
    }

    public byte[] txBytes() {
      return txBytes.clone();
    }

    @Override
    public PayloadKind kind() {
      return PayloadKind.SIGN_REQUEST_TX;
    }
  }

  public static final class SignResultOkPayload implements EnvelopePayload {
    private final String algorithm;
    private final byte[] signature;

    SignResultOkPayload(final String algorithm, final byte[] signature) {
      this.algorithm = algorithm;
      this.signature = signature.clone();
    }

    public String algorithm() {
      return algorithm;
    }

    public byte[] signature() {
      return signature.clone();
    }

    @Override
    public PayloadKind kind() {
      return PayloadKind.SIGN_RESULT_OK;
    }
  }

  public static final class SignResultErrPayload implements EnvelopePayload {
    private final String code;
    private final String message;

    SignResultErrPayload(final String code, final String message) {
      this.code = code;
      this.message = message;
    }

    public String code() {
      return code;
    }

    public String message() {
      return message;
    }

    @Override
    public PayloadKind kind() {
      return PayloadKind.SIGN_RESULT_ERR;
    }
  }

  public static final class DisplayRequestPayload implements EnvelopePayload {
    private final String title;
    private final String body;

    DisplayRequestPayload(final String title, final String body) {
      this.title = title;
      this.body = body;
    }

    public String title() {
      return title;
    }

    public String body() {
      return body;
    }

    @Override
    public PayloadKind kind() {
      return PayloadKind.DISPLAY_REQUEST;
    }
  }

  public static final class DecodedEnvelope {
    private final long sequence;
    private final EnvelopePayload payload;

    DecodedEnvelope(final long sequence, final EnvelopePayload payload) {
      this.sequence = sequence;
      this.payload = payload;
    }

    public long sequence() {
      return sequence;
    }

    public EnvelopePayload payload() {
      return payload;
    }
  }

  private static final class EnvelopeValue {
    private final long sequence;
    private final EnvelopePayload payload;

    EnvelopeValue(final long sequence, final EnvelopePayload payload) {
      this.sequence = sequence;
      this.payload = payload;
    }
  }

  private static final class WalletSignature {
    private final int algorithm;
    private final byte[] signature;

    WalletSignature(final int algorithm, final byte[] signature) {
      this.algorithm = algorithm;
      this.signature = signature.clone();
    }
  }

  private static final TypeAdapter<ConnectFrameCodec.ConnectRole> ROLE_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(
            final NoritoEncoder encoder, final ConnectFrameCodec.ConnectRole value) {
          UINT32.encode(encoder, value == ConnectFrameCodec.ConnectRole.WALLET ? 1L : 0L);
        }

        @Override
        public ConnectFrameCodec.ConnectRole decode(final NoritoDecoder decoder) {
          final int tag = UINT32.decode(decoder).intValue();
          if (tag == 0) {
            return ConnectFrameCodec.ConnectRole.APP;
          }
          if (tag == 1) {
            return ConnectFrameCodec.ConnectRole.WALLET;
          }
          throw new IllegalArgumentException("Unknown Connect role tag: " + tag);
        }
      };

  private static final TypeAdapter<WalletSignature> WALLET_SIGNATURE_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final WalletSignature value) {
          // Rust `WalletSignatureV1` is encoded as a Norito struct with per-field lengths.
          final byte[] algorithmField;
          {
            final NoritoEncoder child = encoder.childEncoder();
            UINT32.encode(child, (long) value.algorithm);
            algorithmField = child.toByteArray();
          }
          final byte[] signatureField;
          {
            final NoritoEncoder child = encoder.childEncoder();
            BYTE_VECTOR.encode(child, value.signature);
            signatureField = child.toByteArray();
          }
          final boolean compactLen = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
          encoder.writeLength(algorithmField.length, compactLen);
          encoder.writeBytes(algorithmField);
          encoder.writeLength(signatureField.length, compactLen);
          encoder.writeBytes(signatureField);
        }

        @Override
        public WalletSignature decode(final NoritoDecoder decoder) {
          final boolean compactLen = decoder.compactLenActive();

          final long algorithmLength = decoder.readLength(compactLen);
          final byte[] algorithmBytes = decoder.readBytes((int) algorithmLength);
          final NoritoDecoder algorithmDecoder =
              new NoritoDecoder(algorithmBytes, decoder.flags(), decoder.flagsHint());
          final int algorithm = UINT32.decode(algorithmDecoder).intValue();
          if (algorithmDecoder.remaining() != 0) {
            throw new IllegalArgumentException(
                "wallet_signature.algorithm trailing bytes: " + algorithmDecoder.remaining());
          }

          final long signatureLength = decoder.readLength(compactLen);
          final byte[] signatureBytes = decoder.readBytes((int) signatureLength);
          final NoritoDecoder signatureDecoder =
              new NoritoDecoder(signatureBytes, decoder.flags(), decoder.flagsHint());
          final byte[] signature = BYTE_VECTOR.decode(signatureDecoder);
          if (signatureDecoder.remaining() != 0) {
            throw new IllegalArgumentException(
                "wallet_signature.signature trailing bytes: " + signatureDecoder.remaining());
          }
          return new WalletSignature(algorithm, signature);
        }
      };

  private static final TypeAdapter<EnvelopePayload> CONTROL_AFTER_KEY_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final EnvelopePayload value) {
          if (value instanceof ControlClosePayload close) {
            UINT32.encode(encoder, (long) CONTROL_CLOSE);
            writeField(encoder, ROLE_ADAPTER, close.role);
            writeField(encoder, UINT16, (long) close.code);
            writeField(encoder, STRING, close.reason);
            writeField(encoder, BOOL, close.retryable);
            return;
          }
          if (value instanceof ControlRejectPayload reject) {
            UINT32.encode(encoder, (long) CONTROL_REJECT);
            writeField(encoder, UINT16, (long) reject.code);
            writeField(encoder, STRING, reject.codeId);
            writeField(encoder, STRING, reject.reason);
            return;
          }
          throw new IllegalArgumentException("Unsupported control payload: " + value.getClass().getName());
        }

        @Override
        public EnvelopePayload decode(final NoritoDecoder decoder) {
          final int tag = UINT32.decode(decoder).intValue();
          if (tag == CONTROL_CLOSE) {
            final ConnectFrameCodec.ConnectRole role =
                decodeField(decoder, ROLE_ADAPTER, "control.close.role");
            final int code = decodeField(decoder, UINT16, "control.close.code").intValue();
            final String reason = decodeField(decoder, STRING, "control.close.reason");
            final boolean retryable = decodeField(decoder, BOOL, "control.close.retryable");
            return new ControlClosePayload(role, code, reason, retryable);
          }
          if (tag == CONTROL_REJECT) {
            final int code = decodeField(decoder, UINT16, "control.reject.code").intValue();
            final String codeId = decodeField(decoder, STRING, "control.reject.code_id");
            final String reason = decodeField(decoder, STRING, "control.reject.reason");
            return new ControlRejectPayload(code, codeId, reason);
          }
          return new UnknownPayload(tag);
        }
      };

  private static final TypeAdapter<EnvelopePayload> PAYLOAD_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final EnvelopePayload value) {
          if (value instanceof ControlClosePayload || value instanceof ControlRejectPayload) {
            UINT32.encode(encoder, (long) PAYLOAD_CONTROL);
            writeField(encoder, CONTROL_AFTER_KEY_ADAPTER, value);
          } else if (value instanceof SignRequestRawPayload raw) {
            UINT32.encode(encoder, (long) PAYLOAD_SIGN_REQUEST_RAW);
            writeField(encoder, STRING, raw.domainTag);
            writeField(encoder, RAW_BYTES, raw.bytes);
          } else if (value instanceof SignRequestTxPayload tx) {
            UINT32.encode(encoder, (long) PAYLOAD_SIGN_REQUEST_TX);
            writeField(encoder, RAW_BYTES, tx.txBytes);
          } else if (value instanceof SignResultOkPayload ok) {
            UINT32.encode(encoder, (long) PAYLOAD_SIGN_RESULT_OK);
            writeField(
                encoder,
                WALLET_SIGNATURE_ADAPTER,
                new WalletSignature(algorithmTag(ok.algorithm), ok.signature));
          } else if (value instanceof SignResultErrPayload err) {
            UINT32.encode(encoder, (long) PAYLOAD_SIGN_RESULT_ERR);
            writeField(encoder, STRING, err.code);
            writeField(encoder, STRING, err.message);
          } else if (value instanceof DisplayRequestPayload display) {
            UINT32.encode(encoder, (long) PAYLOAD_DISPLAY_REQUEST);
            writeField(encoder, STRING, display.title);
            writeField(encoder, STRING, display.body);
          } else {
            throw new IllegalArgumentException(
                "Unsupported envelope payload: " + value.getClass().getName());
          }
        }

        @Override
        public EnvelopePayload decode(final NoritoDecoder decoder) {
          final int tag = UINT32.decode(decoder).intValue();

          final EnvelopePayload payload;
          if (tag == PAYLOAD_CONTROL) {
            payload = decodeField(decoder, CONTROL_AFTER_KEY_ADAPTER, "payload.control");
          } else if (tag == PAYLOAD_SIGN_REQUEST_RAW) {
            final String domainTag =
                decodeField(decoder, STRING, "payload.sign_request_raw.domain_tag");
            final byte[] bytes =
                decodeField(decoder, RAW_BYTES, "payload.sign_request_raw.bytes");
            payload = new SignRequestRawPayload(domainTag, bytes);
          } else if (tag == PAYLOAD_SIGN_REQUEST_TX) {
            final byte[] txBytes =
                decodeField(decoder, RAW_BYTES, "payload.sign_request_tx.tx_bytes");
            payload = new SignRequestTxPayload(txBytes);
          } else if (tag == PAYLOAD_SIGN_RESULT_OK) {
            final WalletSignature signature =
                decodeField(decoder, WALLET_SIGNATURE_ADAPTER, "payload.sign_result_ok.signature");
            payload =
                new SignResultOkPayload(
                    algorithmName(signature.algorithm),
                    signature.signature);
          } else if (tag == PAYLOAD_SIGN_RESULT_ERR) {
            final String code = decodeField(decoder, STRING, "payload.sign_result_err.code");
            final String message =
                decodeField(decoder, STRING, "payload.sign_result_err.message");
            payload = new SignResultErrPayload(code, message);
          } else if (tag == PAYLOAD_DISPLAY_REQUEST) {
            final String title =
                decodeField(decoder, STRING, "payload.display_request.title");
            final String body =
                decodeField(decoder, STRING, "payload.display_request.body");
            payload = new DisplayRequestPayload(title, body);
          } else {
            decoder.readBytes(decoder.remaining());
            return new UnknownPayload(tag);
          }
          return payload;
        }
      };

  private static final TypeAdapter<EnvelopeValue> ENVELOPE_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final EnvelopeValue value) {
          writeField(encoder, UINT64, value.sequence);
          writeField(encoder, PAYLOAD_ADAPTER, value.payload);
        }

        @Override
        public EnvelopeValue decode(final NoritoDecoder decoder) {
          final long sequence = decodeField(decoder, UINT64, "envelope.seq");
          final EnvelopePayload payload = decodeField(decoder, PAYLOAD_ADAPTER, "envelope.payload");
          return new EnvelopeValue(sequence, payload);
        }
      };

  public static DecodedEnvelope decodeEnvelope(final byte[] framedEnvelope)
      throws ConnectProtocolException {
    Objects.requireNonNull(framedEnvelope, "framedEnvelope");
    try {
      final EnvelopeValue decoded = NoritoCodec.decode(framedEnvelope, ENVELOPE_ADAPTER, null);
      return new DecodedEnvelope(decoded.sequence, decoded.payload);
    } catch (final RuntimeException ex) {
      final DecodedEnvelope compatDecoded = tryDecodeEnvelopeCompat(framedEnvelope);
      if (compatDecoded != null) {
        return compatDecoded;
      }
      throw new ConnectProtocolException("Failed to decode Connect envelope", ex);
    }
  }

  private static DecodedEnvelope tryDecodeEnvelopeCompat(final byte[] framedEnvelope) {
    try {
      final NoritoCodec.ArchiveView archive = NoritoCodec.fromBytesView(framedEnvelope, null);
      final NoritoDecoder decoder =
          new NoritoDecoder(archive.asBytes(), archive.flags(), archive.flagsHint());

      final boolean compactLen = decoder.compactLenActive();
      final long seqFieldLen = decoder.readLength(compactLen);
      if (seqFieldLen <= 0 || seqFieldLen > Integer.MAX_VALUE || seqFieldLen > decoder.remaining()) {
        return null;
      }
      final byte[] seqFieldBytes = decoder.readBytes((int) seqFieldLen);
      final NoritoDecoder seqDecoder =
          new NoritoDecoder(seqFieldBytes, archive.flags(), archive.flagsHint());
      final long sequence = UINT64.decode(seqDecoder);
      if (seqDecoder.remaining() != 0) {
        return null;
      }

      final long payloadFieldLen = decoder.readLength(compactLen);
      if (payloadFieldLen <= 0 || payloadFieldLen > Integer.MAX_VALUE || payloadFieldLen > decoder.remaining()) {
        return null;
      }
      final byte[] payloadFieldBytes = decoder.readBytes((int) payloadFieldLen);
      if (decoder.remaining() != 0) {
        return null;
      }

      final EnvelopePayload payload =
          decodeCompatPayload(payloadFieldBytes, archive.flags(), archive.flagsHint());
      if (payload == null) {
        return null;
      }
      return new DecodedEnvelope(sequence, payload);
    } catch (final RuntimeException ex) {
      return null;
    }
  }

  private static EnvelopePayload decodeCompatPayload(
      final byte[] payloadFieldBytes, final int flags, final int flagsHint) {
    final NoritoDecoder payloadDecoder = new NoritoDecoder(payloadFieldBytes, flags, flagsHint);
    final int tag = UINT32.decode(payloadDecoder).intValue();
    final boolean compactLen = payloadDecoder.compactLenActive();
    final long bodyLength = payloadDecoder.readLength(compactLen);
    if (bodyLength < 0 || bodyLength > Integer.MAX_VALUE || bodyLength > payloadDecoder.remaining()) {
      return null;
    }
    final byte[] bodyBytes = payloadDecoder.readBytes((int) bodyLength);
    if (payloadDecoder.remaining() != 0) {
      return null;
    }

    final NoritoDecoder bodyDecoder = new NoritoDecoder(bodyBytes, flags, flagsHint);
    final EnvelopePayload payload;
    if (tag == PAYLOAD_CONTROL) {
      payload = CONTROL_AFTER_KEY_ADAPTER.decode(bodyDecoder);
      if (bodyDecoder.remaining() != 0) {
        return null;
      }
      return payload;
    }
    if (tag == PAYLOAD_SIGN_REQUEST_RAW) {
      final String domainTag = decodeField(bodyDecoder, STRING, "payload.sign_request_raw.domain_tag");
      final byte[] bytes = decodeField(bodyDecoder, RAW_BYTES, "payload.sign_request_raw.bytes");
      if (bodyDecoder.remaining() != 0) {
        return null;
      }
      return new SignRequestRawPayload(domainTag, bytes);
    }
    if (tag == PAYLOAD_SIGN_REQUEST_TX) {
      final byte[] txBytes = decodeRawBytesOrBody(bodyBytes, flags, flagsHint);
      return new SignRequestTxPayload(txBytes);
    }
    if (tag == PAYLOAD_SIGN_RESULT_OK) {
      final WalletSignature signature = WALLET_SIGNATURE_ADAPTER.decode(bodyDecoder);
      if (bodyDecoder.remaining() != 0) {
        return null;
      }
      return new SignResultOkPayload(algorithmName(signature.algorithm), signature.signature);
    }
    if (tag == PAYLOAD_SIGN_RESULT_ERR) {
      final String code = decodeField(bodyDecoder, STRING, "payload.sign_result_err.code");
      final String message = decodeField(bodyDecoder, STRING, "payload.sign_result_err.message");
      if (bodyDecoder.remaining() != 0) {
        return null;
      }
      return new SignResultErrPayload(code, message);
    }
    if (tag == PAYLOAD_DISPLAY_REQUEST) {
      final String title = decodeField(bodyDecoder, STRING, "payload.display_request.title");
      final String body = decodeField(bodyDecoder, STRING, "payload.display_request.body");
      if (bodyDecoder.remaining() != 0) {
        return null;
      }
      return new DisplayRequestPayload(title, body);
    }
    return null;
  }

  private static byte[] decodeRawBytesOrBody(
      final byte[] bodyBytes, final int flags, final int flagsHint) {
    try {
      final NoritoDecoder bodyDecoder = new NoritoDecoder(bodyBytes, flags, flagsHint);
      final byte[] decoded = RAW_BYTES.decode(bodyDecoder);
      if (bodyDecoder.remaining() == 0) {
        return decoded;
      }
    } catch (final RuntimeException ignored) {
      // Fallback to raw body bytes below.
    }
    return bodyBytes.clone();
  }

  public static byte[] encodeSignResultOkEnvelope(
      final long sequence, final byte[] signature, final String algorithm)
      throws ConnectProtocolException {
    Objects.requireNonNull(signature, "signature");
    return encodeEnvelope(new EnvelopeValue(sequence, new SignResultOkPayload(algorithm, signature)));
  }

  public static byte[] encodeSignRequestRawEnvelope(
      final long sequence, final String domainTag, final byte[] bytes)
      throws ConnectProtocolException {
    return encodeEnvelope(new EnvelopeValue(sequence, new SignRequestRawPayload(domainTag, bytes)));
  }

  public static byte[] encodeSignRequestTxEnvelope(final long sequence, final byte[] txBytes)
      throws ConnectProtocolException {
    return encodeEnvelope(new EnvelopeValue(sequence, new SignRequestTxPayload(txBytes)));
  }

  public static byte[] encodeSignResultErrEnvelope(
      final long sequence, final String code, final String message)
      throws ConnectProtocolException {
    return encodeEnvelope(new EnvelopeValue(sequence, new SignResultErrPayload(code, message)));
  }

  public static byte[] encodeControlRejectEnvelope(
      final long sequence,
      final int code,
      final String codeId,
      final String reason)
      throws ConnectProtocolException {
    return encodeEnvelope(new EnvelopeValue(sequence, new ControlRejectPayload(code, codeId, reason)));
  }

  public static byte[] encodeControlCloseEnvelope(
      final long sequence,
      final ConnectFrameCodec.ConnectRole role,
      final int code,
      final String reason,
      final boolean retryable)
      throws ConnectProtocolException {
    return encodeEnvelope(new EnvelopeValue(sequence, new ControlClosePayload(role, code, reason, retryable)));
  }

  private static byte[] encodeEnvelope(final EnvelopeValue value) throws ConnectProtocolException {
    try {
      final byte[] encoded = NoritoCodec.encode(
          value,
          ENVELOPE_SCHEMA_PATH,
          ENVELOPE_ADAPTER,
          CONNECT_LAYOUT_FLAGS);
      return encoded;
    } catch (final RuntimeException ex) {
      throw new ConnectProtocolException("Failed to encode Connect envelope", ex);
    }
  }

  private static <T> void writeField(
      final NoritoEncoder encoder, final TypeAdapter<T> adapter, final T value) {
    final NoritoEncoder child = encoder.childEncoder();
    adapter.encode(child, value);
    final byte[] fieldBytes = child.toByteArray();
    final boolean compactLen = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
    encoder.writeLength(fieldBytes.length, compactLen);
    encoder.writeBytes(fieldBytes);
  }

  private static <T> T decodeField(
      final NoritoDecoder decoder, final TypeAdapter<T> adapter, final String fieldName) {
    final boolean compactLen = decoder.compactLenActive();
    final long fieldLength = decoder.readLength(compactLen);
    if (fieldLength < 0 || fieldLength > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(fieldName + " field too large: " + fieldLength);
    }
    final byte[] fieldBytes = decoder.readBytes((int) fieldLength);
    final NoritoDecoder child = new NoritoDecoder(fieldBytes, decoder.flags(), decoder.flagsHint());
    final T value = adapter.decode(child);
    if (child.remaining() != 0) {
      throw new IllegalArgumentException(
          fieldName + " trailing bytes: " + child.remaining());
    }
    return value;
  }

  private static int algorithmTag(final String algorithm) {
    if (algorithm == null || algorithm.trim().isEmpty()) {
      return 0;
    }
    if ("ed25519".equalsIgnoreCase(algorithm.trim())) {
      return 0;
    }
    throw new IllegalArgumentException("Unsupported wallet signature algorithm: " + algorithm);
  }

  private static String algorithmName(final int tag) {
    if (tag == 0) {
      return "ed25519";
    }
    return "unknown(" + tag + ")";
  }
}

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

  private static final String ENVELOPE_SCHEMA_PATH = "iroha.connect.EnvelopeV1";
  private static final int CONNECT_LAYOUT_FLAGS = 0;

  private static final int PAYLOAD_CONTROL = 0;
  private static final int PAYLOAD_SIGN_REQUEST_RAW = 1;
  private static final int PAYLOAD_SIGN_REQUEST_TX = 2;
  private static final int PAYLOAD_SIGN_RESULT_OK = 3;
  private static final int PAYLOAD_SIGN_RESULT_ERR = 4;
  private static final int PAYLOAD_DISPLAY_REQUEST = 5;

  private static final int CONTROL_CLOSE = 0;
  private static final int CONTROL_REJECT = 1;

  private static final TypeAdapter<Long> UINT8 = NoritoAdapters.uint(8);
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
            ROLE_ADAPTER.encode(encoder, close.role);
            UINT16.encode(encoder, (long) close.code);
            STRING.encode(encoder, close.reason);
            BOOL.encode(encoder, close.retryable);
            return;
          }
          if (value instanceof ControlRejectPayload reject) {
            UINT32.encode(encoder, (long) CONTROL_REJECT);
            UINT16.encode(encoder, (long) reject.code);
            STRING.encode(encoder, reject.codeId);
            STRING.encode(encoder, reject.reason);
            return;
          }
          throw new IllegalArgumentException("Unsupported control payload: " + value.getClass().getName());
        }

        @Override
        public EnvelopePayload decode(final NoritoDecoder decoder) {
          final int tag = UINT32.decode(decoder).intValue();
          if (tag == CONTROL_CLOSE) {
            final ConnectFrameCodec.ConnectRole role = ROLE_ADAPTER.decode(decoder);
            final int code = UINT16.decode(decoder).intValue();
            final String reason = STRING.decode(decoder);
            final boolean retryable = BOOL.decode(decoder);
            return new ControlClosePayload(role, code, reason, retryable);
          }
          if (tag == CONTROL_REJECT) {
            final int code = UINT16.decode(decoder).intValue();
            final String codeId = STRING.decode(decoder);
            final String reason = STRING.decode(decoder);
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
            CONTROL_AFTER_KEY_ADAPTER.encode(encoder, value);
            return;
          }
          if (value instanceof SignRequestRawPayload raw) {
            UINT32.encode(encoder, (long) PAYLOAD_SIGN_REQUEST_RAW);
            STRING.encode(encoder, raw.domainTag);
            RAW_BYTES.encode(encoder, raw.bytes);
            return;
          }
          if (value instanceof SignRequestTxPayload tx) {
            UINT32.encode(encoder, (long) PAYLOAD_SIGN_REQUEST_TX);
            RAW_BYTES.encode(encoder, tx.txBytes);
            return;
          }
          if (value instanceof SignResultOkPayload ok) {
            UINT32.encode(encoder, (long) PAYLOAD_SIGN_RESULT_OK);
            WALLET_SIGNATURE_ADAPTER.encode(
                encoder, new WalletSignature(algorithmTag(ok.algorithm), ok.signature));
            return;
          }
          if (value instanceof SignResultErrPayload err) {
            UINT32.encode(encoder, (long) PAYLOAD_SIGN_RESULT_ERR);
            STRING.encode(encoder, err.code);
            STRING.encode(encoder, err.message);
            return;
          }
          if (value instanceof DisplayRequestPayload display) {
            UINT32.encode(encoder, (long) PAYLOAD_DISPLAY_REQUEST);
            STRING.encode(encoder, display.title);
            STRING.encode(encoder, display.body);
            return;
          }
          throw new IllegalArgumentException("Unsupported envelope payload: " + value.getClass().getName());
        }

        @Override
        public EnvelopePayload decode(final NoritoDecoder decoder) {
          final int tag = UINT32.decode(decoder).intValue();
          if (tag == PAYLOAD_CONTROL) {
            return CONTROL_AFTER_KEY_ADAPTER.decode(decoder);
          }
          if (tag == PAYLOAD_SIGN_REQUEST_RAW) {
            final String domainTag = STRING.decode(decoder);
            final byte[] bytes = RAW_BYTES.decode(decoder);
            return new SignRequestRawPayload(domainTag, bytes);
          }
          if (tag == PAYLOAD_SIGN_REQUEST_TX) {
            final byte[] txBytes = RAW_BYTES.decode(decoder);
            return new SignRequestTxPayload(txBytes);
          }
          if (tag == PAYLOAD_SIGN_RESULT_OK) {
            final WalletSignature signature = WALLET_SIGNATURE_ADAPTER.decode(decoder);
            return new SignResultOkPayload(
                algorithmName(signature.algorithm),
                signature.signature);
          }
          if (tag == PAYLOAD_SIGN_RESULT_ERR) {
            final String code = STRING.decode(decoder);
            final String message = STRING.decode(decoder);
            return new SignResultErrPayload(code, message);
          }
          if (tag == PAYLOAD_DISPLAY_REQUEST) {
            final String title = STRING.decode(decoder);
            final String body = STRING.decode(decoder);
            return new DisplayRequestPayload(title, body);
          }
          return new UnknownPayload(tag);
        }
      };

  private static final TypeAdapter<EnvelopeValue> ENVELOPE_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final EnvelopeValue value) {
          UINT64.encode(encoder, value.sequence);
          PAYLOAD_ADAPTER.encode(encoder, value.payload);
        }

        @Override
        public EnvelopeValue decode(final NoritoDecoder decoder) {
          final long sequence = UINT64.decode(decoder);
          final EnvelopePayload payload = PAYLOAD_ADAPTER.decode(decoder);
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
      throw new ConnectProtocolException("Failed to decode Connect envelope", ex);
    }
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

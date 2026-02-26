package org.hyperledger.iroha.android.connect;

import java.io.ByteArrayOutputStream;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.Objects;
import java.util.Optional;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

/** Connect wire codec for frame/control payloads used by wallet-role flows. */
public final class ConnectFrameCodec {

  private static final int CONNECT_LAYOUT_FLAGS = NoritoHeader.MINOR_VERSION;

  private static final int FRAME_KIND_CONTROL = 0;
  private static final int FRAME_KIND_CIPHERTEXT = 1;

  private static final int CONTROL_OPEN = 0;
  private static final int CONTROL_APPROVE = 1;
  private static final int CONTROL_REJECT = 2;
  private static final int CONTROL_CLOSE = 3;

  private static final TypeAdapter<Long> UINT8 = NoritoAdapters.uint(8);
  private static final TypeAdapter<Long> UINT16 = NoritoAdapters.uint(16);
  private static final TypeAdapter<Long> UINT32 = NoritoAdapters.uint(32);
  private static final TypeAdapter<Long> UINT64 = NoritoAdapters.uint(64);
  private static final TypeAdapter<String> STRING = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<Boolean> BOOL = NoritoAdapters.boolAdapter();
  /**
   * Rust-side `[u8; 32]` values are encoded in canonical AoS layout (32 elements, each prefixed with
   * its element length). This adapter mirrors that exact representation.
   */
  private static final TypeAdapter<byte[]> FIXED_ARRAY_U8_32 =
      new TypeAdapter<>() {
        private static final int LENGTH = 32;

        @Override
        public void encode(final NoritoEncoder encoder, final byte[] value) {
          if (value == null || value.length != LENGTH) {
            throw new IllegalArgumentException(
                "expected " + LENGTH + " bytes, found " + (value == null ? 0 : value.length));
          }
          if ((encoder.flags() & NoritoHeader.PACKED_SEQ) != 0) {
            long offset = 0L;
            for (int i = 0; i < LENGTH; i++) {
              offset += 1L;
              encoder.writeUInt(offset, 64);
            }
            encoder.writeBytes(value);
            return;
          }
          final boolean compactLen = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
          for (byte b : value) {
            encoder.writeLength(1L, compactLen);
            encoder.writeByte(b & 0xFF);
          }
        }

        @Override
        public byte[] decode(final NoritoDecoder decoder) {
          final byte[] out = new byte[LENGTH];
          if ((decoder.flags() & NoritoHeader.PACKED_SEQ) != 0) {
            long previous = 0L;
            for (int i = 0; i < LENGTH; i++) {
              final long current = decoder.readUInt(64);
              final long delta = current - previous;
              if (delta != 1L) {
                throw new IllegalArgumentException("Invalid packed [u8;32] offset delta: " + delta);
              }
              previous = current;
            }
            for (int i = 0; i < LENGTH; i++) {
              out[i] = (byte) decoder.readByte();
            }
            return out;
          }
          final boolean compactLen = decoder.compactLenActive();
          for (int i = 0; i < LENGTH; i++) {
            final long elemLen = decoder.readLength(compactLen);
            if (elemLen != 1L) {
              throw new IllegalArgumentException("Invalid [u8;32] element length: " + elemLen);
            }
            out[i] = (byte) decoder.readByte();
          }
          return out;
        }

        @Override
        public boolean isSelfDelimiting() {
          return true;
        }
      };
  private static final TypeAdapter<byte[]> RAW_BYTES = NoritoAdapters.rawByteVecAdapter();
  private static final TypeAdapter<byte[]> BYTE_VECTOR = NoritoAdapters.byteVecAdapter();
  private static final TypeAdapter<Optional<byte[]>> OPTIONAL_PLACEHOLDER = NoritoAdapters.option(RAW_BYTES);

  private ConnectFrameCodec() {}

  public enum FrameType {
    OPEN,
    REJECT,
    CLOSE,
    CIPHERTEXT,
    OTHER_CONTROL
  }

  public enum ConnectRole {
    APP,
    WALLET
  }

  public static final class OpenControl {
    private final byte[] appPublicKey;
    private final String chainId;

    OpenControl(final byte[] appPublicKey, final String chainId) {
      this.appPublicKey = appPublicKey.clone();
      this.chainId = chainId;
    }

    public byte[] appPublicKey() {
      return appPublicKey.clone();
    }

    public String chainId() {
      return chainId;
    }
  }

  public static final class RejectControl {
    private final int code;
    private final String codeId;
    private final String reason;

    RejectControl(final int code, final String codeId, final String reason) {
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
  }

  public static final class CloseControl {
    private final ConnectRole role;
    private final int code;
    private final String reason;
    private final boolean retryable;

    CloseControl(final ConnectRole role, final int code, final String reason, final boolean retryable) {
      this.role = role;
      this.code = code;
      this.reason = reason;
      this.retryable = retryable;
    }

    public ConnectRole role() {
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
  }

  public static final class Ciphertext {
    private final ConnectDirection direction;
    private final byte[] aead;

    Ciphertext(final ConnectDirection direction, final byte[] aead) {
      this.direction = direction;
      this.aead = aead.clone();
    }

    public ConnectDirection direction() {
      return direction;
    }

    public byte[] aead() {
      return aead.clone();
    }
  }

  public static final class DecodedFrame {
    private final byte[] sessionId;
    private final ConnectDirection direction;
    private final long sequence;
    private final FrameType type;
    private final OpenControl open;
    private final RejectControl reject;
    private final CloseControl close;
    private final Ciphertext ciphertext;

    private DecodedFrame(
        final byte[] sessionId,
        final ConnectDirection direction,
        final long sequence,
        final FrameType type,
        final OpenControl open,
        final RejectControl reject,
        final CloseControl close,
        final Ciphertext ciphertext) {
      this.sessionId = sessionId.clone();
      this.direction = direction;
      this.sequence = sequence;
      this.type = type;
      this.open = open;
      this.reject = reject;
      this.close = close;
      this.ciphertext = ciphertext;
    }

    public byte[] sessionId() {
      return sessionId.clone();
    }

    public ConnectDirection direction() {
      return direction;
    }

    public long sequence() {
      return sequence;
    }

    public FrameType type() {
      return type;
    }

    public OpenControl open() {
      return open;
    }

    public RejectControl reject() {
      return reject;
    }

    public CloseControl close() {
      return close;
    }

    public Ciphertext ciphertext() {
      return ciphertext;
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

  private static final class Constraints {
    private final String chainId;

    Constraints(final String chainId) {
      this.chainId = chainId;
    }
  }

  private static final TypeAdapter<ConnectDirection> DIRECTION_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final ConnectDirection value) {
          final long tag = value == ConnectDirection.WALLET_TO_APP ? 1L : 0L;
          UINT32.encode(encoder, tag);
        }

        @Override
        public ConnectDirection decode(final NoritoDecoder decoder) {
          final int tag = UINT32.decode(decoder).intValue();
          return ConnectDirection.fromTag(tag);
        }
      };

  private static final TypeAdapter<ConnectRole> ROLE_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final ConnectRole value) {
          final long tag = value == ConnectRole.WALLET ? 1L : 0L;
          UINT32.encode(encoder, tag);
        }

        @Override
        public ConnectRole decode(final NoritoDecoder decoder) {
          final int tag = UINT32.decode(decoder).intValue();
          if (tag == 0) {
            return ConnectRole.APP;
          }
          if (tag == 1) {
            return ConnectRole.WALLET;
          }
          throw new IllegalArgumentException("Unknown Connect role tag: " + tag);
        }
      };

  private static final TypeAdapter<Constraints> CONSTRAINTS_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final Constraints value) {
          STRING.encode(encoder, value.chainId);
        }

        @Override
        public Constraints decode(final NoritoDecoder decoder) {
          return new Constraints(STRING.decode(decoder));
        }
      };

  private static final TypeAdapter<WalletSignature> WALLET_SIGNATURE_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final WalletSignature value) {
          // Rust `WalletSignatureV1` is a Norito struct with length-prefixed fields.
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

  private static final TypeAdapter<Ciphertext> CIPHERTEXT_ADAPTER =
      new TypeAdapter<>() {
        @Override
        public void encode(final NoritoEncoder encoder, final Ciphertext value) {
          // Rust `ConnectCiphertextV1` is a struct with per-field length prefixes.
          final byte[] directionField;
          {
            final NoritoEncoder child = encoder.childEncoder();
            DIRECTION_ADAPTER.encode(child, value.direction);
            directionField = child.toByteArray();
          }
          final byte[] aeadField;
          {
            final NoritoEncoder child = encoder.childEncoder();
            RAW_BYTES.encode(child, value.aead);
            aeadField = child.toByteArray();
          }
          final boolean compactLen = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
          encoder.writeLength(directionField.length, compactLen);
          encoder.writeBytes(directionField);
          encoder.writeLength(aeadField.length, compactLen);
          encoder.writeBytes(aeadField);
        }

        @Override
        public Ciphertext decode(final NoritoDecoder decoder) {
          final boolean compactLen = decoder.compactLenActive();

          final long directionLength = decoder.readLength(compactLen);
          final byte[] directionBytes = decoder.readBytes((int) directionLength);
          final NoritoDecoder directionDecoder =
              new NoritoDecoder(directionBytes, decoder.flags(), decoder.flagsHint());
          final ConnectDirection direction = DIRECTION_ADAPTER.decode(directionDecoder);
          if (directionDecoder.remaining() != 0) {
            throw new IllegalArgumentException(
                "ciphertext.direction trailing bytes: " + directionDecoder.remaining());
          }

          final long aeadLength = decoder.readLength(compactLen);
          final byte[] aeadBytes = decoder.readBytes((int) aeadLength);
          final NoritoDecoder aeadDecoder =
              new NoritoDecoder(aeadBytes, decoder.flags(), decoder.flagsHint());
          final byte[] aead = RAW_BYTES.decode(aeadDecoder);
          if (aeadDecoder.remaining() != 0) {
            throw new IllegalArgumentException(
                "ciphertext.aead trailing bytes: " + aeadDecoder.remaining());
          }
          return new Ciphertext(direction, aead);
        }
      };

  public static DecodedFrame decode(final byte[] rawFrame) throws ConnectProtocolException {
    Objects.requireNonNull(rawFrame, "rawFrame");
    final Cursor frameCursor = new Cursor(rawFrame);

    final byte[] sid = decodeLengthPrefixedField(frameCursor, FIXED_ARRAY_U8_32, "sid");
    final ConnectDirection direction =
        decodeLengthPrefixedField(frameCursor, DIRECTION_ADAPTER, "direction");
    final long sequence = decodeLengthPrefixedField(frameCursor, UINT64, "sequence");

    final long kindLength = frameCursor.readU64();
    final byte[] kindBytes = frameCursor.readBytes(asInt(kindLength, "kind length"));
    frameCursor.ensureFullyConsumed("connect frame");

    final Cursor kindCursor = new Cursor(kindBytes);
    final int kindTag = kindCursor.readU32();
    final long kindPayloadLength = kindCursor.readU64();
    final byte[] kindPayload = kindCursor.readBytes(asInt(kindPayloadLength, "kind payload length"));
    kindCursor.ensureFullyConsumed("connect frame kind");

    if (kindTag == FRAME_KIND_CIPHERTEXT) {
      final Ciphertext ciphertext = decodeField(kindPayload, CIPHERTEXT_ADAPTER, "ciphertext");
      return new DecodedFrame(sid, direction, sequence, FrameType.CIPHERTEXT, null, null, null, ciphertext);
    }

    if (kindTag != FRAME_KIND_CONTROL) {
      throw new ConnectProtocolException("Unsupported connect frame kind tag: " + kindTag);
    }

    final Cursor controlCursor = new Cursor(kindPayload);
    final int controlTag = controlCursor.readU32();
    final long controlBodyLength = controlCursor.readU64();
    final byte[] controlBody = controlCursor.readBytes(asInt(controlBodyLength, "control payload length"));
    controlCursor.ensureFullyConsumed("connect control payload");

    switch (controlTag) {
      case CONTROL_OPEN:
        return decodeOpenFrame(sid, direction, sequence, controlBody);
      case CONTROL_REJECT:
        return decodeRejectFrame(sid, direction, sequence, controlBody);
      case CONTROL_CLOSE:
        return decodeCloseFrame(sid, direction, sequence, controlBody);
      default:
        return new DecodedFrame(sid, direction, sequence, FrameType.OTHER_CONTROL, null, null, null, null);
    }
  }

  public static byte[] encodeApproveFrame(
      final byte[] sessionId,
      final long sequence,
      final byte[] walletPublicKey,
      final String accountId,
      final byte[] walletSignature)
      throws ConnectProtocolException {
    if (accountId == null || accountId.trim().isEmpty()) {
      throw new ConnectProtocolException("accountId must not be blank");
    }
    final byte[] walletPkField = encodeField(walletPublicKey, FIXED_ARRAY_U8_32, "wallet_pk");
    final byte[] accountField = encodeField(accountId, STRING, "account_id");
    final byte[] permissionsField = encodeField(Optional.empty(), OPTIONAL_PLACEHOLDER, "permissions");
    final byte[] proofField = encodeField(Optional.empty(), OPTIONAL_PLACEHOLDER, "proof");
    final WalletSignature signature = new WalletSignature(0, walletSignature);
    final byte[] signatureField = encodeField(signature, WALLET_SIGNATURE_ADAPTER, "wallet_signature");

    final ByteArrayOutputStream body = new ByteArrayOutputStream();
    writeLengthPrefixed(body, walletPkField);
    writeLengthPrefixed(body, accountField);
    writeLengthPrefixed(body, permissionsField);
    writeLengthPrefixed(body, proofField);
    writeLengthPrefixed(body, signatureField);

    final byte[] controlPayload = wrapTaggedPayload(CONTROL_APPROVE, body.toByteArray());
    final byte[] kindPayload = wrapTaggedPayload(FRAME_KIND_CONTROL, controlPayload);
    return encodeFrame(sessionId, ConnectDirection.WALLET_TO_APP, sequence, kindPayload);
  }

  public static byte[] encodeRejectFrame(
      final byte[] sessionId,
      final long sequence,
      final int code,
      final String codeId,
      final String reason)
      throws ConnectProtocolException {
    final byte[] codeField = encodeField((long) code, UINT16, "reject_code");
    final byte[] codeIdField = encodeField(codeId, STRING, "reject_code_id");
    final byte[] reasonField = encodeField(reason, STRING, "reject_reason");

    final ByteArrayOutputStream body = new ByteArrayOutputStream();
    writeLengthPrefixed(body, codeField);
    writeLengthPrefixed(body, codeIdField);
    writeLengthPrefixed(body, reasonField);

    final byte[] controlPayload = wrapTaggedPayload(CONTROL_REJECT, body.toByteArray());
    final byte[] kindPayload = wrapTaggedPayload(FRAME_KIND_CONTROL, controlPayload);
    return encodeFrame(sessionId, ConnectDirection.WALLET_TO_APP, sequence, kindPayload);
  }

  public static byte[] encodeCiphertextFrame(
      final byte[] sessionId,
      final ConnectDirection direction,
      final long sequence,
      final byte[] aead)
      throws ConnectProtocolException {
    final Ciphertext ciphertext = new Ciphertext(direction, aead);
    final byte[] cipherPayload = encodeField(ciphertext, CIPHERTEXT_ADAPTER, "ciphertext");
    final byte[] kindPayload = wrapTaggedPayload(FRAME_KIND_CIPHERTEXT, cipherPayload);
    return encodeFrame(sessionId, direction, sequence, kindPayload);
  }

  private static DecodedFrame decodeOpenFrame(
      final byte[] sid,
      final ConnectDirection direction,
      final long sequence,
      final byte[] controlBody)
      throws ConnectProtocolException {
    final Cursor cursor = new Cursor(controlBody);
    final byte[] appPk = decodeLengthPrefixedField(cursor, FIXED_ARRAY_U8_32, "open.app_pk");
    skipLengthPrefixedField(cursor, "open.app_meta");
    final Constraints constraints = decodeLengthPrefixedField(cursor, CONSTRAINTS_ADAPTER, "open.constraints");
    skipLengthPrefixedField(cursor, "open.permissions");
    cursor.ensureFullyConsumed("open control");

    final OpenControl open = new OpenControl(appPk, constraints.chainId);
    return new DecodedFrame(sid, direction, sequence, FrameType.OPEN, open, null, null, null);
  }

  private static DecodedFrame decodeRejectFrame(
      final byte[] sid,
      final ConnectDirection direction,
      final long sequence,
      final byte[] controlBody)
      throws ConnectProtocolException {
    final Cursor cursor = new Cursor(controlBody);
    final int code = decodeLengthPrefixedField(cursor, UINT16, "reject.code").intValue();
    final String codeId = decodeLengthPrefixedField(cursor, STRING, "reject.code_id");
    final String reason = decodeLengthPrefixedField(cursor, STRING, "reject.reason");
    cursor.ensureFullyConsumed("reject control");

    final RejectControl reject = new RejectControl(code, codeId, reason);
    return new DecodedFrame(sid, direction, sequence, FrameType.REJECT, null, reject, null, null);
  }

  private static DecodedFrame decodeCloseFrame(
      final byte[] sid,
      final ConnectDirection direction,
      final long sequence,
      final byte[] controlBody)
      throws ConnectProtocolException {
    final Cursor cursor = new Cursor(controlBody);
    final ConnectRole role = decodeLengthPrefixedField(cursor, ROLE_ADAPTER, "close.role");
    final int code = decodeLengthPrefixedField(cursor, UINT16, "close.code").intValue();
    final String reason = decodeLengthPrefixedField(cursor, STRING, "close.reason");
    final boolean retryable = decodeLengthPrefixedField(cursor, BOOL, "close.retryable");
    cursor.ensureFullyConsumed("close control");

    final CloseControl close = new CloseControl(role, code, reason, retryable);
    return new DecodedFrame(sid, direction, sequence, FrameType.CLOSE, null, null, close, null);
  }

  private static byte[] encodeFrame(
      final byte[] sessionId,
      final ConnectDirection direction,
      final long sequence,
      final byte[] kindPayload)
      throws ConnectProtocolException {
    final byte[] sidField = encodeField(sessionId, FIXED_ARRAY_U8_32, "sid");
    final byte[] directionField = encodeField(direction, DIRECTION_ADAPTER, "direction");
    final byte[] sequenceField = encodeField(sequence, UINT64, "sequence");

    final ByteArrayOutputStream frame = new ByteArrayOutputStream();
    writeLengthPrefixed(frame, sidField);
    writeLengthPrefixed(frame, directionField);
    writeLengthPrefixed(frame, sequenceField);
    writeLengthPrefixed(frame, kindPayload);
    return frame.toByteArray();
  }

  private static byte[] wrapTaggedPayload(final int tag, final byte[] payload) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeU32(out, tag);
    writeU64(out, payload.length);
    out.write(payload, 0, payload.length);
    return out.toByteArray();
  }

  private static <T> byte[] encodeField(
      final T value, final TypeAdapter<T> adapter, final String label) throws ConnectProtocolException {
    try {
      final NoritoCodec.AdaptiveEncoding encoding = NoritoCodec.encodeWithHeaderFlags(value, adapter);
      if (encoding.flags() != CONNECT_LAYOUT_FLAGS) {
        throw new ConnectProtocolException(
            "Unsupported Norito flags in " + label + ": " + encoding.flags());
      }
      return encoding.payload();
    } catch (final RuntimeException ex) {
      throw new ConnectProtocolException("Failed to encode " + label, ex);
    }
  }

  private static <T> T decodeField(
      final byte[] fieldBytes, final TypeAdapter<T> adapter, final String label)
      throws ConnectProtocolException {
    try (NoritoCodec.DecodeFlagsGuard ignored =
        NoritoCodec.DecodeFlagsGuard.enterWithHint(CONNECT_LAYOUT_FLAGS, CONNECT_LAYOUT_FLAGS)) {
      final NoritoDecoder decoder = new NoritoDecoder(fieldBytes, CONNECT_LAYOUT_FLAGS, CONNECT_LAYOUT_FLAGS);
      final T value = adapter.decode(decoder);
      if (decoder.remaining() != 0) {
        throw new ConnectProtocolException(
            label + " did not consume full payload (remaining=" + decoder.remaining() + ")");
      }
      return value;
    } catch (final ConnectProtocolException ex) {
      throw ex;
    } catch (final RuntimeException ex) {
      throw new ConnectProtocolException("Failed to decode " + label, ex);
    }
  }

  private static <T> T decodeLengthPrefixedField(
      final Cursor cursor, final TypeAdapter<T> adapter, final String label)
      throws ConnectProtocolException {
    final long length = cursor.readU64();
    final byte[] field = cursor.readBytes(asInt(length, label + " length"));
    return decodeField(field, adapter, label);
  }

  private static void skipLengthPrefixedField(final Cursor cursor, final String label)
      throws ConnectProtocolException {
    final long length = cursor.readU64();
    cursor.readBytes(asInt(length, label + " length"));
  }

  private static int asInt(final long value, final String label) throws ConnectProtocolException {
    if (value < 0 || value > Integer.MAX_VALUE) {
      throw new ConnectProtocolException("Invalid " + label + ": " + value);
    }
    return (int) value;
  }

  private static void writeLengthPrefixed(final ByteArrayOutputStream out, final byte[] bytes) {
    writeU64(out, bytes.length);
    out.write(bytes, 0, bytes.length);
  }

  private static void writeU32(final ByteArrayOutputStream out, final int value) {
    final ByteBuffer buffer = ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN);
    buffer.putInt(value);
    out.write(buffer.array(), 0, 4);
  }

  private static void writeU64(final ByteArrayOutputStream out, final long value) {
    final ByteBuffer buffer = ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN);
    buffer.putLong(value);
    out.write(buffer.array(), 0, 8);
  }

  private static final class Cursor {
    private final byte[] data;
    private int offset;

    Cursor(final byte[] data) {
      this.data = data;
      this.offset = 0;
    }

    int readU32() throws ConnectProtocolException {
      final byte[] bytes = readBytes(4);
      final ByteBuffer buffer = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN);
      return buffer.getInt();
    }

    long readU64() throws ConnectProtocolException {
      final byte[] bytes = readBytes(8);
      final ByteBuffer buffer = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN);
      return buffer.getLong();
    }

    byte[] readBytes(final int length) throws ConnectProtocolException {
      if (length < 0) {
        throw new ConnectProtocolException("Negative read length: " + length);
      }
      final int end = offset + length;
      if (end < offset || end > data.length) {
        throw new ConnectProtocolException(
            "Connect payload truncated (offset=" + offset + ", length=" + length + ", total=" + data.length + ")");
      }
      final byte[] out = new byte[length];
      System.arraycopy(data, offset, out, 0, length);
      offset = end;
      return out;
    }

    void ensureFullyConsumed(final String label) throws ConnectProtocolException {
      if (offset != data.length) {
        throw new ConnectProtocolException(
            label + " has trailing bytes (used=" + offset + ", total=" + data.length + ")");
      }
    }
  }
}

package org.hyperledger.iroha.android.offline;

import java.util.Arrays;
import java.util.List;
import java.util.Objects;

/** Bearer-named NFC APDU datastream facade for Offline Bearer v2 payloads. */
public final class OfflineBearerNfcApduProtocol {
  public static final byte[] AID = OfflineNoteNfcApduProtocol.AID.clone();
  public static final String AID_HEX = OfflineNoteNfcApduProtocol.AID_HEX;
  public static final int ANDROID_SAFE_CHUNK_BYTES =
      OfflineNoteNfcApduProtocol.ANDROID_SAFE_CHUNK_BYTES;
  public static final int MAX_EXTENDED_READ_CHUNK_BYTES =
      OfflineNoteNfcApduProtocol.MAX_EXTENDED_READ_CHUNK_BYTES;
  public static final int MAX_EXTENDED_WRITE_CHUNK_BYTES =
      OfflineNoteNfcApduProtocol.MAX_EXTENDED_WRITE_CHUNK_BYTES;
  public static final int MAX_INCOMING_PAYLOAD_BYTES =
      OfflineNoteNfcApduProtocol.MAX_INCOMING_PAYLOAD_BYTES;

  public static final byte[] STATUS_SUCCESS = OfflineNoteNfcApduProtocol.STATUS_SUCCESS.clone();
  public static final byte[] STATUS_WRONG_DATA = OfflineNoteNfcApduProtocol.STATUS_WRONG_DATA.clone();
  public static final byte[] STATUS_NOT_FOUND = OfflineNoteNfcApduProtocol.STATUS_NOT_FOUND.clone();
  public static final byte[] STATUS_CONDITIONS_NOT_SATISFIED =
      OfflineNoteNfcApduProtocol.STATUS_CONDITIONS_NOT_SATISFIED.clone();
  public static final byte[] STATUS_UNSUPPORTED = OfflineNoteNfcApduProtocol.STATUS_UNSUPPORTED.clone();

  private OfflineBearerNfcApduProtocol() {}

  public static byte[] selectAidApdu() {
    return OfflineNoteNfcApduProtocol.selectAidApdu();
  }

  public static byte[] getInfoApdu() {
    return OfflineNoteNfcApduProtocol.getInfoApdu();
  }

  public static byte[] readChunkApdu(final int offset) {
    return OfflineNoteNfcApduProtocol.readChunkApdu(offset);
  }

  public static byte[] readChunkApdu(final int offset, final int length) {
    return OfflineNoteNfcApduProtocol.readChunkApdu(offset, length);
  }

  public static byte[] writeMetaApdu(final PayloadKind kind, final byte[] payloadBytes) {
    return OfflineNoteNfcApduProtocol.writeMetaApdu(kind.toNoteKind(), payloadBytes);
  }

  public static byte[] writeChunkApdu(final int offset, final byte[] bytes) {
    return OfflineNoteNfcApduProtocol.writeChunkApdu(offset, bytes);
  }

  public static byte[] writeChunkApdu(
      final int offset, final byte[] bytes, final int startIndex, final int endIndex) {
    return OfflineNoteNfcApduProtocol.writeChunkApdu(offset, bytes, startIndex, endIndex);
  }

  public static byte[] commitApdu() {
    return OfflineNoteNfcApduProtocol.commitApdu();
  }

  public static List<byte[]> writePayloadApdus(final PayloadKind kind, final byte[] payloadBytes) {
    return OfflineNoteNfcApduProtocol.writePayloadApdus(kind.toNoteKind(), payloadBytes);
  }

  public static List<byte[]> writePayloadApdus(
      final PayloadKind kind, final byte[] payloadBytes, final int maxChunkLength) {
    return OfflineNoteNfcApduProtocol.writePayloadApdus(
        kind.toNoteKind(), payloadBytes, maxChunkLength);
  }

  public static List<byte[]> readPayloadApdus(final int payloadLength) {
    return OfflineNoteNfcApduProtocol.readPayloadApdus(payloadLength);
  }

  public static List<byte[]> readPayloadApdus(final int payloadLength, final int maxChunkLength) {
    return OfflineNoteNfcApduProtocol.readPayloadApdus(payloadLength, maxChunkLength);
  }

  public static Command parseCommand(final byte[] apdu) {
    return Command.fromNote(OfflineNoteNfcApduProtocol.parseCommand(apdu));
  }

  public static byte[] encodeInfo(final PayloadKind kind, final byte[] payloadBytes) {
    return OfflineNoteNfcApduProtocol.encodeInfo(kind.toNoteKind(), payloadBytes);
  }

  public static byte[] encodeInfo(
      final PayloadKind kind, final byte[] payloadBytes, final int maxChunkLength) {
    return OfflineNoteNfcApduProtocol.encodeInfo(kind.toNoteKind(), payloadBytes, maxChunkLength);
  }

  public static PayloadInfo decodeInfo(final byte[] data) {
    final OfflineNoteNfcApduProtocol.PayloadInfo info =
        OfflineNoteNfcApduProtocol.decodeInfo(data);
    return info == null ? null : PayloadInfo.fromNote(info);
  }

  public static byte[] response() {
    return OfflineNoteNfcApduProtocol.response();
  }

  public static byte[] response(final byte[] data) {
    return OfflineNoteNfcApduProtocol.response(data);
  }

  public static byte[] response(final byte[] data, final int offset, final int length) {
    return OfflineNoteNfcApduProtocol.response(data, offset, length);
  }

  public static int responseStatus(final byte[] response) {
    return OfflineNoteNfcApduProtocol.responseStatus(response);
  }

  public static byte[] responseData(final byte[] response) {
    return OfflineNoteNfcApduProtocol.responseData(response);
  }

  public static byte[] sha256(final byte[] bytes) {
    return OfflineNoteNfcApduProtocol.sha256(bytes);
  }

  public static boolean payloadDigestMatches(
      final byte[] payloadBytes, final byte[] expectedSha256) {
    return OfflineNoteNfcApduProtocol.payloadDigestMatches(payloadBytes, expectedSha256);
  }

  public static int requestedReadChunkLength(final byte[] apdu) {
    return OfflineNoteNfcApduProtocol.requestedReadChunkLength(apdu);
  }

  public static int iosFastWriteChunkLength(final boolean peerSupportsExtendedChunks) {
    return OfflineNoteNfcApduProtocol.iosFastWriteChunkLength(peerSupportsExtendedChunks);
  }

  /** NFC APDU payload kind for Offline Bearer v2 handoffs. */
  public enum PayloadKind {
    RECEIVE_REQUEST(OfflineNoteNfcApduProtocol.PayloadKind.RECEIVE_REQUEST),
    PAYMENT_TOKEN(OfflineNoteNfcApduProtocol.PayloadKind.PAYMENT_TOKEN),
    RECEIPT_ACK(OfflineNoteNfcApduProtocol.PayloadKind.RECEIPT_ACK);

    private final OfflineNoteNfcApduProtocol.PayloadKind noteKind;

    PayloadKind(final OfflineNoteNfcApduProtocol.PayloadKind noteKind) {
      this.noteKind = noteKind;
    }

    public int code() {
      return noteKind.code();
    }

    private OfflineNoteNfcApduProtocol.PayloadKind toNoteKind() {
      return noteKind;
    }

    private static PayloadKind fromNote(final OfflineNoteNfcApduProtocol.PayloadKind noteKind) {
      if (noteKind == null) {
        return null;
      }
      for (final PayloadKind kind : values()) {
        if (kind.noteKind == noteKind) {
          return kind;
        }
      }
      return null;
    }

    public static PayloadKind fromCode(final int code) {
      return fromNote(OfflineNoteNfcApduProtocol.PayloadKind.fromCode(code));
    }
  }

  /** Decoded NFC get-info metadata. */
  public static final class PayloadInfo {
    private final PayloadKind kind;
    private final int payloadLength;
    private final int maxChunkLength;
    private final byte[] sha256;

    public PayloadInfo(
        final PayloadKind kind,
        final int payloadLength,
        final int maxChunkLength,
        final byte[] sha256) {
      this.kind = Objects.requireNonNull(kind, "kind");
      this.payloadLength = payloadLength;
      this.maxChunkLength = maxChunkLength;
      this.sha256 = Objects.requireNonNull(sha256, "sha256").clone();
    }

    private static PayloadInfo fromNote(final OfflineNoteNfcApduProtocol.PayloadInfo info) {
      return new PayloadInfo(
          PayloadKind.fromNote(info.kind()),
          info.payloadLength(),
          info.maxChunkLength(),
          info.sha256());
    }

    public PayloadKind kind() {
      return kind;
    }

    public int payloadLength() {
      return payloadLength;
    }

    public int maxChunkLength() {
      return maxChunkLength;
    }

    public byte[] sha256() {
      return sha256.clone();
    }

    private OfflineNoteNfcApduProtocol.PayloadInfo toNoteInfo() {
      return new OfflineNoteNfcApduProtocol.PayloadInfo(
          kind.toNoteKind(), payloadLength, maxChunkLength, sha256);
    }

    @Override
    public boolean equals(final Object other) {
      if (!(other instanceof PayloadInfo that)) {
        return false;
      }
      return kind == that.kind
          && payloadLength == that.payloadLength
          && maxChunkLength == that.maxChunkLength
          && Arrays.equals(sha256, that.sha256);
    }

    @Override
    public int hashCode() {
      int result = kind.hashCode();
      result = 31 * result + payloadLength;
      result = 31 * result + maxChunkLength;
      result = 31 * result + Arrays.hashCode(sha256);
      return result;
    }
  }

  /** Parsed APDU command. */
  public static final class Command {
    private final Type type;
    private final int offset;
    private final int requestedLength;
    private final PayloadKind kind;
    private final int payloadLength;
    private final byte[] sha256;
    private final byte[] bytes;

    private Command(
        final Type type,
        final int offset,
        final int requestedLength,
        final PayloadKind kind,
        final int payloadLength,
        final byte[] sha256,
        final byte[] bytes) {
      this.type = Objects.requireNonNull(type, "type");
      this.offset = offset;
      this.requestedLength = requestedLength;
      this.kind = kind;
      this.payloadLength = payloadLength;
      this.sha256 = sha256 == null ? null : sha256.clone();
      this.bytes = bytes == null ? null : bytes.clone();
    }

    private static Command fromNote(final OfflineNoteNfcApduProtocol.Command command) {
      return new Command(
          Type.valueOf(command.type().name()),
          command.offset(),
          command.requestedLength(),
          PayloadKind.fromNote(command.kind()),
          command.payloadLength(),
          command.sha256(),
          command.bytes());
    }

    public Type type() {
      return type;
    }

    public int offset() {
      return offset;
    }

    public int requestedLength() {
      return requestedLength;
    }

    public PayloadKind kind() {
      return kind;
    }

    public int payloadLength() {
      return payloadLength;
    }

    public byte[] sha256() {
      return sha256 == null ? null : sha256.clone();
    }

    public byte[] bytes() {
      return bytes == null ? null : bytes.clone();
    }
  }

  /** APDU command type. */
  public enum Type {
    SELECT,
    GET_INFO,
    READ_CHUNK,
    WRITE_META,
    WRITE_CHUNK,
    COMMIT,
    UNSUPPORTED,
    INVALID
  }

  /** Incrementally validates APDU write chunks before exposing a completed NFC payload. */
  public static final class PayloadAssembler {
    private final OfflineNoteNfcApduProtocol.PayloadAssembler delegate;

    public PayloadAssembler(final PayloadInfo info) {
      this.delegate = new OfflineNoteNfcApduProtocol.PayloadAssembler(info.toNoteInfo());
    }

    public PayloadAssembler(
        final PayloadKind kind, final int expectedLength, final byte[] expectedSha256) {
      this.delegate =
          new OfflineNoteNfcApduProtocol.PayloadAssembler(
              kind.toNoteKind(), expectedLength, expectedSha256);
    }

    public PayloadKind kind() {
      return PayloadKind.fromNote(delegate.kind());
    }

    public int expectedLength() {
      return delegate.expectedLength();
    }

    public byte[] expectedSha256() {
      return delegate.expectedSha256();
    }

    public boolean isComplete() {
      return delegate.isComplete();
    }

    public boolean write(final int offset, final byte[] chunk) {
      return delegate.write(offset, chunk);
    }

    public byte[] commit() {
      return delegate.commit();
    }
  }
}

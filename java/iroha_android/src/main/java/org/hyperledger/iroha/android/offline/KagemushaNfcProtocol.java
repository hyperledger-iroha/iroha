package org.hyperledger.iroha.android.offline;

import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.TreeMap;

/** Platform-neutral NFC APDU datastream used by Android HCE/IsoDep and iOS CardSession peers. */
public final class KagemushaNfcProtocol {
  private static final byte[] CANONICAL_AID =
      new byte[] {(byte) 0xF0, 0x50, 0x4B, 0x45, 0x50, 0x4B, 0x52, 0x4E, 0x46, 0x43, 0x01};
  public static final String AID_HEX = IrohaPeerNfcV1.APPLICATION_IDENTIFIER_HEX;
  public static final int RAW_TRANSPORT_VERSION = 4;
  public static final int SAFE_CHUNK_BYTES = 220;
  public static final int MAX_EXTENDED_READ_CHUNK_BYTES = 1024;
  public static final int MAX_EXTENDED_WRITE_CHUNK_BYTES = 16 * 1024;
  public static final int MAXIMUM_PAYLOAD_BYTES = KagemushaPeerTransport.MAXIMUM_ARCHIVE_BYTES;
  private static final int SPARSE_FRAGMENT_ALLOWANCE = 64;
  private static final int MAXIMUM_SPARSE_FRAGMENT_COUNT =
      (MAXIMUM_PAYLOAD_BYTES + SAFE_CHUNK_BYTES - 1) / SAFE_CHUNK_BYTES
          + SPARSE_FRAGMENT_ALLOWANCE;

  private static final byte[] CANONICAL_STATUS_SUCCESS = new byte[] {(byte) 0x90, 0x00};
  private static final byte[] CANONICAL_STATUS_WRONG_DATA =
      new byte[] {(byte) 0x6A, (byte) 0x80};
  private static final byte[] CANONICAL_STATUS_NOT_FOUND =
      new byte[] {(byte) 0x6A, (byte) 0x82};
  private static final byte[] CANONICAL_STATUS_CONDITIONS_NOT_SATISFIED =
      new byte[] {(byte) 0x69, (byte) 0x85};
  private static final byte[] CANONICAL_STATUS_UNSUPPORTED =
      new byte[] {(byte) 0x6D, 0x00};

  private static final int CLA_IROHA = 0x80;
  private static final int INS_GET_INFO = 0x10;
  private static final int INS_READ_CHUNK = 0x11;
  private static final int INS_WRITE_META = 0x20;
  private static final int INS_WRITE_CHUNK = 0x21;
  private static final int INS_COMMIT = 0x22;
  private static final int OFFSET_BYTES = 4;
  private static final int READ_REQUEST_BYTES = OFFSET_BYTES + 2;

  private KagemushaNfcProtocol() {}

  public static byte[] defaultApplicationIdentifier() {
    return CANONICAL_AID.clone();
  }

  public static byte[] statusSuccess() {
    return CANONICAL_STATUS_SUCCESS.clone();
  }

  public static byte[] statusWrongData() {
    return CANONICAL_STATUS_WRONG_DATA.clone();
  }

  public static byte[] statusNotFound() {
    return CANONICAL_STATUS_NOT_FOUND.clone();
  }

  public static byte[] statusConditionsNotSatisfied() {
    return CANONICAL_STATUS_CONDITIONS_NOT_SATISFIED.clone();
  }

  public static byte[] statusUnsupported() {
    return CANONICAL_STATUS_UNSUPPORTED.clone();
  }

  public static byte[] selectAidApdu() {
    final byte[] apdu = new byte[5 + CANONICAL_AID.length + 1];
    apdu[0] = 0x00;
    apdu[1] = (byte) 0xA4;
    apdu[2] = 0x04;
    apdu[3] = 0x00;
    apdu[4] = (byte) CANONICAL_AID.length;
    System.arraycopy(CANONICAL_AID, 0, apdu, 5, CANONICAL_AID.length);
    apdu[apdu.length - 1] = 0x00;
    return apdu;
  }

  public static byte[] getInfoApdu() {
    return new byte[] {
      (byte) CLA_IROHA, (byte) INS_GET_INFO, (byte) RAW_TRANSPORT_VERSION, 0x00, 0x00
    };
  }

  public static byte[] readChunkApdu(final int offset) {
    return readChunkApdu(offset, SAFE_CHUNK_BYTES);
  }

  public static byte[] readChunkApdu(final int offset, final int length) {
    requireChunkLength(length, MAX_EXTENDED_READ_CHUNK_BYTES);
    requireTransferRange(offset, length, MAX_EXTENDED_READ_CHUNK_BYTES);
    final byte[] data = new byte[READ_REQUEST_BYTES];
    writeInt32(offset, data, 0);
    data[4] = (byte) ((length >>> 8) & 0xFF);
    data[5] = (byte) (length & 0xFF);
    return dataApdu(INS_READ_CHUNK, data);
  }

  public static byte[] writeMetaApdu(
      final PayloadKind kind, final byte[] payloadBytes) {
    Objects.requireNonNull(kind, "kind");
    Objects.requireNonNull(payloadBytes, "payloadBytes");
    requirePayloadLength(payloadBytes.length);
    final byte[] meta = new byte[38];
    meta[0] = (byte) RAW_TRANSPORT_VERSION;
    meta[1] = (byte) kind.code();
    writeInt32(payloadBytes.length, meta, 2);
    System.arraycopy(sha256(payloadBytes), 0, meta, 6, 32);
    return dataApdu(INS_WRITE_META, meta);
  }

  public static byte[] writeChunkApdu(final int offset, final byte[] bytes) {
    Objects.requireNonNull(bytes, "bytes");
    return writeChunkApdu(offset, bytes, 0, bytes.length);
  }

  public static byte[] writeChunkApdu(
      final int offset, final byte[] bytes, final int startIndex, final int endIndex) {
    Objects.requireNonNull(bytes, "bytes");
    if (startIndex < 0 || startIndex > bytes.length) {
      throw new IllegalArgumentException("startIndex out of bounds");
    }
    if (endIndex < startIndex || endIndex > bytes.length) {
      throw new IllegalArgumentException("endIndex out of bounds");
    }
    final int length = endIndex - startIndex;
    requireChunkLength(length, MAX_EXTENDED_WRITE_CHUNK_BYTES);
    requireTransferRange(offset, length, MAX_EXTENDED_WRITE_CHUNK_BYTES);
    final byte[] data = new byte[OFFSET_BYTES + length];
    writeInt32(offset, data, 0);
    System.arraycopy(bytes, startIndex, data, OFFSET_BYTES, length);
    return dataApdu(INS_WRITE_CHUNK, data);
  }

  public static byte[] commitApdu() {
    return new byte[] {
      (byte) CLA_IROHA, (byte) INS_COMMIT, (byte) RAW_TRANSPORT_VERSION, 0x00, 0x00
    };
  }

  /** Builds a canonical bulk write whose non-final chunks are at least 220 bytes. */
  public static List<byte[]> writePayloadApdus(
      final PayloadKind kind, final byte[] payloadBytes) {
    return writePayloadApdus(kind, payloadBytes, SAFE_CHUNK_BYTES);
  }

  /** Builds a canonical bulk write whose non-final chunks are at least 220 bytes. */
  public static List<byte[]> writePayloadApdus(
      final PayloadKind kind, final byte[] payloadBytes, final int maxChunkLength) {
    Objects.requireNonNull(payloadBytes, "payloadBytes");
    requirePayloadLength(payloadBytes.length);
    if (maxChunkLength < SAFE_CHUNK_BYTES) {
      throw new IllegalArgumentException(
          "bulk-write chunks must be at least " + SAFE_CHUNK_BYTES + " bytes");
    }
    requireChunkLength(maxChunkLength, MAX_EXTENDED_WRITE_CHUNK_BYTES);
    final List<byte[]> apdus = new ArrayList<>();
    apdus.add(writeMetaApdu(kind, payloadBytes));
    int offset = 0;
    while (offset < payloadBytes.length) {
      final int end = Math.min(offset + maxChunkLength, payloadBytes.length);
      apdus.add(writeChunkApdu(offset, payloadBytes, offset, end));
      offset = end;
    }
    apdus.add(commitApdu());
    return Collections.unmodifiableList(apdus);
  }

  public static List<byte[]> readPayloadApdus(final int payloadLength) {
    return readPayloadApdus(payloadLength, SAFE_CHUNK_BYTES);
  }

  public static List<byte[]> readPayloadApdus(final int payloadLength, final int maxChunkLength) {
    requirePayloadLength(payloadLength);
    requireChunkLength(maxChunkLength, MAX_EXTENDED_READ_CHUNK_BYTES);
    final List<byte[]> apdus = new ArrayList<>();
    int offset = 0;
    while (offset < payloadLength) {
      final int requested = Math.min(maxChunkLength, payloadLength - offset);
      apdus.add(readChunkApdu(offset, requested));
      offset += requested;
    }
    return Collections.unmodifiableList(apdus);
  }

  public static Command parseCommand(final byte[] apdu) {
    if (apdu == null || apdu.length < 4) {
      return Command.invalid();
    }
    if (isSelectAid(apdu)) {
      return Command.select();
    }
    if (isAnySelectAid(apdu)) {
      return Command.selectOtherApplication();
    }
    if ((apdu[0] & 0xFF) != CLA_IROHA) {
      return Command.unsupported();
    }
    final int ins = apdu[1] & 0xFF;
    final boolean canonicalParameters =
        (apdu[2] & 0xFF) == RAW_TRANSPORT_VERSION && apdu[3] == 0;
    return switch (ins) {
      case INS_GET_INFO ->
          canonicalParameters && isNoDataApdu(apdu) ? Command.getInfo() : Command.invalid();
      case INS_READ_CHUNK -> {
        final byte[] data = canonicalParameters ? commandData(apdu) : null;
        if (data == null || data.length != READ_REQUEST_BYTES) {
          yield Command.invalid();
        }
        final int offset = readInt32(data, 0);
        final int length = readUInt16(data, OFFSET_BYTES);
        yield transferRangeIsValid(offset, length, MAX_EXTENDED_READ_CHUNK_BYTES)
            ? Command.readChunk(offset, length)
            : Command.invalid();
      }
      case INS_WRITE_META -> {
        if (!canonicalParameters) {
          yield Command.invalid();
        }
        final byte[] data = commandData(apdu);
        yield data == null ? Command.invalid() : parseWriteMeta(data);
      }
      case INS_WRITE_CHUNK -> {
        final byte[] data = canonicalParameters ? commandData(apdu) : null;
        if (data == null
            || data.length <= OFFSET_BYTES
            || data.length > OFFSET_BYTES + MAX_EXTENDED_WRITE_CHUNK_BYTES) {
          yield Command.invalid();
        }
        final int offset = readInt32(data, 0);
        final byte[] chunk = Arrays.copyOfRange(data, OFFSET_BYTES, data.length);
        yield transferRangeIsValid(offset, chunk.length, MAX_EXTENDED_WRITE_CHUNK_BYTES)
            ? Command.writeChunk(offset, chunk)
            : Command.invalid();
      }
      case INS_COMMIT ->
          canonicalParameters && isNoDataApdu(apdu) ? Command.commit() : Command.invalid();
      default -> Command.unsupported();
    };
  }

  public static byte[] encodeInfo(final PayloadKind kind, final byte[] payloadBytes) {
    return encodeInfo(kind, payloadBytes, SAFE_CHUNK_BYTES);
  }

  public static byte[] encodeInfo(
      final PayloadKind kind, final byte[] payloadBytes, final int maxChunkLength) {
    Objects.requireNonNull(kind, "kind");
    Objects.requireNonNull(payloadBytes, "payloadBytes");
    requirePayloadLength(payloadBytes.length);
    requireChunkLength(maxChunkLength, MAX_EXTENDED_READ_CHUNK_BYTES);
    final byte[] info = new byte[40];
    info[0] = (byte) RAW_TRANSPORT_VERSION;
    info[1] = (byte) kind.code();
    writeInt32(payloadBytes.length, info, 2);
    info[6] = (byte) ((maxChunkLength >>> 8) & 0xFF);
    info[7] = (byte) (maxChunkLength & 0xFF);
    System.arraycopy(sha256(payloadBytes), 0, info, 8, 32);
    return info;
  }

  public static PayloadInfo decodeInfo(final byte[] data) {
    Objects.requireNonNull(data, "data");
    if (data.length != 40 || (data[0] & 0xFF) != RAW_TRANSPORT_VERSION) {
      return null;
    }
    final PayloadKind kind = PayloadKind.fromCode(data[1] & 0xFF);
    if (kind == null) {
      return null;
    }
    final int payloadLength = readInt32(data, 2);
    final int maxChunkLength = readUInt16(data, 6);
    if (payloadLength <= 0
        || payloadLength > MAXIMUM_PAYLOAD_BYTES
        || maxChunkLength <= 0
        || maxChunkLength > MAX_EXTENDED_READ_CHUNK_BYTES
        || !containsNonzero(data, 8, 40)) {
      return null;
    }
    return new PayloadInfo(
        RAW_TRANSPORT_VERSION,
        kind,
        payloadLength,
        maxChunkLength,
        Arrays.copyOfRange(data, 8, 40));
  }

  public static byte[] response() {
    return response(new byte[0]);
  }

  public static byte[] response(final byte[] data) {
    Objects.requireNonNull(data, "data");
    return response(data, 0, data.length);
  }

  public static byte[] response(final byte[] data, final int offset, final int length) {
    Objects.requireNonNull(data, "data");
    if (offset < 0 || offset > data.length) {
      throw new IllegalArgumentException("offset out of bounds");
    }
    if (length < 0
        || length > data.length - offset
        || length > Integer.MAX_VALUE - CANONICAL_STATUS_SUCCESS.length) {
      throw new IllegalArgumentException("length out of bounds");
    }
    final byte[] response = new byte[length + CANONICAL_STATUS_SUCCESS.length];
    System.arraycopy(data, offset, response, 0, length);
    System.arraycopy(
        CANONICAL_STATUS_SUCCESS, 0, response, length, CANONICAL_STATUS_SUCCESS.length);
    return response;
  }

  public static int responseStatus(final byte[] response) {
    Objects.requireNonNull(response, "response");
    if (response.length < 2) {
      return -1;
    }
    return ((response[response.length - 2] & 0xFF) << 8) | (response[response.length - 1] & 0xFF);
  }

  public static byte[] responseData(final byte[] response) {
    Objects.requireNonNull(response, "response");
    if (response.length < 2) {
      return new byte[0];
    }
    return Arrays.copyOf(response, response.length - 2);
  }

  public static byte[] sha256(final byte[] bytes) {
    Objects.requireNonNull(bytes, "bytes");
    try {
      return MessageDigest.getInstance("SHA-256").digest(bytes);
    } catch (NoSuchAlgorithmException ex) {
      throw new IllegalStateException("SHA-256 is unavailable", ex);
    }
  }

  public static boolean payloadDigestMatches(
      final byte[] payloadBytes, final byte[] expectedSha256) {
    return Arrays.equals(sha256(payloadBytes), Objects.requireNonNull(expectedSha256, "expectedSha256"));
  }

  public static int requestedReadChunkLength(final byte[] apdu) {
    Objects.requireNonNull(apdu, "apdu");
    if (apdu.length < 5
        || (apdu[0] & 0xFF) != CLA_IROHA
        || (apdu[1] & 0xFF) != INS_READ_CHUNK
        || (apdu[2] & 0xFF) != RAW_TRANSPORT_VERSION
        || apdu[3] != 0) {
      return SAFE_CHUNK_BYTES;
    }
    final byte[] data = commandData(apdu);
    if (data == null || data.length != READ_REQUEST_BYTES) return SAFE_CHUNK_BYTES;
    final int offset = readInt32(data, 0);
    final int length = readUInt16(data, OFFSET_BYTES);
    return transferRangeIsValid(offset, length, MAX_EXTENDED_READ_CHUNK_BYTES)
        ? length
        : SAFE_CHUNK_BYTES;
  }

  public static int iosFastWriteChunkLength(final boolean peerSupportsExtendedChunks) {
    return peerSupportsExtendedChunks ? MAX_EXTENDED_WRITE_CHUNK_BYTES : SAFE_CHUNK_BYTES;
  }

  private static boolean isSelectAid(final byte[] apdu) {
    if (!isAnySelectAid(apdu)) return false;
    final int length = apdu[4] & 0xFF;
    final int payloadEnd = 5 + length;
    return Arrays.equals(Arrays.copyOfRange(apdu, 5, payloadEnd), CANONICAL_AID);
  }

  private static boolean isAnySelectAid(final byte[] apdu) {
    if (apdu.length < 5
        || (apdu[0] & 0xFF) != 0x00
        || (apdu[1] & 0xFF) != 0xA4
        || (apdu[2] & 0xFF) != 0x04
        || (apdu[3] & 0xFF) != 0x00) {
      return false;
    }
    final int length = apdu[4] & 0xFF;
    final int payloadEnd = 5 + length;
    return length > 0
        && (apdu.length == payloadEnd
            || (apdu.length == payloadEnd + 1 && (apdu[payloadEnd] & 0xFF) == 0x00));
  }

  private static byte[] commandData(final byte[] apdu) {
    if (apdu.length == 4) {
      return new byte[0];
    }
    if (apdu.length < 5) {
      return null;
    }
    final int length = apdu[4] & 0xFF;
    if (length == 0) {
      if (apdu.length == 5) {
        return new byte[0];
      }
      if (apdu.length < 7) {
        return null;
      }
      final int extendedLength = ((apdu[5] & 0xFF) << 8) | (apdu[6] & 0xFF);
      if (extendedLength <= 0 || apdu.length != 7 + extendedLength) {
        return null;
      }
      return Arrays.copyOfRange(apdu, 7, 7 + extendedLength);
    }
    if (apdu.length != 5 + length) {
      return null;
    }
    return Arrays.copyOfRange(apdu, 5, 5 + length);
  }

  private static boolean isNoDataApdu(final byte[] apdu) {
    return apdu.length == 4 || (apdu.length == 5 && (apdu[4] & 0xFF) == 0);
  }

  private static Command parseWriteMeta(final byte[] data) {
    if (data.length != 38 || (data[0] & 0xFF) != RAW_TRANSPORT_VERSION) {
      return Command.invalid();
    }
    final PayloadKind kind = PayloadKind.fromCode(data[1] & 0xFF);
    if (kind == null) {
      return Command.invalid();
    }
    final int payloadLength = readInt32(data, 2);
    if (payloadLength <= 0
        || payloadLength > MAXIMUM_PAYLOAD_BYTES
        || !containsNonzero(data, 6, 38)) {
      return Command.invalid();
    }
    return Command.writeMeta(kind, payloadLength, Arrays.copyOfRange(data, 6, 38));
  }

  private static byte[] dataApdu(final int instruction, final byte[] data) {
    if (data.length == 0 || data.length > 0xFFFF) {
      throw new IllegalArgumentException("APDU data length out of bounds");
    }
    final int headerLength = data.length <= 0xFF ? 5 : 7;
    final byte[] apdu = new byte[headerLength + data.length];
    apdu[0] = (byte) CLA_IROHA;
    apdu[1] = (byte) instruction;
    apdu[2] = (byte) RAW_TRANSPORT_VERSION;
    apdu[3] = 0;
    if (data.length <= 0xFF) {
      apdu[4] = (byte) data.length;
    } else {
      apdu[4] = 0x00;
      apdu[5] = (byte) ((data.length >>> 8) & 0xFF);
      apdu[6] = (byte) (data.length & 0xFF);
    }
    System.arraycopy(data, 0, apdu, headerLength, data.length);
    return apdu;
  }

  private static boolean transferRangeIsValid(
      final int offset, final int length, final int maximumChunkLength) {
    if (offset < 0 || length <= 0 || length > maximumChunkLength) return false;
    final long end = (long) offset + (long) length;
    return offset < MAXIMUM_PAYLOAD_BYTES && end <= MAXIMUM_PAYLOAD_BYTES;
  }

  private static void requireTransferRange(
      final int offset, final int length, final int maximumChunkLength) {
    if (!transferRangeIsValid(offset, length, maximumChunkLength)) {
      throw new IllegalArgumentException("transfer range out of bounds");
    }
  }

  private static void requirePayloadLength(final int length) {
    if (length <= 0 || length > MAXIMUM_PAYLOAD_BYTES) {
      throw new IllegalArgumentException("payload length out of bounds");
    }
  }

  private static void requireChunkLength(final int length, final int maxChunkLength) {
    if (length <= 0 || length > maxChunkLength) {
      throw new IllegalArgumentException("chunk length out of bounds");
    }
  }

  private static int sparseFragmentBudget(final int payloadLength) {
    final int canonicalFragments =
        (payloadLength + SAFE_CHUNK_BYTES - 1) / SAFE_CHUNK_BYTES;
    return Math.min(
        canonicalFragments + SPARSE_FRAGMENT_ALLOWANCE, MAXIMUM_SPARSE_FRAGMENT_COUNT);
  }

  private static void writeInt32(final int value, final byte[] out, final int offset) {
    out[offset] = (byte) ((value >>> 24) & 0xFF);
    out[offset + 1] = (byte) ((value >>> 16) & 0xFF);
    out[offset + 2] = (byte) ((value >>> 8) & 0xFF);
    out[offset + 3] = (byte) (value & 0xFF);
  }

  private static int readInt32(final byte[] bytes, final int offset) {
    return ((bytes[offset] & 0xFF) << 24)
        | ((bytes[offset + 1] & 0xFF) << 16)
        | ((bytes[offset + 2] & 0xFF) << 8)
        | (bytes[offset + 3] & 0xFF);
  }

  private static int readUInt16(final byte[] bytes, final int offset) {
    return ((bytes[offset] & 0xFF) << 8) | (bytes[offset + 1] & 0xFF);
  }

  private static boolean containsNonzero(
      final byte[] bytes, final int start, final int endExclusive) {
    for (int index = start; index < endExclusive; index++) {
      if (bytes[index] != 0) return true;
    }
    return false;
  }

  /** NFC APDU payload kind. */
  public enum PayloadKind {
    RECEIVE_REQUEST(1),
    PAYMENT(2),
    ACKNOWLEDGEMENT(3);

    private final int code;

    PayloadKind(final int code) {
      this.code = code;
    }

    public int code() {
      return code;
    }

    public static PayloadKind fromCode(final int code) {
      for (final PayloadKind kind : values()) {
        if (kind.code == code) {
          return kind;
        }
      }
      return null;
    }
  }

  /** Decoded NFC get-info metadata. */
  public static final class PayloadInfo {
    private final int transportVersion;
    private final PayloadKind kind;
    private final int payloadLength;
    private final int maxChunkLength;
    private final byte[] sha256;

    public PayloadInfo(
        final int transportVersion,
        final PayloadKind kind,
        final int payloadLength,
        final int maxChunkLength,
        final byte[] sha256) {
      this.transportVersion = transportVersion;
      this.kind = Objects.requireNonNull(kind, "kind");
      this.payloadLength = payloadLength;
      this.maxChunkLength = maxChunkLength;
      this.sha256 = Objects.requireNonNull(sha256, "sha256").clone();
    }

    public int transportVersion() {
      return transportVersion;
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

    public static Command select() {
      return new Command(Type.SELECT, 0, 0, null, 0, null, null);
    }

    public static Command selectOtherApplication() {
      return new Command(Type.SELECT_OTHER_APPLICATION, 0, 0, null, 0, null, null);
    }

    public static Command getInfo() {
      return new Command(Type.GET_INFO, 0, 0, null, 0, null, null);
    }

    public static Command readChunk(final int offset, final int requestedLength) {
      return new Command(Type.READ_CHUNK, offset, requestedLength, null, 0, null, null);
    }

    public static Command writeMeta(
        final PayloadKind kind, final int payloadLength, final byte[] sha256) {
      return new Command(Type.WRITE_META, 0, 0, kind, payloadLength, sha256, null);
    }

    public static Command writeChunk(final int offset, final byte[] bytes) {
      return new Command(Type.WRITE_CHUNK, offset, 0, null, 0, null, bytes);
    }

    public static Command commit() {
      return new Command(Type.COMMIT, 0, 0, null, 0, null, null);
    }

    public static Command unsupported() {
      return new Command(Type.UNSUPPORTED, 0, 0, null, 0, null, null);
    }

    public static Command invalid() {
      return new Command(Type.INVALID, 0, 0, null, 0, null, null);
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

    @Override
    public boolean equals(final Object other) {
      if (!(other instanceof Command that)) {
        return false;
      }
      return type == that.type
          && offset == that.offset
          && requestedLength == that.requestedLength
          && kind == that.kind
          && payloadLength == that.payloadLength
          && Arrays.equals(sha256, that.sha256)
          && Arrays.equals(bytes, that.bytes);
    }

    @Override
    public int hashCode() {
      int result = type.hashCode();
      result = 31 * result + offset;
      result = 31 * result + requestedLength;
      result = 31 * result + Objects.hashCode(kind);
      result = 31 * result + payloadLength;
      result = 31 * result + Arrays.hashCode(sha256);
      result = 31 * result + Arrays.hashCode(bytes);
      return result;
    }
  }

  /** APDU command type. */
  public enum Type {
    SELECT,
    SELECT_OTHER_APPLICATION,
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
    private static final class StoredFragment {
      private final int offset;
      private final byte[] bytes;

      private StoredFragment(final int offset, final byte[] bytes) {
        this.offset = offset;
        this.bytes = bytes;
      }

      private int end() {
        return offset + bytes.length;
      }
    }

    private final PayloadKind kind;
    private final int expectedLength;
    private final byte[] expectedSha256;
    private final TreeMap<Integer, byte[]> fragments = new TreeMap<>();
    private final TreeMap<Integer, Integer> coveredRanges = new TreeMap<>();
    // The canonical 220-byte writer needs ceil(length / 220) fragments. A fixed
    // allowance admits modest overlap/out-of-order splitting without allowing
    // attacker-selected one-byte writes to create unbounded nodes.
    private final int fragmentBudget;
    private int bufferedByteCount;
    private boolean cleared;

    public PayloadAssembler(final PayloadInfo info) {
      this(info.kind(), info.payloadLength(), info.sha256());
    }

    public PayloadAssembler(
        final PayloadKind kind, final int expectedLength, final byte[] expectedSha256) {
      this.kind = Objects.requireNonNull(kind, "kind");
      if (expectedLength <= 0 || expectedLength > MAXIMUM_PAYLOAD_BYTES) {
        throw new IllegalArgumentException("payload length out of bounds");
      }
      if (Objects.requireNonNull(expectedSha256, "expectedSha256").length != 32) {
        throw new IllegalArgumentException("sha256 must be 32 bytes");
      }
      if (!containsNonzero(expectedSha256, 0, expectedSha256.length)) {
        throw new IllegalArgumentException("sha256 must be non-zero");
      }
      this.expectedLength = expectedLength;
      this.expectedSha256 = expectedSha256.clone();
      this.fragmentBudget = sparseFragmentBudget(expectedLength);
    }

    public PayloadKind kind() {
      return kind;
    }

    public int expectedLength() {
      return expectedLength;
    }

    public synchronized byte[] expectedSha256() {
      return expectedSha256.clone();
    }

    /** Returns the unique payload bytes currently retained by the sparse assembler. */
    public synchronized int bufferedByteCount() {
      return bufferedByteCount;
    }

    public synchronized boolean isComplete() {
      return !cleared
          && bufferedByteCount == expectedLength
          && coveredRanges.size() == 1
          && coveredRanges.firstKey() == 0
          && coveredRanges.firstEntry().getValue() == expectedLength;
    }

    public synchronized boolean write(final int offset, final byte[] chunk) {
      Objects.requireNonNull(chunk, "chunk");
      if (cleared
          || offset < 0
          || offset > expectedLength
          || chunk.length == 0
          || chunk.length > MAX_EXTENDED_WRITE_CHUNK_BYTES) {
        return false;
      }
      if (chunk.length > expectedLength - offset) {
        return false;
      }
      final int end = offset + chunk.length;
      final List<StoredFragment> overlaps = overlappingFragments(offset, end);
      for (final StoredFragment fragment : overlaps) {
        final int overlapStart = Math.max(offset, fragment.offset);
        final int overlapEnd = Math.min(end, fragment.end());
        for (int target = overlapStart; target < overlapEnd; target++) {
          if (fragment.bytes[target - fragment.offset] != chunk[target - offset]) {
            return false;
          }
        }
      }

      int proposedFragmentCount = 0;
      int budgetCursor = offset;
      for (final StoredFragment fragment : overlaps) {
        if (budgetCursor < fragment.offset) {
          proposedFragmentCount += 1;
        }
        budgetCursor = Math.max(budgetCursor, Math.min(fragment.end(), end));
      }
      if (budgetCursor < end) {
        proposedFragmentCount += 1;
      }
      if (fragments.size() + proposedFragmentCount > fragmentBudget) {
        clear();
        return false;
      }

      // Materialize only uncovered ranges before mutating state. Coalesced
      // interval metadata avoids per-byte flags, while immutable fragments
      // avoid quadratic copying as adjacent writes grow the covered range.
      final List<StoredFragment> additions = new ArrayList<>();
      int cursor = offset;
      for (final StoredFragment fragment : overlaps) {
        if (cursor < fragment.offset) {
          final int gapEnd = Math.min(fragment.offset, end);
          additions.add(
              new StoredFragment(
                  cursor,
                  Arrays.copyOfRange(chunk, cursor - offset, gapEnd - offset)));
        }
        cursor = Math.max(cursor, Math.min(fragment.end(), end));
      }
      if (cursor < end) {
        additions.add(
            new StoredFragment(cursor, Arrays.copyOfRange(chunk, cursor - offset, end - offset)));
      }
      if (additions.isEmpty()) {
        return true;
      }
      for (final StoredFragment addition : additions) {
        fragments.put(addition.offset, addition.bytes);
        bufferedByteCount += addition.bytes.length;
      }
      mergeCoverage(offset, end);
      return true;
    }

    public synchronized byte[] commit() {
      if (cleared) {
        throw new IllegalStateException("payload assembler is cleared");
      }
      if (!isComplete()) {
        throw new IllegalStateException("payload is incomplete");
      }
      byte[] assembled = null;
      boolean succeeded = false;
      try {
        assembled = new byte[expectedLength];
        int cursor = 0;
        for (final Map.Entry<Integer, byte[]> fragment : fragments.entrySet()) {
          if (fragment.getKey() != cursor
              || fragment.getValue().length > expectedLength - cursor) {
            throw new IllegalStateException("payload reconstruction mismatch");
          }
          System.arraycopy(fragment.getValue(), 0, assembled, cursor, fragment.getValue().length);
          cursor += fragment.getValue().length;
        }
        if (cursor != expectedLength) {
          throw new IllegalStateException("payload reconstruction mismatch");
        }
        if (!payloadDigestMatches(assembled, expectedSha256)) {
          throw new IllegalStateException("payload checksum mismatch");
        }
        succeeded = true;
        return assembled;
      } finally {
        if (!succeeded && assembled != null) {
          Arrays.fill(assembled, (byte) 0);
        }
        // Exact coverage makes commit a one-shot operation. Both success and
        // terminal validation failure consume and zeroize the state.
        clear();
      }
    }

    /** Zeroizes all owned payload and digest bytes and makes this assembler unusable. */
    public synchronized void clear() {
      Arrays.fill(expectedSha256, (byte) 0);
      for (final byte[] fragment : fragments.values()) {
        Arrays.fill(fragment, (byte) 0);
      }
      fragments.clear();
      coveredRanges.clear();
      bufferedByteCount = 0;
      cleared = true;
    }

    private List<StoredFragment> overlappingFragments(final int start, final int end) {
      final List<StoredFragment> overlapping = new ArrayList<>();
      Map.Entry<Integer, byte[]> entry = fragments.floorEntry(start);
      if (entry == null || entry.getKey() + entry.getValue().length <= start) {
        entry = fragments.ceilingEntry(start);
      }
      while (entry != null && entry.getKey() < end) {
        overlapping.add(new StoredFragment(entry.getKey(), entry.getValue()));
        entry = fragments.higherEntry(entry.getKey());
      }
      return overlapping;
    }

    private void mergeCoverage(final int start, final int end) {
      int mergedStart = start;
      int mergedEnd = end;
      final Map.Entry<Integer, Integer> floor = coveredRanges.floorEntry(mergedStart);
      if (floor != null && floor.getValue() >= mergedStart) {
        mergedStart = floor.getKey();
        mergedEnd = Math.max(mergedEnd, floor.getValue());
        coveredRanges.remove(floor.getKey());
      }
      Map.Entry<Integer, Integer> next = coveredRanges.ceilingEntry(mergedStart);
      while (next != null && next.getKey() <= mergedEnd) {
        mergedEnd = Math.max(mergedEnd, next.getValue());
        coveredRanges.remove(next.getKey());
        next = coveredRanges.ceilingEntry(mergedStart);
      }
      coveredRanges.put(mergedStart, mergedEnd);
    }
  }
}

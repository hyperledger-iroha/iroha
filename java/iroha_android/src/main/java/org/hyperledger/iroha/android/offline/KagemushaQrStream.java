package org.hyperledger.iroha.android.offline;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.LinkedHashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.zip.CRC32;

/**
 * Canonical fixed-XOR Kagemusha QR stream shared with the Swift and Kotlin SDKs.
 *
 * <p>The decoder bounds every allocation before decoding, accepts frames in any order, recovers at
 * most one missing data frame per fixed parity group, and rolls back its complete state when a frame
 * is invalid.
 */
public final class KagemushaQrStream {
  public static final int MINIMUM_CHUNK_SIZE = 64;
  public static final int MAXIMUM_CHUNK_SIZE = 512;
  public static final int MINIMUM_PARITY_GROUP = 2;
  public static final int MAXIMUM_PARITY_GROUP = 16;
  public static final int STANDARD_CHUNK_SIZE = 256;
  public static final int STANDARD_PARITY_GROUP = 4;

  private static final int ENVELOPE_VERSION = 1;
  private static final int ENVELOPE_BYTES = 46;
  private static final int FRAME_VERSION = 1;
  private static final int FRAME_FIXED_OVERHEAD = 30;
  private static final int MAXIMUM_FRAME_BYTES = FRAME_FIXED_OVERHEAD + MAXIMUM_CHUNK_SIZE;
  public static final int MAXIMUM_FRAME_TEXT_BYTES =
      KagemushaPeerTransport.QR_STREAM_TEXT_PREFIX.length()
          + (MAXIMUM_FRAME_BYTES * 4 + 2) / 3;
  private static final int MAXIMUM_DATA_FRAMES =
      (KagemushaPeerTransport.MAXIMUM_ARCHIVE_BYTES + MINIMUM_CHUNK_SIZE - 1)
          / MINIMUM_CHUNK_SIZE;
  private static final int MAXIMUM_PARITY_FRAMES =
      (MAXIMUM_DATA_FRAMES + MINIMUM_PARITY_GROUP - 1) / MINIMUM_PARITY_GROUP;

  private KagemushaQrStream() {}

  public static List<String> encode(final KagemushaPeerTransport.Payload payload) {
    return encode(payload, Options.STANDARD);
  }

  public static List<String> encode(
      final KagemushaPeerTransport.Payload payload, final Options options) {
    Objects.requireNonNull(payload, "payload");
    Objects.requireNonNull(options, "options");
    final byte[] archive = payload.archive();
    try {
      require(
          archive.length > 0 && archive.length <= KagemushaPeerTransport.MAXIMUM_ARCHIVE_BYTES,
          "Kagemusha QR payload exceeds its bound");
      final Envelope envelope = Envelope.create(payload.kind(), archive, options);
      final byte[] streamId = envelope.streamId();
      final List<Frame> frames = new ArrayList<>();
      frames.add(new Frame(FrameKind.HEADER, streamId, 0, 1, envelope.encode()));

      final List<byte[]> chunks = chunks(archive, options.chunkSize());
      for (int index = 0; index < chunks.size(); index++) {
        frames.add(new Frame(FrameKind.DATA, streamId, index, chunks.size(), chunks.get(index)));
      }
      for (int group = 0; group < envelope.parityChunks; group++) {
        final byte[] parity = new byte[options.chunkSize()];
        final int start = group * options.parityGroup();
        final int end = Math.min(start + options.parityGroup(), chunks.size());
        for (int chunkIndex = start; chunkIndex < end; chunkIndex++) {
          final byte[] chunk = chunks.get(chunkIndex);
          for (int byteIndex = 0; byteIndex < chunk.length; byteIndex++) {
            parity[byteIndex] ^= chunk[byteIndex];
          }
        }
        frames.add(
            new Frame(FrameKind.PARITY, streamId, group, envelope.parityChunks, parity));
      }

      final List<String> encoded = new ArrayList<>(frames.size());
      for (final Frame frame : frames) {
        final byte[] bytes = frame.encode();
        try {
          encoded.add(
              KagemushaPeerTransport.QR_STREAM_TEXT_PREFIX
                  + KagemushaPeerTransport.base64UrlEncode(bytes));
        } finally {
          Arrays.fill(bytes, (byte) 0);
        }
      }
      return Collections.unmodifiableList(encoded);
    } finally {
      Arrays.fill(archive, (byte) 0);
    }
  }

  public static Frame decodeFrameText(final String value) {
    Objects.requireNonNull(value, "value");
    require(
        value.getBytes(StandardCharsets.UTF_8).length <= MAXIMUM_FRAME_TEXT_BYTES
            && value.startsWith(KagemushaPeerTransport.QR_STREAM_TEXT_PREFIX),
        "Kagemusha QR frame is not canonical");
    final String body = value.substring(KagemushaPeerTransport.QR_STREAM_TEXT_PREFIX.length());
    final byte[] bytes = KagemushaPeerTransport.base64UrlDecode(body);
    try {
      require(
          (KagemushaPeerTransport.QR_STREAM_TEXT_PREFIX
                  + KagemushaPeerTransport.base64UrlEncode(bytes))
              .equals(value),
          "Kagemusha QR frame is not canonical");
      return Frame.decode(bytes);
    } finally {
      Arrays.fill(bytes, (byte) 0);
    }
  }

  /** Validated bounded QR options. */
  public static final class Options {
    public static final Options STANDARD =
        new Options(STANDARD_CHUNK_SIZE, STANDARD_PARITY_GROUP);

    private final int chunkSize;
    private final int parityGroup;

    public Options(final int chunkSize, final int parityGroup) {
      require(
          chunkSize >= MINIMUM_CHUNK_SIZE && chunkSize <= MAXIMUM_CHUNK_SIZE,
          "Kagemusha QR chunk size is unsupported");
      require(
          parityGroup >= MINIMUM_PARITY_GROUP && parityGroup <= MAXIMUM_PARITY_GROUP,
          "Kagemusha QR parity group is unsupported");
      this.chunkSize = chunkSize;
      this.parityGroup = parityGroup;
    }

    public int chunkSize() {
      return chunkSize;
    }

    public int parityGroup() {
      return parityGroup;
    }

    @Override
    public boolean equals(final Object other) {
      return other instanceof Options that
          && chunkSize == that.chunkSize
          && parityGroup == that.parityGroup;
    }

    @Override
    public int hashCode() {
      return 31 * chunkSize + parityGroup;
    }
  }

  public enum FrameKind {
    HEADER(0),
    DATA(1),
    PARITY(2);

    private final int code;

    FrameKind(final int code) {
      this.code = code;
    }

    public int code() {
      return code;
    }

    private static FrameKind fromCode(final int code) {
      for (final FrameKind value : values()) if (value.code == code) return value;
      return null;
    }
  }

  /** One immutable binary Kagemusha QR frame. */
  public static final class Frame {
    private final FrameKind kind;
    private final byte[] streamId;
    private final int index;
    private final int total;
    private final byte[] payload;

    public Frame(
        final FrameKind kind,
        final byte[] streamId,
        final int index,
        final int total,
        final byte[] payload) {
      this.kind = Objects.requireNonNull(kind, "kind");
      this.streamId = Objects.requireNonNull(streamId, "streamId").clone();
      this.payload = Objects.requireNonNull(payload, "payload").clone();
      require(
          this.streamId.length == 16 && anyNonzero(this.streamId),
          "Malformed Kagemusha QR stream identifier");
      require(total >= 1 && total <= 0xffff && index >= 0 && index < total,
          "Malformed Kagemusha QR frame index");
      require(this.payload.length > 0 && this.payload.length <= MAXIMUM_CHUNK_SIZE,
          "Malformed Kagemusha QR frame payload");
      if (kind == FrameKind.HEADER) {
        require(index == 0 && total == 1 && this.payload.length == ENVELOPE_BYTES,
            "Malformed Kagemusha QR header frame");
      }
      this.index = index;
      this.total = total;
    }

    public FrameKind kind() {
      return kind;
    }

    public byte[] streamId() {
      return streamId.clone();
    }

    public int index() {
      return index;
    }

    public int total() {
      return total;
    }

    public byte[] payload() {
      return payload.clone();
    }

    public byte[] encode() {
      final int payloadEnd = 26 + payload.length;
      final byte[] out = new byte[payloadEnd + 4];
      out[0] = 0x4b;
      out[1] = 0x51;
      out[2] = FRAME_VERSION;
      out[3] = (byte) kind.code;
      System.arraycopy(streamId, 0, out, 4, streamId.length);
      writeU16(out, 20, index);
      writeU16(out, 22, total);
      writeU16(out, 24, payload.length);
      System.arraycopy(payload, 0, out, 26, payload.length);
      writeU32(out, payloadEnd, crc32(out, 2, payloadEnd));
      return out;
    }

    public static Frame decode(final byte[] data) {
      Objects.requireNonNull(data, "data");
      require(
          data.length >= FRAME_FIXED_OVERHEAD
              && data.length <= MAXIMUM_FRAME_BYTES
              && data[0] == 0x4b
              && data[1] == 0x51
              && (data[2] & 0xff) == FRAME_VERSION,
          "Malformed Kagemusha QR frame");
      final FrameKind kind = FrameKind.fromCode(data[3] & 0xff);
      require(kind != null, "Malformed Kagemusha QR frame kind");
      final int payloadLength = readU16(data, 24);
      final int payloadEnd = 26 + payloadLength;
      require(payloadEnd + 4 == data.length, "Malformed Kagemusha QR frame length");
      require(readU32(data, payloadEnd) == crc32(data, 2, payloadEnd),
          "Kagemusha QR frame checksum mismatch");
      return new Frame(
          kind,
          Arrays.copyOfRange(data, 4, 20),
          readU16(data, 20),
          readU16(data, 22),
          Arrays.copyOfRange(data, 26, payloadEnd));
    }

    @Override
    public boolean equals(final Object other) {
      return other instanceof Frame that
          && kind == that.kind
          && index == that.index
          && total == that.total
          && Arrays.equals(streamId, that.streamId)
          && Arrays.equals(payload, that.payload);
    }

    @Override
    public int hashCode() {
      int result = kind.hashCode();
      result = 31 * result + Arrays.hashCode(streamId);
      result = 31 * result + index;
      result = 31 * result + total;
      return 31 * result + Arrays.hashCode(payload);
    }
  }

  /** Immutable decoder progress/result. */
  public static final class DecodeResult {
    private final KagemushaPeerTransport.Payload payload;
    private final KagemushaPeerTransport.Kind payloadKind;
    private final int receivedDataFrames;
    private final int totalDataFrames;
    private final int recoveredDataFrames;

    private DecodeResult(
        final KagemushaPeerTransport.Payload payload,
        final KagemushaPeerTransport.Kind payloadKind,
        final int receivedDataFrames,
        final int totalDataFrames,
        final int recoveredDataFrames) {
      this.payload = payload;
      this.payloadKind = payloadKind;
      this.receivedDataFrames = receivedDataFrames;
      this.totalDataFrames = totalDataFrames;
      this.recoveredDataFrames = recoveredDataFrames;
    }

    public KagemushaPeerTransport.Payload payload() {
      return payload;
    }

    public KagemushaPeerTransport.Kind payloadKind() {
      return payloadKind;
    }

    public int receivedDataFrames() {
      return receivedDataFrames;
    }

    public int totalDataFrames() {
      return totalDataFrames;
    }

    public int recoveredDataFrames() {
      return recoveredDataFrames;
    }

    public boolean isComplete() {
      return payload != null;
    }

    public double progress() {
      return totalDataFrames == 0
          ? 0.0
          : Math.min(1.0, (double) receivedDataFrames / totalDataFrames);
    }
  }

  /** Transactional, allocation-bounded, thread-safe stream decoder. */
  public static final class Decoder {
    private byte[] streamId;
    private Envelope envelope;
    private LinkedHashMap<Integer, byte[]> dataFrames = new LinkedHashMap<>();
    private LinkedHashMap<Integer, Integer> dataTotals = new LinkedHashMap<>();
    private LinkedHashMap<Integer, byte[]> parityFrames = new LinkedHashMap<>();
    private LinkedHashMap<Integer, Integer> parityTotals = new LinkedHashMap<>();
    private LinkedHashSet<Integer> recovered = new LinkedHashSet<>();
    private KagemushaPeerTransport.Payload completedPayload;

    public synchronized void reset() {
      clearMap(dataFrames);
      clearMap(parityFrames);
      if (streamId != null) Arrays.fill(streamId, (byte) 0);
      streamId = null;
      envelope = null;
      dataFrames = new LinkedHashMap<>();
      dataTotals = new LinkedHashMap<>();
      parityFrames = new LinkedHashMap<>();
      parityTotals = new LinkedHashMap<>();
      recovered = new LinkedHashSet<>();
      completedPayload = null;
    }

    public synchronized DecodeResult ingest(final String frameText) {
      final Frame frame = decodeFrameText(frameText);
      final Snapshot snapshot = snapshot();
      try {
        final DecodeResult result = ingest(frame);
        snapshot.clearCopies();
        return result;
      } catch (RuntimeException failure) {
        restore(snapshot);
        throw failure;
      }
    }

    private DecodeResult ingest(final Frame frame) {
      final byte[] frameStreamId = frame.streamId();
      try {
        if (streamId == null) streamId = frameStreamId.clone();
        else require(Arrays.equals(streamId, frameStreamId),
            "Kagemusha QR frame belongs to another stream");
      } finally {
        Arrays.fill(frameStreamId, (byte) 0);
      }

      switch (frame.kind()) {
        case HEADER -> {
          final Envelope decoded = Envelope.decode(frame.payload());
          final byte[] decodedStreamId = decoded.streamId();
          final byte[] expectedStreamId = frame.streamId();
          try {
            require(Arrays.equals(decodedStreamId, expectedStreamId),
                "Kagemusha QR digest mismatch");
          } finally {
            Arrays.fill(decodedStreamId, (byte) 0);
            Arrays.fill(expectedStreamId, (byte) 0);
          }
          if (envelope != null) require(envelope.equals(decoded),
              "Conflicting Kagemusha QR header");
          envelope = decoded;
        }
        case DATA -> {
          require(frame.total() <= MAXIMUM_DATA_FRAMES,
              "Kagemusha QR data frame count is invalid");
          store(frame, dataFrames, dataTotals);
        }
        case PARITY -> {
          require(frame.total() <= MAXIMUM_PARITY_FRAMES,
              "Kagemusha QR parity frame count is invalid");
          store(frame, parityFrames, parityTotals);
        }
      }

      if (envelope != null) {
        validateBuffered(envelope);
        recover(envelope);
        if (completedPayload == null) completedPayload = finalizePayload(envelope);
      }
      return result();
    }

    private static void store(
        final Frame frame,
        final Map<Integer, byte[]> frames,
        final Map<Integer, Integer> totals) {
      require(frame.index() >= 0 && frame.index() < frame.total(),
          "Kagemusha QR frame index is invalid");
      final byte[] payload = frame.payload();
      final byte[] previous = frames.get(frame.index());
      if (previous != null) {
        try {
          require(
              Arrays.equals(previous, payload)
                  && Objects.equals(totals.get(frame.index()), frame.total()),
              "Conflicting duplicate Kagemusha QR frame");
        } finally {
          Arrays.fill(payload, (byte) 0);
        }
      } else {
        frames.put(frame.index(), payload);
        totals.put(frame.index(), frame.total());
      }
    }

    private void validateBuffered(final Envelope header) {
      for (final Map.Entry<Integer, byte[]> entry : dataFrames.entrySet()) {
        final int index = entry.getKey();
        require(
            index < header.dataChunks
                && Objects.equals(dataTotals.get(index), header.dataChunks)
                && entry.getValue().length == header.expectedDataChunkLength(index),
            "Kagemusha QR data frame does not match its header");
      }
      for (final Map.Entry<Integer, byte[]> entry : parityFrames.entrySet()) {
        final int index = entry.getKey();
        require(
            index < header.parityChunks
                && Objects.equals(parityTotals.get(index), header.parityChunks)
                && entry.getValue().length == header.chunkSize,
            "Kagemusha QR parity frame does not match its header");
      }
    }

    private void recover(final Envelope header) {
      for (int group = 0; group < header.parityChunks; group++) {
        final byte[] parity = parityFrames.get(group);
        if (parity == null) continue;
        final int start = group * header.parityGroup;
        final int end = Math.min(start + header.parityGroup, header.dataChunks);
        int missing = -1;
        int missingCount = 0;
        for (int index = start; index < end; index++) {
          if (!dataFrames.containsKey(index)) {
            missing = index;
            missingCount++;
          }
        }
        if (missingCount != 1) continue;
        final byte[] chunk = parity.clone();
        for (int index = start; index < end; index++) {
          if (index == missing) continue;
          final byte[] present = dataFrames.get(index);
          require(present != null, "Incomplete Kagemusha QR parity group");
          for (int byteIndex = 0; byteIndex < present.length; byteIndex++) {
            chunk[byteIndex] ^= present[byteIndex];
          }
        }
        final byte[] exact = Arrays.copyOf(chunk, header.expectedDataChunkLength(missing));
        Arrays.fill(chunk, (byte) 0);
        dataFrames.put(missing, exact);
        dataTotals.put(missing, header.dataChunks);
        recovered.add(missing);
      }
    }

    private KagemushaPeerTransport.Payload finalizePayload(final Envelope header) {
      if (dataFrames.size() != header.dataChunks) return null;
      final ByteArrayOutputStream output = new ByteArrayOutputStream(header.totalBytes);
      for (int index = 0; index < header.dataChunks; index++) {
        final byte[] chunk = dataFrames.get(index);
        if (chunk == null) return null;
        output.write(chunk, 0, chunk.length);
      }
      final byte[] archive = output.toByteArray();
      try {
        require(archive.length == header.totalBytes, "Kagemusha QR archive size mismatch");
        require(Arrays.equals(sha256(archive), header.payloadDigest),
            "Kagemusha QR digest mismatch");
        return KagemushaPeerTransport.Payload.decode(archive, header.payloadKind);
      } finally {
        Arrays.fill(archive, (byte) 0);
      }
    }

    private DecodeResult result() {
      return new DecodeResult(
          completedPayload,
          envelope == null ? null : envelope.payloadKind,
          dataFrames.size(),
          envelope == null ? 0 : envelope.dataChunks,
          recovered.size());
    }

    private Snapshot snapshot() {
      return new Snapshot(
          streamId == null ? null : streamId.clone(),
          envelope,
          copyMap(dataFrames),
          new LinkedHashMap<>(dataTotals),
          copyMap(parityFrames),
          new LinkedHashMap<>(parityTotals),
          new LinkedHashSet<>(recovered),
          completedPayload);
    }

    private void restore(final Snapshot snapshot) {
      clearMap(dataFrames);
      clearMap(parityFrames);
      if (streamId != null) Arrays.fill(streamId, (byte) 0);
      streamId = snapshot.streamId;
      envelope = snapshot.envelope;
      dataFrames = snapshot.dataFrames;
      dataTotals = snapshot.dataTotals;
      parityFrames = snapshot.parityFrames;
      parityTotals = snapshot.parityTotals;
      recovered = snapshot.recovered;
      completedPayload = snapshot.completedPayload;
    }
  }

  private static final class Snapshot {
    private final byte[] streamId;
    private final Envelope envelope;
    private final LinkedHashMap<Integer, byte[]> dataFrames;
    private final LinkedHashMap<Integer, Integer> dataTotals;
    private final LinkedHashMap<Integer, byte[]> parityFrames;
    private final LinkedHashMap<Integer, Integer> parityTotals;
    private final LinkedHashSet<Integer> recovered;
    private final KagemushaPeerTransport.Payload completedPayload;

    private Snapshot(
        final byte[] streamId,
        final Envelope envelope,
        final LinkedHashMap<Integer, byte[]> dataFrames,
        final LinkedHashMap<Integer, Integer> dataTotals,
        final LinkedHashMap<Integer, byte[]> parityFrames,
        final LinkedHashMap<Integer, Integer> parityTotals,
        final LinkedHashSet<Integer> recovered,
        final KagemushaPeerTransport.Payload completedPayload) {
      this.streamId = streamId;
      this.envelope = envelope;
      this.dataFrames = dataFrames;
      this.dataTotals = dataTotals;
      this.parityFrames = parityFrames;
      this.parityTotals = parityTotals;
      this.recovered = recovered;
      this.completedPayload = completedPayload;
    }

    private void clearCopies() {
      if (streamId != null) Arrays.fill(streamId, (byte) 0);
      clearMap(dataFrames);
      clearMap(parityFrames);
    }
  }

  private static final class Envelope {
    private final KagemushaPeerTransport.Kind payloadKind;
    private final int parityGroup;
    private final int chunkSize;
    private final int dataChunks;
    private final int parityChunks;
    private final int totalBytes;
    private final byte[] payloadDigest;

    private Envelope(
        final KagemushaPeerTransport.Kind payloadKind,
        final int parityGroup,
        final int chunkSize,
        final int dataChunks,
        final int parityChunks,
        final int totalBytes,
        final byte[] payloadDigest) {
      this.payloadKind = Objects.requireNonNull(payloadKind, "payloadKind");
      this.payloadDigest = Objects.requireNonNull(payloadDigest, "payloadDigest").clone();
      require(chunkSize >= MINIMUM_CHUNK_SIZE && chunkSize <= MAXIMUM_CHUNK_SIZE,
          "Invalid Kagemusha QR header");
      require(parityGroup >= MINIMUM_PARITY_GROUP && parityGroup <= MAXIMUM_PARITY_GROUP,
          "Invalid Kagemusha QR header");
      require(totalBytes >= 1 && totalBytes <= KagemushaPeerTransport.MAXIMUM_ARCHIVE_BYTES,
          "Invalid Kagemusha QR header");
      require(dataChunks == (totalBytes + chunkSize - 1) / chunkSize,
          "Invalid Kagemusha QR header");
      require(parityChunks == (dataChunks + parityGroup - 1) / parityGroup,
          "Invalid Kagemusha QR header");
      require(this.payloadDigest.length == 32 && anyNonzero(this.payloadDigest),
          "Invalid Kagemusha QR header");
      this.parityGroup = parityGroup;
      this.chunkSize = chunkSize;
      this.dataChunks = dataChunks;
      this.parityChunks = parityChunks;
      this.totalBytes = totalBytes;
    }

    private static Envelope create(
        final KagemushaPeerTransport.Kind kind,
        final byte[] payload,
        final Options options) {
      require(payload.length > 0 && payload.length <= KagemushaPeerTransport.MAXIMUM_ARCHIVE_BYTES,
          "Kagemusha QR payload exceeds its bound");
      final int dataChunks = (payload.length + options.chunkSize() - 1) / options.chunkSize();
      return new Envelope(
          kind,
          options.parityGroup(),
          options.chunkSize(),
          dataChunks,
          (dataChunks + options.parityGroup() - 1) / options.parityGroup(),
          payload.length,
          sha256(payload));
    }

    private byte[] streamId() {
      return Arrays.copyOf(payloadDigest, 16);
    }

    private int expectedDataChunkLength(final int index) {
      return index == dataChunks - 1 ? totalBytes - index * chunkSize : chunkSize;
    }

    private byte[] encode() {
      final byte[] out = new byte[ENVELOPE_BYTES];
      out[0] = ENVELOPE_VERSION;
      out[1] = (byte) payloadKind.code();
      out[2] = (byte) parityGroup;
      out[3] = 0;
      writeU16(out, 4, chunkSize);
      writeU16(out, 6, dataChunks);
      writeU16(out, 8, parityChunks);
      writeU32(out, 10, totalBytes);
      System.arraycopy(payloadDigest, 0, out, 14, payloadDigest.length);
      return out;
    }

    private static Envelope decode(final byte[] data) {
      Objects.requireNonNull(data, "data");
      require(
          data.length == ENVELOPE_BYTES
              && (data[0] & 0xff) == ENVELOPE_VERSION
              && data[3] == 0,
          "Invalid Kagemusha QR header");
      final KagemushaPeerTransport.Kind kind =
          KagemushaPeerTransport.Kind.fromCode(data[1] & 0xff);
      require(kind != null, "Invalid Kagemusha QR payload kind");
      final long totalBytes = readU32(data, 10);
      require(totalBytes <= Integer.MAX_VALUE, "Invalid Kagemusha QR header");
      return new Envelope(
          kind,
          data[2] & 0xff,
          readU16(data, 4),
          readU16(data, 6),
          readU16(data, 8),
          (int) totalBytes,
          Arrays.copyOfRange(data, 14, 46));
    }

    @Override
    public boolean equals(final Object other) {
      return other instanceof Envelope that
          && payloadKind == that.payloadKind
          && parityGroup == that.parityGroup
          && chunkSize == that.chunkSize
          && dataChunks == that.dataChunks
          && parityChunks == that.parityChunks
          && totalBytes == that.totalBytes
          && Arrays.equals(payloadDigest, that.payloadDigest);
    }

    @Override
    public int hashCode() {
      return 31 * payloadKind.hashCode() + Arrays.hashCode(payloadDigest);
    }
  }

  private static List<byte[]> chunks(final byte[] data, final int size) {
    final List<byte[]> chunks = new ArrayList<>((data.length + size - 1) / size);
    for (int start = 0; start < data.length; start += size) {
      chunks.add(Arrays.copyOfRange(data, start, Math.min(start + size, data.length)));
    }
    return chunks;
  }

  private static byte[] sha256(final byte[] value) {
    try {
      return MessageDigest.getInstance("SHA-256").digest(value);
    } catch (NoSuchAlgorithmException failure) {
      throw new IllegalStateException("SHA-256 is unavailable", failure);
    }
  }

  private static long crc32(final byte[] value, final int start, final int endExclusive) {
    final CRC32 crc = new CRC32();
    crc.update(value, start, endExclusive - start);
    return crc.getValue();
  }

  private static void writeU16(final byte[] out, final int offset, final int value) {
    out[offset] = (byte) (value >>> 8);
    out[offset + 1] = (byte) value;
  }

  private static int readU16(final byte[] value, final int offset) {
    return ((value[offset] & 0xff) << 8) | (value[offset + 1] & 0xff);
  }

  private static void writeU32(final byte[] out, final int offset, final long value) {
    out[offset] = (byte) (value >>> 24);
    out[offset + 1] = (byte) (value >>> 16);
    out[offset + 2] = (byte) (value >>> 8);
    out[offset + 3] = (byte) value;
  }

  private static long readU32(final byte[] value, final int offset) {
    return ((long) (value[offset] & 0xff) << 24)
        | ((long) (value[offset + 1] & 0xff) << 16)
        | ((long) (value[offset + 2] & 0xff) << 8)
        | (long) (value[offset + 3] & 0xff);
  }

  private static LinkedHashMap<Integer, byte[]> copyMap(final Map<Integer, byte[]> source) {
    final LinkedHashMap<Integer, byte[]> output = new LinkedHashMap<>();
    for (final Map.Entry<Integer, byte[]> entry : source.entrySet()) {
      output.put(entry.getKey(), entry.getValue().clone());
    }
    return output;
  }

  private static void clearMap(final Map<Integer, byte[]> source) {
    for (final byte[] value : source.values()) Arrays.fill(value, (byte) 0);
  }

  private static boolean anyNonzero(final byte[] value) {
    for (final byte item : value) if (item != 0) return true;
    return false;
  }

  private static void require(final boolean condition, final String message) {
    if (!condition) throw new IllegalArgumentException(message);
  }
}

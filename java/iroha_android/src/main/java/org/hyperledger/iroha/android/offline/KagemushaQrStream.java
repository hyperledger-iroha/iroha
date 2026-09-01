package org.hyperledger.iroha.android.offline;

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
import java.util.zip.CRC32;

/**
 * Canonical fixed-XOR Kagemusha QR stream shared with the Swift and Kotlin SDKs.
 *
 * <p>The decoder bounds every allocation before decoding, requires the authenticated header first,
 * accepts subsequent frames out of order, and recovers at most one missing data frame per fixed
 * parity group. Invalid frames are rejected before mutation or roll back only their local mutation.
 */
public final class KagemushaQrStream {
  public static final int MINIMUM_CHUNK_SIZE = 64;
  public static final int MAXIMUM_CHUNK_SIZE = 512;
  public static final int MINIMUM_PARITY_GROUP = 2;
  public static final int MAXIMUM_PARITY_GROUP = 16;
  public static final int STANDARD_CHUNK_SIZE = 256;
  public static final int STANDARD_PARITY_GROUP = 4;
  /** Maximum frame count for one stream, including its header. */
  public static final int MAXIMUM_STREAM_FRAMES = 4096;

  private static final int ENVELOPE_VERSION = 1;
  private static final int ENVELOPE_BYTES = 50;
  private static final int FRAME_VERSION = 1;
  private static final int FRAME_FIXED_OVERHEAD = 34;
  private static final int MAXIMUM_FRAME_BYTES = FRAME_FIXED_OVERHEAD + MAXIMUM_CHUNK_SIZE;
  public static final int MAXIMUM_FRAME_TEXT_BYTES =
      KagemushaPeerTransport.QR_STREAM_TEXT_PREFIX.length()
          + (MAXIMUM_FRAME_BYTES * 4 + 2) / 3;

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
      final List<Frame> frames =
          new ArrayList<>(1 + envelope.dataChunks + envelope.parityChunks);
      final List<byte[]> chunks = new ArrayList<>(envelope.dataChunks);
      try {
        final byte[] headerBytes = envelope.encode();
        try {
          frames.add(new Frame(FrameKind.HEADER, streamId, 0, 1, headerBytes));
        } finally {
          Arrays.fill(headerBytes, (byte) 0);
        }

        chunks.addAll(chunks(archive, options.chunkSize()));
        for (int index = 0; index < chunks.size(); index++) {
          frames.add(new Frame(FrameKind.DATA, streamId, index, chunks.size(), chunks.get(index)));
        }
        for (int group = 0; group < envelope.parityChunks; group++) {
          final byte[] parity = new byte[options.chunkSize()];
          try {
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
          } finally {
            Arrays.fill(parity, (byte) 0);
          }
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
        clearArrays(chunks);
        for (final Frame frame : frames) frame.zeroize();
        Arrays.fill(streamId, (byte) 0);
        envelope.zeroize();
      }
    } finally {
      Arrays.fill(archive, (byte) 0);
    }
  }

  static int preflightStreamFrameCount(final int payloadBytes, final Options options) {
    Objects.requireNonNull(options, "options");
    require(
        payloadBytes >= 1 && payloadBytes <= KagemushaPeerTransport.MAXIMUM_ARCHIVE_BYTES,
        "Kagemusha QR payload exceeds its bound");
    final int dataChunks = (payloadBytes + options.chunkSize() - 1) / options.chunkSize();
    final int parityChunks =
        (dataChunks + options.parityGroup() - 1) / options.parityGroup();
    final int frameCount = 1 + dataChunks + parityChunks;
    require(
        frameCount <= MAXIMUM_STREAM_FRAMES,
        "Kagemusha QR stream requires " + frameCount
            + " frames; the limit is " + MAXIMUM_STREAM_FRAMES);
    return frameCount;
  }

  public static Frame decodeFrameText(final String value) {
    Objects.requireNonNull(value, "value");
    require(
        value.length() <= MAXIMUM_FRAME_TEXT_BYTES
            && value.startsWith(KagemushaPeerTransport.QR_STREAM_TEXT_PREFIX)
            && isAscii(value),
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

  /** One externally immutable binary Kagemusha QR frame. */
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
      Objects.requireNonNull(streamId, "streamId");
      Objects.requireNonNull(payload, "payload");
      require(
          streamId.length == 16 && anyNonzero(streamId),
          "Malformed Kagemusha QR stream identifier");
      require(total >= 1 && total < MAXIMUM_STREAM_FRAMES && index >= 0 && index < total,
          "Malformed Kagemusha QR frame index");
      require(payload.length > 0 && payload.length <= MAXIMUM_CHUNK_SIZE,
          "Malformed Kagemusha QR frame payload");
      switch (kind) {
        case HEADER -> require(index == 0 && total == 1 && payload.length == ENVELOPE_BYTES,
            "Malformed Kagemusha QR header frame");
        case DATA, PARITY -> { }
      }
      this.index = index;
      this.total = total;
      this.streamId = streamId.clone();
      this.payload = payload.clone();
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
      final int payloadEnd = 30 + payload.length;
      final byte[] out = new byte[payloadEnd + 4];
      out[0] = 0x4b;
      out[1] = 0x51;
      out[2] = FRAME_VERSION;
      out[3] = (byte) kind.code;
      System.arraycopy(streamId, 0, out, 4, streamId.length);
      writeU32(out, 20, index);
      writeU32(out, 24, total);
      writeU16(out, 28, payload.length);
      System.arraycopy(payload, 0, out, 30, payload.length);
      writeU32(out, payloadEnd, crc32(out, 2, payloadEnd));
      return out;
    }

    void zeroize() {
      Arrays.fill(streamId, (byte) 0);
      Arrays.fill(payload, (byte) 0);
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
      final int payloadLength = readU16(data, 28);
      final int payloadEnd = 30 + payloadLength;
      require(payloadEnd + 4 == data.length, "Malformed Kagemusha QR frame length");
      require(readU32(data, payloadEnd) == crc32(data, 2, payloadEnd),
          "Kagemusha QR frame checksum mismatch");
      final byte[] streamId = Arrays.copyOfRange(data, 4, 20);
      final byte[] payload = Arrays.copyOfRange(data, 30, payloadEnd);
      try {
        return new Frame(
            kind,
            streamId,
            readU32Int(data, 20),
            readU32Int(data, 24),
            payload);
      } finally {
        Arrays.fill(streamId, (byte) 0);
        Arrays.fill(payload, (byte) 0);
      }
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
    private LinkedHashMap<Integer, byte[]> parityFrames = new LinkedHashMap<>();
    private LinkedHashSet<Integer> recovered = new LinkedHashSet<>();
    private KagemushaPeerTransport.Payload completedPayload;

    public synchronized void reset() {
      resetState();
    }

    private void resetState() {
      clearMap(dataFrames);
      clearMap(parityFrames);
      if (streamId != null) Arrays.fill(streamId, (byte) 0);
      if (envelope != null) envelope.zeroize();
      streamId = null;
      envelope = null;
      dataFrames = new LinkedHashMap<>();
      parityFrames = new LinkedHashMap<>();
      recovered = new LinkedHashSet<>();
      completedPayload = null;
    }

    public synchronized DecodeResult ingest(final String frameText) {
      final Frame frame = decodeFrameText(frameText);
      try {
        return ingest(frame);
      } finally {
        frame.zeroize();
      }
    }

    private DecodeResult ingest(final Frame frame) {
      if (envelope == null) {
        require(frame.kind() == FrameKind.HEADER,
            "Kagemusha QR header must be ingested first");
        final byte[] headerBytes = frame.payload();
        final Envelope decoded;
        try {
          decoded = Envelope.decode(headerBytes);
        } finally {
          Arrays.fill(headerBytes, (byte) 0);
        }
        final byte[] decodedStreamId = decoded.streamId();
        final byte[] frameStreamId = frame.streamId();
        boolean retained = false;
        try {
          require(Arrays.equals(decodedStreamId, frameStreamId),
              "Kagemusha QR digest mismatch");
          streamId = frameStreamId.clone();
          envelope = decoded;
          retained = true;
          return result();
        } finally {
          Arrays.fill(decodedStreamId, (byte) 0);
          Arrays.fill(frameStreamId, (byte) 0);
          if (!retained) decoded.zeroize();
        }
      }

      final byte[] frameStreamId = frame.streamId();
      try {
        require(Arrays.equals(streamId, frameStreamId),
            "Kagemusha QR frame belongs to another stream");
      } finally {
        Arrays.fill(frameStreamId, (byte) 0);
      }

      switch (frame.kind()) {
        case HEADER -> {
          final byte[] headerBytes = frame.payload();
          final Envelope decoded;
          try {
            decoded = Envelope.decode(headerBytes);
          } finally {
            Arrays.fill(headerBytes, (byte) 0);
          }
          final byte[] decodedStreamId = decoded.streamId();
          final byte[] expectedStreamId = frame.streamId();
          try {
            require(Arrays.equals(decodedStreamId, expectedStreamId),
                "Kagemusha QR digest mismatch");
            require(envelope.equals(decoded), "Conflicting Kagemusha QR header");
          } finally {
            Arrays.fill(decodedStreamId, (byte) 0);
            Arrays.fill(expectedStreamId, (byte) 0);
            decoded.zeroize();
          }
        }
        case DATA -> ingestData(frame, envelope);
        case PARITY -> ingestParity(frame, envelope);
      }
      return result();
    }

    private void ingestData(final Frame frame, final Envelope header) {
      require(frame.total() == header.dataChunks
              && frame.index() >= 0
              && frame.index() < header.dataChunks,
          "Kagemusha QR data frame count is invalid");
      final byte[] payload = frame.payload();
      if (payload.length != header.expectedDataChunkLength(frame.index())) {
        Arrays.fill(payload, (byte) 0);
        throw new IllegalArgumentException("Kagemusha QR data frame does not match its header");
      }
      final byte[] previous = dataFrames.get(frame.index());
      if (previous != null) {
        try {
          require(Arrays.equals(previous, payload),
              "Conflicting duplicate Kagemusha QR frame");
        } finally {
          Arrays.fill(payload, (byte) 0);
        }
        return;
      }
      ingestNewFrame(frame, dataFrames, payload, frame.index() / header.parityGroup, header);
    }

    private void ingestParity(final Frame frame, final Envelope header) {
      require(frame.total() == header.parityChunks
              && frame.index() >= 0
              && frame.index() < header.parityChunks,
          "Kagemusha QR parity frame count is invalid");
      final byte[] payload = frame.payload();
      if (payload.length != header.chunkSize) {
        Arrays.fill(payload, (byte) 0);
        throw new IllegalArgumentException("Kagemusha QR parity frame does not match its header");
      }
      final byte[] previous = parityFrames.get(frame.index());
      if (previous != null) {
        try {
          require(Arrays.equals(previous, payload),
              "Conflicting duplicate Kagemusha QR frame");
        } finally {
          Arrays.fill(payload, (byte) 0);
        }
        return;
      }
      ingestNewFrame(frame, parityFrames, payload, frame.index(), header);
    }

    private void ingestNewFrame(
        final Frame frame,
        final LinkedHashMap<Integer, byte[]> frames,
        final byte[] payload,
        final int parityGroup,
        final Envelope header) {
      frames.put(frame.index(), payload);
      Integer recoveredIndex = null;
      try {
        recoveredIndex = recoverGroup(header, parityGroup);
      } catch (RuntimeException failure) {
        if (recoveredIndex != null) {
          final byte[] removed = dataFrames.remove(recoveredIndex);
          if (removed != null) Arrays.fill(removed, (byte) 0);
          recovered.remove(recoveredIndex);
        }
        final byte[] removed = frames.remove(frame.index());
        if (removed != null) Arrays.fill(removed, (byte) 0);
        throw failure;
      }
      if (completedPayload != null || dataFrames.size() != header.dataChunks) return;
      try {
        completedPayload = finalizePayload(header);
      } catch (RuntimeException failure) {
        // Exact coverage consumes a failing stream so another final-frame retry
        // cannot repeat the whole allocation/hash/decode operation.
        resetState();
        throw failure;
      }
    }

    private Integer recoverGroup(final Envelope header, final int group) {
      final byte[] parity = parityFrames.get(group);
      if (parity == null) return null;
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
      if (missingCount != 1) return null;
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
      recovered.add(missing);
      return missing;
    }

    private KagemushaPeerTransport.Payload finalizePayload(final Envelope header) {
      require(dataFrames.size() == header.dataChunks, "Kagemusha QR archive is incomplete");
      final byte[] archive = new byte[header.totalBytes];
      try {
        int offset = 0;
        for (int index = 0; index < header.dataChunks; index++) {
          final byte[] chunk = dataFrames.get(index);
          require(chunk != null, "Kagemusha QR archive is incomplete");
          System.arraycopy(chunk, 0, archive, offset, chunk.length);
          offset += chunk.length;
        }
        require(offset == header.totalBytes, "Kagemusha QR archive size mismatch");
        final byte[] digest = sha256(archive);
        try {
          require(Arrays.equals(digest, header.payloadDigest), "Kagemusha QR digest mismatch");
        } finally {
          Arrays.fill(digest, (byte) 0);
        }
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
      Objects.requireNonNull(payloadDigest, "payloadDigest");
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
      require(1L + dataChunks + parityChunks <= MAXIMUM_STREAM_FRAMES,
          "Invalid Kagemusha QR header");
      require(payloadDigest.length == 32 && anyNonzero(payloadDigest),
          "Invalid Kagemusha QR header");
      this.payloadDigest = payloadDigest.clone();
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
      preflightStreamFrameCount(payload.length, options);
      final int dataChunks = (payload.length + options.chunkSize() - 1) / options.chunkSize();
      final byte[] digest = sha256(payload);
      try {
        return new Envelope(
            kind,
            options.parityGroup(),
            options.chunkSize(),
            dataChunks,
            (dataChunks + options.parityGroup() - 1) / options.parityGroup(),
            payload.length,
            digest);
      } finally {
        Arrays.fill(digest, (byte) 0);
      }
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
      writeU32(out, 6, dataChunks);
      writeU32(out, 10, parityChunks);
      writeU32(out, 14, totalBytes);
      System.arraycopy(payloadDigest, 0, out, 18, payloadDigest.length);
      return out;
    }

    private void zeroize() {
      Arrays.fill(payloadDigest, (byte) 0);
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
      final long totalBytes = readU32(data, 14);
      require(totalBytes <= Integer.MAX_VALUE, "Invalid Kagemusha QR header");
      final byte[] digest = Arrays.copyOfRange(data, 18, 50);
      try {
        return new Envelope(
            kind,
            data[2] & 0xff,
            readU16(data, 4),
            readU32Int(data, 6),
            readU32Int(data, 10),
            (int) totalBytes,
            digest);
      } finally {
        Arrays.fill(digest, (byte) 0);
      }
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

  private static int readU32Int(final byte[] value, final int offset) {
    final long decoded = readU32(value, offset);
    require(decoded <= Integer.MAX_VALUE, "Kagemusha QR count exceeds the SDK limit");
    return (int) decoded;
  }

  private static void clearMap(final Map<Integer, byte[]> source) {
    for (final byte[] value : source.values()) Arrays.fill(value, (byte) 0);
  }

  private static void clearArrays(final List<byte[]> source) {
    for (final byte[] value : source) Arrays.fill(value, (byte) 0);
  }

  private static boolean anyNonzero(final byte[] value) {
    for (final byte item : value) if (item != 0) return true;
    return false;
  }

  private static boolean isAscii(final String value) {
    for (int index = 0; index < value.length(); index++) {
      if (value.charAt(index) > 0x7f) return false;
    }
    return true;
  }

  private static void require(final boolean condition, final String message) {
    if (!condition) throw new IllegalArgumentException(message);
  }
}

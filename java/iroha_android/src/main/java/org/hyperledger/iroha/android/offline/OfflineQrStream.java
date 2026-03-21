package org.hyperledger.iroha.android.offline;

import java.util.ArrayList;
import java.util.Arrays;
import java.util.HashSet;
import java.util.List;
import java.util.Objects;
import java.util.Set;
import java.util.zip.CRC32;
import org.hyperledger.iroha.android.crypto.Blake2b;

/** QR stream framing helpers for offline payload transfer. */
public final class OfflineQrStream {

  private static final byte[] MAGIC = {(byte) 0x49, (byte) 0x51};
  private static final byte FRAME_VERSION = 1;
  private static final byte ENVELOPE_VERSION = 1;
  private static final byte ENCODING_BINARY = 0;
  private static final int ENVELOPE_LENGTH = 48;

  private OfflineQrStream() {}

  public enum FrameKind {
    HEADER(0),
    DATA(1),
    PARITY(2);

    private final int value;

    FrameKind(final int value) {
      this.value = value;
    }

    public int value() {
      return value;
    }

    public static FrameKind fromValue(final int value) {
      for (FrameKind kind : values()) {
        if (kind.value == value) {
          return kind;
        }
      }
      throw new IllegalArgumentException("Unsupported frame kind: " + value);
    }
  }

  public enum FrameEncoding {
    BINARY,
    BASE64,
  }

  public enum PayloadKind {
    UNSPECIFIED(0),
    OFFLINE_TO_ONLINE_TRANSFER(1),
    OFFLINE_SPEND_RECEIPT(2),
    OFFLINE_ENVELOPE(3);

    private final int value;

    PayloadKind(final int value) {
      this.value = value;
    }

    public int value() {
      return value;
    }
  }

  public static final class TextCodec {
    private static final String PREFIX = "iroha:qr1:";

    private TextCodec() {}

    public static String encode(final byte[] data, final FrameEncoding encoding) {
      Objects.requireNonNull(data, "data");
      Objects.requireNonNull(encoding, "encoding");
      final String base64 = java.util.Base64.getEncoder().encodeToString(data);
      if (encoding == FrameEncoding.BASE64) {
        return PREFIX + base64;
      }
      return base64;
    }

    public static byte[] decode(final String value, final FrameEncoding encoding) {
      Objects.requireNonNull(value, "value");
      Objects.requireNonNull(encoding, "encoding");
      final String trimmed = value.trim();
      final String payload;
      if (encoding == FrameEncoding.BASE64) {
        if (!trimmed.startsWith(PREFIX)) {
          throw new IllegalArgumentException("QR text prefix missing");
        }
        payload = trimmed.substring(PREFIX.length());
      } else {
        payload = trimmed;
      }
      return java.util.Base64.getDecoder().decode(payload);
    }
  }

  public static final class Options {
    private final int chunkSize;
    private final int parityGroup;

    public Options() {
      this(360, 0);
    }

    public Options(final int chunkSize, final int parityGroup) {
      if (chunkSize <= 0 || chunkSize > 0xFFFF) {
        throw new IllegalArgumentException("chunkSize must be between 1 and 65535");
      }
      if (parityGroup < 0 || parityGroup > 0xFF) {
        throw new IllegalArgumentException("parityGroup must be between 0 and 255");
      }
      this.chunkSize = chunkSize;
      this.parityGroup = parityGroup;
    }

    public int chunkSize() {
      return chunkSize;
    }

    public int parityGroup() {
      return parityGroup;
    }
  }

  public static final class Envelope {
    private final int flags;
    private final int encoding;
    private final int parityGroup;
    private final int chunkSize;
    private final int dataChunks;
    private final int parityChunks;
    private final int payloadKind;
    private final long payloadLength;
    private final byte[] payloadHash;

    public Envelope(
        final int flags,
        final int encoding,
        final int parityGroup,
        final int chunkSize,
        final int dataChunks,
        final int parityChunks,
        final int payloadKind,
        final long payloadLength,
        final byte[] payloadHash) {
      if (payloadHash == null || payloadHash.length != 32) {
        throw new IllegalArgumentException("payloadHash must be 32 bytes");
      }
      this.flags = flags;
      this.encoding = encoding;
      this.parityGroup = parityGroup;
      this.chunkSize = chunkSize;
      this.dataChunks = dataChunks;
      this.parityChunks = parityChunks;
      this.payloadKind = payloadKind;
      this.payloadLength = payloadLength;
      this.payloadHash = payloadHash.clone();
    }

    public byte[] streamId() {
      return Arrays.copyOf(payloadHash, 16);
    }

    public int chunkSize() {
      return chunkSize;
    }

    public int dataChunks() {
      return dataChunks;
    }

    public int parityChunks() {
      return parityChunks;
    }

    public int parityGroup() {
      return parityGroup;
    }

    public long payloadLength() {
      return payloadLength;
    }

    public byte[] payloadHash() {
      return payloadHash.clone();
    }

    public byte[] encode() {
      final byte[] out = new byte[ENVELOPE_LENGTH];
      int offset = 0;
      out[offset++] = ENVELOPE_VERSION;
      out[offset++] = (byte) flags;
      out[offset++] = (byte) encoding;
      out[offset++] = (byte) parityGroup;
      writeUInt16LE(out, offset, chunkSize);
      offset += 2;
      writeUInt16LE(out, offset, dataChunks);
      offset += 2;
      writeUInt16LE(out, offset, parityChunks);
      offset += 2;
      writeUInt16LE(out, offset, payloadKind);
      offset += 2;
      writeUInt32LE(out, offset, payloadLength);
      offset += 4;
      System.arraycopy(payloadHash, 0, out, offset, payloadHash.length);
      return out;
    }

    public static Envelope decode(final byte[] bytes) {
      Objects.requireNonNull(bytes, "bytes");
      if (bytes.length < ENVELOPE_LENGTH) {
        throw new IllegalArgumentException("Envelope is too short");
      }
      int offset = 0;
      final int version = bytes[offset++] & 0xFF;
      if (version != ENVELOPE_VERSION) {
        throw new IllegalArgumentException("Unsupported envelope version: " + version);
      }
      final int flags = bytes[offset++] & 0xFF;
      final int encoding = bytes[offset++] & 0xFF;
      final int parityGroup = bytes[offset++] & 0xFF;
      final int chunkSize = readUInt16LE(bytes, offset);
      offset += 2;
      final int dataChunks = readUInt16LE(bytes, offset);
      offset += 2;
      final int parityChunks = readUInt16LE(bytes, offset);
      offset += 2;
      final int payloadKind = readUInt16LE(bytes, offset);
      offset += 2;
      final long payloadLength = readUInt32LE(bytes, offset);
      offset += 4;
      final byte[] payloadHash = Arrays.copyOfRange(bytes, offset, offset + 32);
      return new Envelope(
          flags,
          encoding,
          parityGroup,
          chunkSize,
          dataChunks,
          parityChunks,
          payloadKind,
          payloadLength,
          payloadHash);
    }
  }

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
      if (this.streamId.length != 16) {
        throw new IllegalArgumentException("streamId must be 16 bytes");
      }
      this.index = index;
      this.total = total;
      this.payload = payload == null ? new byte[0] : payload.clone();
    }

    public FrameKind kind() {
      return kind;
    }

    public byte[] payload() {
      return payload.clone();
    }

    public int index() {
      return index;
    }

    public byte[] streamId() {
      return streamId.clone();
    }

    public byte[] encode() {
      if (payload.length > 0xFFFF) {
        throw new IllegalArgumentException("payload length exceeds 65535");
      }
      final int headerLength = 2 + 1 + 1 + 16 + 2 + 2 + 2;
      final byte[] out = new byte[headerLength + payload.length + 4];
      int offset = 0;
      out[offset++] = MAGIC[0];
      out[offset++] = MAGIC[1];
      out[offset++] = FRAME_VERSION;
      out[offset++] = (byte) kind.value();
      System.arraycopy(streamId, 0, out, offset, streamId.length);
      offset += streamId.length;
      writeUInt16LE(out, offset, index);
      offset += 2;
      writeUInt16LE(out, offset, total);
      offset += 2;
      writeUInt16LE(out, offset, payload.length);
      offset += 2;
      System.arraycopy(payload, 0, out, offset, payload.length);
      offset += payload.length;
      final long crc = crc32(out, 2, offset - 2);
      writeUInt32LE(out, offset, crc);
      return out;
    }

    public static Frame decode(final byte[] bytes) {
      Objects.requireNonNull(bytes, "bytes");
      final int headerLength = 2 + 1 + 1 + 16 + 2 + 2 + 2;
      if (bytes.length < headerLength + 4) {
        throw new IllegalArgumentException("Frame is too short");
      }
      if (bytes[0] != MAGIC[0] || bytes[1] != MAGIC[1]) {
        throw new IllegalArgumentException("Frame magic mismatch");
      }
      final int version = bytes[2] & 0xFF;
      if (version != FRAME_VERSION) {
        throw new IllegalArgumentException("Unsupported frame version: " + version);
      }
      final FrameKind kind = FrameKind.fromValue(bytes[3] & 0xFF);
      final byte[] streamId = Arrays.copyOfRange(bytes, 4, 20);
      final int index = readUInt16LE(bytes, 20);
      final int total = readUInt16LE(bytes, 22);
      final int payloadLength = readUInt16LE(bytes, 24);
      final int payloadEnd = 26 + payloadLength;
      if (payloadEnd + 4 > bytes.length) {
        throw new IllegalArgumentException("Frame payload length exceeds buffer");
      }
      final byte[] payload = Arrays.copyOfRange(bytes, 26, payloadEnd);
      final long expected = readUInt32LE(bytes, payloadEnd);
      final long computed = crc32(bytes, 2, payloadEnd - 2);
      if (expected != computed) {
        throw new IllegalArgumentException("Frame checksum mismatch");
      }
      return new Frame(kind, streamId, index, total, payload);
    }
  }

  public static final class DecodeResult {
    private final byte[] payload;
    private final int receivedChunks;
    private final int totalChunks;
    private final int recoveredChunks;

    public DecodeResult(
        final byte[] payload,
        final int receivedChunks,
        final int totalChunks,
        final int recoveredChunks) {
      this.payload = payload;
      this.receivedChunks = receivedChunks;
      this.totalChunks = totalChunks;
      this.recoveredChunks = recoveredChunks;
    }

    public byte[] payload() {
      return payload;
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
      if (totalChunks == 0) {
        return 0.0;
      }
      return receivedChunks / (double) totalChunks;
    }
  }

  public static final class Encoder {
    public static List<Frame> encodeFrames(final byte[] payload) {
      return encodeFrames(payload, PayloadKind.UNSPECIFIED, new Options());
    }

    public static List<Frame> encodeFrames(
        final byte[] payload, final PayloadKind payloadKind, final Options options) {
      Objects.requireNonNull(payload, "payload");
      Objects.requireNonNull(options, "options");
      final int chunkSize = options.chunkSize();
      final int dataChunks =
          payload.length == 0 ? 0 : (int) Math.ceil(payload.length / (double) chunkSize);
      if (dataChunks > 0xFFFF) {
        throw new IllegalArgumentException("dataChunks exceeds 65535");
      }
      final int parityGroup = options.parityGroup();
      final int parityChunks =
          parityGroup > 0 ? (int) Math.ceil(dataChunks / (double) parityGroup) : 0;
      if (parityChunks > 0xFFFF) {
        throw new IllegalArgumentException("parityChunks exceeds 65535");
      }
      final byte[] payloadHash = Blake2b.digest256(payload);
      final Envelope envelope =
          new Envelope(
              0,
              ENCODING_BINARY,
              parityGroup,
              chunkSize,
              dataChunks,
              parityChunks,
              payloadKind.value(),
              payload.length,
              payloadHash);
      final List<Frame> frames = new ArrayList<>();
      frames.add(new Frame(FrameKind.HEADER, envelope.streamId(), 0, 1, envelope.encode()));
      for (int index = 0; index < dataChunks; index++) {
        final int start = index * chunkSize;
        final int end = Math.min(payload.length, start + chunkSize);
        final byte[] chunk = Arrays.copyOfRange(payload, start, end);
        frames.add(new Frame(FrameKind.DATA, envelope.streamId(), index, dataChunks, chunk));
      }
      if (parityGroup > 0) {
        for (int groupIndex = 0; groupIndex < parityChunks; groupIndex++) {
          final byte[] parity =
              xorParity(payload, chunkSize, dataChunks, groupIndex, parityGroup);
          frames.add(
              new Frame(FrameKind.PARITY, envelope.streamId(), groupIndex, parityChunks, parity));
        }
      }
      return frames;
    }

    public static List<byte[]> encodeFrameBytes(
        final byte[] payload, final PayloadKind payloadKind, final Options options) {
      final List<Frame> frames = encodeFrames(payload, payloadKind, options);
      final List<byte[]> out = new ArrayList<>(frames.size());
      for (Frame frame : frames) {
        out.add(frame.encode());
      }
      return out;
    }
  }

  public static final class Decoder {
    private Envelope envelope;
    private byte[][] dataChunks = new byte[0][];
    private byte[][] parityChunks = new byte[0][];
    private final List<Frame> pendingFrames = new ArrayList<>();
    private final Set<Integer> recovered = new HashSet<>();

    public DecodeResult ingest(final byte[] frameBytes) {
      final Frame frame = Frame.decode(frameBytes);
      ingest(frame);
      final byte[] payload = finalizeIfComplete();
      final int received = countNonNull(dataChunks);
      return new DecodeResult(payload, received, dataChunks.length, recovered.size());
    }

    private void ingest(final Frame frame) {
      if (frame.kind() == FrameKind.HEADER) {
        final Envelope decoded = Envelope.decode(frame.payload());
        if (!Arrays.equals(decoded.streamId(), frame.streamId())) {
          throw new IllegalArgumentException("Stream id mismatch");
        }
        if (envelope != null && Arrays.equals(envelope.streamId(), decoded.streamId())) {
          return;
        }
        envelope = decoded;
        dataChunks = new byte[decoded.dataChunks()][];
        parityChunks = new byte[decoded.parityChunks()][];
        if (!pendingFrames.isEmpty()) {
          final List<Frame> buffered = new ArrayList<>(pendingFrames);
          pendingFrames.clear();
          for (Frame bufferedFrame : buffered) {
            if (Arrays.equals(bufferedFrame.streamId(), decoded.streamId())) {
              ingest(bufferedFrame);
            }
          }
        }
        return;
      }
      if (envelope == null) {
        pendingFrames.add(frame);
        return;
      }
      if (!Arrays.equals(frame.streamId(), envelope.streamId())) {
        return;
      }
      if (frame.kind() == FrameKind.DATA) {
        final int index = frame.index();
        if (index < dataChunks.length && dataChunks[index] == null) {
          dataChunks[index] = frame.payload();
        }
      } else if (frame.kind() == FrameKind.PARITY) {
        final int index = frame.index();
        if (index < parityChunks.length && parityChunks[index] == null) {
          parityChunks[index] = frame.payload();
        }
      }
      recoverMissing();
    }

    private void recoverMissing() {
      if (envelope == null || envelope.parityGroup() == 0) {
        return;
      }
      final int groupSize = envelope.parityGroup();
      final int chunkSize = envelope.chunkSize();
      for (int groupIndex = 0; groupIndex < parityChunks.length; groupIndex++) {
        final byte[] parity = parityChunks[groupIndex];
        if (parity == null) {
          continue;
        }
        final int startIndex = groupIndex * groupSize;
        final int endIndex = Math.min(dataChunks.length, startIndex + groupSize);
        if (startIndex >= endIndex) {
          continue;
        }
        Integer missingIndex = null;
        final byte[] xor = Arrays.copyOf(parity, parity.length);
        for (int dataIndex = startIndex; dataIndex < endIndex; dataIndex++) {
          final byte[] chunk = dataChunks[dataIndex];
          if (chunk != null) {
            final byte[] padded = new byte[chunkSize];
            System.arraycopy(chunk, 0, padded, 0, Math.min(chunk.length, chunkSize));
            for (int i = 0; i < chunkSize; i++) {
              xor[i] ^= padded[i];
            }
          } else if (missingIndex == null) {
            missingIndex = dataIndex;
          } else {
            missingIndex = null;
            break;
          }
        }
        if (missingIndex != null) {
          final int length = expectedChunkLength(missingIndex);
          dataChunks[missingIndex] = Arrays.copyOf(xor, length);
          recovered.add(missingIndex);
        }
      }
    }

    private int expectedChunkLength(final int index) {
      if (envelope == null) {
        return 0;
      }
      final int chunkSize = envelope.chunkSize();
      final int total = envelope.dataChunks();
      if (total == 0) {
        return 0;
      }
      if (index < total - 1) {
        return chunkSize;
      }
      final long tail = envelope.payloadLength() - (long) chunkSize * (total - 1);
      return (int) Math.max(0, Math.min(chunkSize, tail));
    }

    private byte[] finalizeIfComplete() {
      if (envelope == null) {
        return null;
      }
      for (byte[] chunk : dataChunks) {
        if (chunk == null) {
          return null;
        }
      }
      int totalLength = 0;
      for (int i = 0; i < dataChunks.length; i++) {
        totalLength += expectedChunkLength(i);
      }
      final byte[] payload = new byte[totalLength];
      int offset = 0;
      for (int i = 0; i < dataChunks.length; i++) {
        final byte[] chunk = dataChunks[i];
        final int length = expectedChunkLength(i);
        System.arraycopy(chunk, 0, payload, offset, length);
        offset += length;
      }
      final byte[] hash = Blake2b.digest256(payload);
      if (!Arrays.equals(hash, envelope.payloadHash())) {
        throw new IllegalArgumentException("Payload hash mismatch");
      }
      return payload;
    }
  }

  public static final class ScanSession {
    private final Decoder decoder = new Decoder();

    public ScanSession() {}

    public DecodeResult ingest(final byte[] frameBytes) {
      return decoder.ingest(frameBytes);
    }
  }

  public static final class Color {
    public final double red;
    public final double green;
    public final double blue;

    public Color(final double red, final double green, final double blue) {
      this.red = red;
      this.green = green;
      this.blue = blue;
    }
  }

  public static final class FrameStyle {
    public final double petalPhase;
    public final double accentStrength;
    public final double gradientAngle;

    public FrameStyle(final double petalPhase, final double accentStrength, final double gradientAngle) {
      this.petalPhase = petalPhase;
      this.accentStrength = accentStrength;
      this.gradientAngle = gradientAngle;
    }
  }

  public static final class PlaybackStyle {
    public final double petalPhase;
    public final double accentStrength;
    public final double gradientAngle;
    public final double driftOffset;
    public final double progressAlpha;

    public PlaybackStyle(
        final double petalPhase,
        final double accentStrength,
        final double gradientAngle,
        final double driftOffset,
        final double progressAlpha) {
      this.petalPhase = petalPhase;
      this.accentStrength = accentStrength;
      this.gradientAngle = gradientAngle;
      this.driftOffset = driftOffset;
      this.progressAlpha = progressAlpha;
    }
  }

  public static final class Theme {
    public final String name;
    public final Color backgroundStart;
    public final Color backgroundEnd;
    public final Color accent;
    public final Color petal;
    public final int petalCount;
    public final double pulsePeriod;

    public Theme(
        final String name,
        final Color backgroundStart,
        final Color backgroundEnd,
        final Color accent,
        final Color petal,
        final int petalCount,
        final double pulsePeriod) {
      this.name = name;
      this.backgroundStart = backgroundStart;
      this.backgroundEnd = backgroundEnd;
      this.accent = accent;
      this.petal = petal;
      this.petalCount = petalCount;
      this.pulsePeriod = pulsePeriod;
    }

    public FrameStyle frameStyle(final int frameIndex, final int totalFrames) {
      final int safeTotal = Math.max(totalFrames, 1);
      final double phase = (frameIndex % safeTotal) / (double) safeTotal;
      final double pulse =
          (Math.sin((frameIndex / pulsePeriod) * Math.PI * 2.0) + 1.0) / 2.0;
      return new FrameStyle(phase, pulse, phase * 360.0);
    }
  }

  public static final class PlaybackSkin {
    public final String name;
    public final Theme theme;
    public final double frameRate;
    public final double petalDriftSpeed;
    public final double progressOverlayAlpha;
    public final boolean reducedMotion;
    public final boolean lowPower;

    public PlaybackSkin(
        final String name,
        final Theme theme,
        final double frameRate,
        final double petalDriftSpeed,
        final double progressOverlayAlpha,
        final boolean reducedMotion,
        final boolean lowPower) {
      this.name = name;
      this.theme = theme;
      this.frameRate = frameRate;
      this.petalDriftSpeed = petalDriftSpeed;
      this.progressOverlayAlpha = progressOverlayAlpha;
      this.reducedMotion = reducedMotion;
      this.lowPower = lowPower;
    }

    public PlaybackStyle frameStyle(final int frameIndex, final int totalFrames, final double progress) {
      final int safeTotal = Math.max(totalFrames, 1);
      final double phase = (frameIndex % safeTotal) / (double) safeTotal;
      final double pulse =
          (Math.sin((frameIndex / theme.pulsePeriod) * Math.PI * 2.0) + 1.0) / 2.0;
      final double angle = phase * 360.0;
      final double drift = reducedMotion ? 0.0 : Math.sin(phase * Math.PI * 2.0) * petalDriftSpeed;
      final double clamped = Math.min(Math.max(progress, 0.0), 1.0);
      return new PlaybackStyle(phase, pulse, angle, drift, progressOverlayAlpha * clamped);
    }
  }

  public static final Theme SAKURA_THEME =
      new Theme(
          "sakura",
          new Color(0.98, 0.94, 0.96),
          new Color(1.0, 0.98, 0.99),
          new Color(0.92, 0.48, 0.6),
          new Color(0.98, 0.8, 0.86),
          6,
          48);

  public static final Theme SAKURA_STORM_THEME =
      new Theme(
          "sakura-storm",
          new Color(0.05, 0.02, 0.08),
          new Color(0.02, 0.01, 0.04),
          new Color(0.95, 0.71, 0.87),
          new Color(0.98, 0.92, 0.97),
          8,
          36);

  public static final PlaybackSkin SAKURA_SKIN =
      new PlaybackSkin("sakura", SAKURA_THEME, 12.0, 1.0, 0.4, false, false);

  public static final PlaybackSkin SAKURA_REDUCED_MOTION_SKIN =
      new PlaybackSkin("sakura-reduced-motion", SAKURA_THEME, 6.0, 0.0, 0.25, true, false);

  public static final PlaybackSkin SAKURA_LOW_POWER_SKIN =
      new PlaybackSkin(
          "sakura-low-power",
          new Theme(
              "sakura-low-power",
              SAKURA_THEME.backgroundStart,
              SAKURA_THEME.backgroundEnd,
              SAKURA_THEME.accent,
              SAKURA_THEME.petal,
              4,
              72),
          8.0,
          0.4,
          0.3,
          false,
          true);

  public static final PlaybackSkin SAKURA_STORM_SKIN =
      new PlaybackSkin("sakura-storm", SAKURA_STORM_THEME, 12.0, 0.6, 0.34, false, false);

  private static byte[] xorParity(
      final byte[] payload,
      final int chunkSize,
      final int dataChunks,
      final int groupIndex,
      final int groupSize) {
    final byte[] parity = new byte[chunkSize];
    final int startIndex = groupIndex * groupSize;
    final int endIndex = Math.min(dataChunks, startIndex + groupSize);
    if (startIndex >= endIndex) {
      return parity;
    }
    for (int chunkIndex = startIndex; chunkIndex < endIndex; chunkIndex++) {
      final int start = chunkIndex * chunkSize;
      final int end = Math.min(payload.length, start + chunkSize);
      for (int offset = start; offset < end; offset++) {
        parity[offset - start] ^= payload[offset];
      }
    }
    return parity;
  }

  private static long crc32(final byte[] bytes, final int offset, final int length) {
    final CRC32 crc = new CRC32();
    crc.update(bytes, offset, length);
    return crc.getValue();
  }

  private static void writeUInt16LE(final byte[] bytes, final int offset, final int value) {
    bytes[offset] = (byte) (value & 0xFF);
    bytes[offset + 1] = (byte) ((value >> 8) & 0xFF);
  }

  private static void writeUInt32LE(final byte[] bytes, final int offset, final long value) {
    bytes[offset] = (byte) (value & 0xFF);
    bytes[offset + 1] = (byte) ((value >> 8) & 0xFF);
    bytes[offset + 2] = (byte) ((value >> 16) & 0xFF);
    bytes[offset + 3] = (byte) ((value >> 24) & 0xFF);
  }

  private static int readUInt16LE(final byte[] bytes, final int offset) {
    return (bytes[offset] & 0xFF) | ((bytes[offset + 1] & 0xFF) << 8);
  }

  private static long readUInt32LE(final byte[] bytes, final int offset) {
    return ((long) bytes[offset] & 0xFF)
        | (((long) bytes[offset + 1] & 0xFF) << 8)
        | (((long) bytes[offset + 2] & 0xFF) << 16)
        | (((long) bytes[offset + 3] & 0xFF) << 24);
  }

  private static int countNonNull(final byte[][] chunks) {
    int count = 0;
    for (byte[] chunk : chunks) {
      if (chunk != null) {
        count++;
      }
    }
    return count;
  }
}

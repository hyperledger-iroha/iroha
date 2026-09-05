package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Objects;

/** One immutable, CRC32C-protected IRQR frame. */
public final class IrohaPeerQRFrameV1 {
  public static final int VERSION = 1;
  public static final int PAYLOAD_OFFSET = 32;
  public static final int FIXED_OVERHEAD = 36;
  private static final int MAXIMUM_DATA_SHARDS =
      (IrohaPeerWireMessageV1.MAXIMUM_KAGEMUSHA_ENCODED_BYTES + 255) / 256;
  private static final byte[] MAGIC = "IRQR".getBytes(StandardCharsets.US_ASCII);

  public enum FrameKind {
    COMPLETE(0), HEADER(1), DATA(2), PARITY(3);

    private final int code;

    FrameKind(final int code) {
      this.code = code;
    }

    public int code() {
      return code;
    }

    static FrameKind fromCode(final int code) {
      for (final FrameKind value : values()) if (value.code == code) return value;
      return null;
    }
  }

  private final FrameKind frameKind;
  private final IrohaPeerPayloadProfile profile;
  private final IrohaPeerPayloadKind payloadKind;
  private final byte[] streamId;
  private final int index;
  private final int total;
  private final byte[] payload;

  public IrohaPeerQRFrameV1(
      final FrameKind frameKind,
      final IrohaPeerPayloadProfile profile,
      final IrohaPeerPayloadKind payloadKind,
      final byte[] streamId,
      final int index,
      final int total,
      final byte[] payload) {
    this.frameKind = Objects.requireNonNull(frameKind, "frameKind");
    this.profile = Objects.requireNonNull(profile, "profile");
    this.payloadKind = Objects.requireNonNull(payloadKind, "payloadKind");
    this.streamId = Objects.requireNonNull(streamId, "streamId").clone();
    this.payload = Objects.requireNonNull(payload, "payload").clone();
    require(this.streamId.length == 16, "Malformed IRQR stream identifier");
    require(total >= 1 && total <= MAXIMUM_DATA_SHARDS && index >= 0 && index <= 0xffff,
        "Malformed IRQR frame index");
    require(this.payload.length <= 0xffff, "Malformed IRQR payload");
    switch (frameKind) {
      case COMPLETE -> require(index == 0 && total == 1
              && this.payload.length > IrohaPeerWireMessageV1.HEADER_LENGTH
              && this.payload.length <= IrohaPeerWireMessageV1.HEADER_LENGTH
                  + IrohaPeerWireMessageV1.MAXIMUM_KAGEMUSHA_ENCODED_BYTES,
          "Malformed complete IRQR frame");
      case HEADER -> require(index == 0
              && this.payload.length == IrohaPeerWireMessageV1.HEADER_LENGTH,
          "Malformed header IRQR frame");
      case DATA -> require(index < total && this.payload.length == IrohaPeerQRCodecV1.SHARD_BYTES,
          "Malformed data IRQR frame");
      case PARITY -> require(index < (total + 1) / 2
              && this.payload.length == IrohaPeerQRCodecV1.SHARD_BYTES,
          "Malformed parity IRQR frame");
    }
    this.index = index;
    this.total = total;
  }

  public FrameKind frameKind() {
    return frameKind;
  }

  public IrohaPeerPayloadProfile profile() {
    return profile;
  }

  public IrohaPeerPayloadKind payloadKind() {
    return payloadKind;
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
    final int payloadEnd = PAYLOAD_OFFSET + payload.length;
    final byte[] out = new byte[payloadEnd + 4];
    System.arraycopy(MAGIC, 0, out, 0, MAGIC.length);
    out[4] = VERSION;
    out[5] = (byte) frameKind.code;
    IrohaPeerWireMessageV1.writeU16(out, 6, profile.code());
    out[8] = (byte) payloadKind.code();
    out[9] = 0;
    System.arraycopy(streamId, 0, out, 10, streamId.length);
    IrohaPeerWireMessageV1.writeU16(out, 26, index);
    IrohaPeerWireMessageV1.writeU16(out, 28, total);
    IrohaPeerWireMessageV1.writeU16(out, 30, payload.length);
    System.arraycopy(payload, 0, out, PAYLOAD_OFFSET, payload.length);
    IrohaPeerWireMessageV1.writeU32(out, payloadEnd, crc32c(out, 0, payloadEnd));
    return out;
  }

  public static IrohaPeerQRFrameV1 decode(final byte[] data) {
    Objects.requireNonNull(data, "data");
    require(data.length >= FIXED_OVERHEAD && rangeEquals(data, 0, MAGIC)
            && (data[4] & 0xff) == VERSION,
        "Malformed IRQR frame");
    final FrameKind frameKind = FrameKind.fromCode(data[5] & 0xff);
    final IrohaPeerPayloadProfile profile =
        IrohaPeerPayloadProfile.fromCode(IrohaPeerWireMessageV1.readU16(data, 6));
    final IrohaPeerPayloadKind payloadKind = IrohaPeerPayloadKind.fromCode(data[8] & 0xff);
    require(frameKind != null && profile != null && payloadKind != null && data[9] == 0,
        "Malformed IRQR metadata");
    final int payloadLength = IrohaPeerWireMessageV1.readU16(data, 30);
    final int payloadEnd = PAYLOAD_OFFSET + payloadLength;
    require(payloadEnd + 4 == data.length, "Malformed IRQR frame length");
    require(IrohaPeerWireMessageV1.readU32(data, payloadEnd) == crc32c(data, 0, payloadEnd),
        "IRQR checksum mismatch");
    return new IrohaPeerQRFrameV1(
        frameKind,
        profile,
        payloadKind,
        Arrays.copyOfRange(data, 10, 26),
        IrohaPeerWireMessageV1.readU16(data, 26),
        IrohaPeerWireMessageV1.readU16(data, 28),
        Arrays.copyOfRange(data, PAYLOAD_OFFSET, payloadEnd));
  }

  @Override
  public boolean equals(final Object other) {
    return other instanceof IrohaPeerQRFrameV1 that
        && frameKind == that.frameKind
        && profile == that.profile
        && payloadKind == that.payloadKind
        && index == that.index
        && total == that.total
        && Arrays.equals(streamId, that.streamId)
        && Arrays.equals(payload, that.payload);
  }

  @Override
  public int hashCode() {
    return 31 * frameKind.hashCode() + Arrays.hashCode(payload);
  }

  static long crc32c(final byte[] value, final int start, final int endExclusive) {
    int crc = -1;
    for (int index = start; index < endExclusive; index++) {
      crc ^= value[index] & 0xff;
      for (int bit = 0; bit < 8; bit++) {
        crc = (crc & 1) == 0 ? crc >>> 1 : (crc >>> 1) ^ 0x82f63b78;
      }
    }
    return ((long) (crc ^ -1)) & 0xffff_ffffL;
  }

  private static boolean rangeEquals(final byte[] value, final int offset, final byte[] expected) {
    if (offset < 0 || offset + expected.length > value.length) return false;
    for (int index = 0; index < expected.length; index++) {
      if (value[offset + index] != expected[index]) return false;
    }
    return true;
  }

  private static void require(final boolean condition, final String message) {
    if (!condition) throw new IllegalArgumentException(message);
  }
}

package org.hyperledger.iroha.android.numeric;

import java.math.BigInteger;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.util.Arrays;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import org.hyperledger.iroha.android.crypto.Blake2b;

/** Lossless values and strict wire codecs for Kotodama V1 exact numerics. */
public final class NumericV1 {
  /** Stable strict-decoder failure category. */
  public enum ErrorCode {
    MANTISSA_OVERFLOW,
    NONCANONICAL_MANTISSA,
    INVALID_SCALE,
    NONCANONICAL_DECIMAL,
    NEGATIVE_QUANTITY,
    INVALID_TEXT,
    FRAME_TOO_SHORT,
    FRAME_TOO_LARGE,
    INVALID_HEADER,
    SCHEMA_MISMATCH,
    COMPRESSION_NOT_ALLOWED,
    LAYOUT_FLAGS_NOT_ALLOWED,
    LENGTH_MISMATCH,
    CHECKSUM_MISMATCH,
    TRUNCATED_ENVELOPE,
    UNKNOWN_TYPE,
    TYPE_NOT_ALLOWED,
    WRONG_TYPE,
    INVALID_ENVELOPE_VERSION,
    OVERSIZED_LENGTH,
    PAYLOAD_HASH_MISMATCH
  }

  /** Strict Kotodama V1 numeric validation failure. */
  public static final class NumericException extends IllegalArgumentException {
    private final ErrorCode code;

    NumericException(final ErrorCode code, final String message) {
      super(message);
      this.code = code;
    }

    /** Return the stable failure category. */
    public ErrorCode code() {
      return code;
    }
  }

  /** Lossless signed 4,096-bit integer. */
  public static final class IntValue {
    private final BigInteger value;

    private IntValue(final BigInteger value) {
      this.value = checkedMantissa(value);
    }

    /** Construct from an arbitrary-precision integer. */
    public static IntValue of(final BigInteger value) {
      return new IntValue(requireNonNull(value, "value"));
    }

    /** Parse a canonical base-10 integer string. */
    public static IntValue parse(final String value) {
      requireNonNull(value, "value");
      if (!CANONICAL_INTEGER.matcher(value).matches() || "-0".equals(value)) {
        fail(ErrorCode.INVALID_TEXT, "int must use canonical base-10 syntax");
      }
      return of(new BigInteger(value));
    }

    /** Return the exact arbitrary-precision value. */
    public BigInteger value() {
      return value;
    }

    @Override
    public String toString() {
      return value.toString();
    }

    @Override
    public boolean equals(final Object other) {
      return other instanceof IntValue && value.equals(((IntValue) other).value);
    }

    @Override
    public int hashCode() {
      return value.hashCode();
    }
  }

  /** Lossless exact decimal with canonical scale. */
  public static final class DecimalValue {
    private final BigInteger mantissa;
    private final int scale;

    private DecimalValue(final BigInteger mantissa, final int scale) {
      this.mantissa = mantissa;
      this.scale = scale;
    }

    /** Construct and canonicalize a mantissa/scale pair. */
    public static DecimalValue of(final BigInteger mantissa, final int scale) {
      final Scaled normalized = normalizeScaled(requireNonNull(mantissa, "mantissa"), scale, false);
      return new DecimalValue(normalized.mantissa, normalized.scale);
    }

    /** Parse and canonicalize an exact decimal string. */
    public static DecimalValue parse(final String value) {
      final Scaled normalized = parseScaled(value, false);
      return new DecimalValue(normalized.mantissa, normalized.scale);
    }

    /** Return the exact signed mantissa. */
    public BigInteger mantissa() {
      return mantissa;
    }

    /** Return the canonical decimal scale. */
    public int scale() {
      return scale;
    }

    @Override
    public String toString() {
      return scaledText(mantissa, scale);
    }

    @Override
    public boolean equals(final Object other) {
      if (!(other instanceof DecimalValue)) return false;
      final DecimalValue value = (DecimalValue) other;
      return scale == value.scale && mantissa.equals(value.mantissa);
    }

    @Override
    public int hashCode() {
      return 31 * mantissa.hashCode() + scale;
    }
  }

  /** Lossless nominal non-negative asset quantity. */
  public static final class QuantityValue {
    private final BigInteger mantissa;
    private final int scale;

    private QuantityValue(final BigInteger mantissa, final int scale) {
      this.mantissa = mantissa;
      this.scale = scale;
    }

    /** Construct and canonicalize a non-negative mantissa/scale pair. */
    public static QuantityValue of(final BigInteger mantissa, final int scale) {
      final Scaled normalized = normalizeScaled(requireNonNull(mantissa, "mantissa"), scale, true);
      return new QuantityValue(normalized.mantissa, normalized.scale);
    }

    /** Parse and canonicalize an exact non-negative quantity string. */
    public static QuantityValue parse(final String value) {
      final Scaled normalized = parseScaled(value, true);
      return new QuantityValue(normalized.mantissa, normalized.scale);
    }

    /** Return the exact non-negative mantissa. */
    public BigInteger mantissa() {
      return mantissa;
    }

    /** Return the canonical decimal scale. */
    public int scale() {
      return scale;
    }

    @Override
    public String toString() {
      return scaledText(mantissa, scale);
    }

    @Override
    public boolean equals(final Object other) {
      if (!(other instanceof QuantityValue)) return false;
      final QuantityValue value = (QuantityValue) other;
      return scale == value.scale && mantissa.equals(value.mantissa);
    }

    @Override
    public int hashCode() {
      return 31 * mantissa.hashCode() + scale;
    }
  }

  /** Minimum signed V1 integer. */
  public static final BigInteger INT_MIN = BigInteger.ONE.shiftLeft(4095).negate();
  /** Maximum signed V1 integer. */
  public static final BigInteger INT_MAX = BigInteger.ONE.shiftLeft(4095).subtract(BigInteger.ONE);
  /** Maximum canonical decimal scale. */
  public static final int MAX_SCALE = 28;

  private NumericV1() {}

  /** Encode a canonical integer Norito frame. */
  public static byte[] encodeIntFrame(final IntValue value) {
    return encodeFrame(Kind.INT, requireNonNull(value, "value").value, 0);
  }

  /** Encode a canonical decimal Norito frame. */
  public static byte[] encodeDecimalFrame(final DecimalValue value) {
    requireNonNull(value, "value");
    return encodeFrame(Kind.DECIMAL, value.mantissa, value.scale);
  }

  /** Encode a canonical quantity Norito frame. */
  public static byte[] encodeQuantityFrame(final QuantityValue value) {
    requireNonNull(value, "value");
    return encodeFrame(Kind.QUANTITY, value.mantissa, value.scale);
  }

  /** Strictly decode an integer Norito frame. */
  public static IntValue decodeIntFrame(final byte[] frame) {
    return IntValue.of(decodeFrame(Kind.INT, frame).mantissa);
  }

  /** Strictly decode a decimal Norito frame. */
  public static DecimalValue decodeDecimalFrame(final byte[] frame) {
    final Scaled value = decodeFrame(Kind.DECIMAL, frame);
    return DecimalValue.of(value.mantissa, value.scale);
  }

  /** Strictly decode a quantity Norito frame. */
  public static QuantityValue decodeQuantityFrame(final byte[] frame) {
    final Scaled value = decodeFrame(Kind.QUANTITY, frame);
    return QuantityValue.of(value.mantissa, value.scale);
  }

  /** Encode an integer pointer envelope. */
  public static byte[] encodeIntEnvelope(final IntValue value) {
    return encodeEnvelope(Kind.INT, encodeIntFrame(value));
  }

  /** Encode a decimal pointer envelope. */
  public static byte[] encodeDecimalEnvelope(final DecimalValue value) {
    return encodeEnvelope(Kind.DECIMAL, encodeDecimalFrame(value));
  }

  /** Encode a quantity pointer envelope. */
  public static byte[] encodeQuantityEnvelope(final QuantityValue value) {
    return encodeEnvelope(Kind.QUANTITY, encodeQuantityFrame(value));
  }

  /** Strictly decode an integer pointer envelope. */
  public static IntValue decodeIntEnvelope(final byte[] envelope) {
    return decodeIntFrame(decodeEnvelope(Kind.INT, envelope));
  }

  /** Strictly decode a decimal pointer envelope. */
  public static DecimalValue decodeDecimalEnvelope(final byte[] envelope) {
    return decodeDecimalFrame(decodeEnvelope(Kind.DECIMAL, envelope));
  }

  /** Strictly decode a quantity pointer envelope. */
  public static QuantityValue decodeQuantityEnvelope(final byte[] envelope) {
    return decodeQuantityFrame(decodeEnvelope(Kind.QUANTITY, envelope));
  }

  private static byte[] encodeFrame(
      final Kind kind, final BigInteger mantissa, final int scale) {
    final byte[] twos = encodeTwos(mantissa);
    final ByteBuffer bodyBuffer = ByteBuffer.allocate(4 + twos.length + (kind.scaled ? 1 : 0))
        .order(ByteOrder.LITTLE_ENDIAN)
        .putInt(twos.length)
        .put(twos);
    if (kind.scaled) bodyBuffer.put((byte) scale);
    final byte[] body = bodyBuffer.array();
    return ByteBuffer.allocate(FRAME_HEADER_BYTES + body.length)
        .order(ByteOrder.LITTLE_ENDIAN)
        .put(MAGIC)
        .put((byte) 0)
        .put((byte) 0)
        .put(kind.schemaHash)
        .put((byte) 0)
        .putLong(body.length)
        .putLong(crc64(body))
        .put((byte) 0)
        .put(body)
        .array();
  }

  private static Scaled decodeFrame(final Kind kind, final byte[] untrustedFrame) {
    final byte[] frame = requireNonNull(untrustedFrame, "frame");
    final int maximum = FRAME_HEADER_BYTES + 4 + MAX_MANTISSA_BYTES + (kind.scaled ? 1 : 0);
    if (frame.length < FRAME_HEADER_BYTES) fail(ErrorCode.FRAME_TOO_SHORT, "frame is truncated");
    if (frame.length > maximum) fail(ErrorCode.FRAME_TOO_LARGE, "frame is oversized");
    if (!Arrays.equals(Arrays.copyOfRange(frame, 0, 4), MAGIC) || frame[4] != 0 || frame[5] != 0) {
      fail(ErrorCode.INVALID_HEADER, "frame has the wrong magic or version");
    }
    if (!Arrays.equals(Arrays.copyOfRange(frame, 6, 22), kind.schemaHash)) {
      fail(ErrorCode.SCHEMA_MISMATCH, "frame schema does not match");
    }
    if (frame[22] != 0) fail(ErrorCode.COMPRESSION_NOT_ALLOWED, "compression is forbidden");
    if (frame[39] != 0) fail(ErrorCode.LAYOUT_FLAGS_NOT_ALLOWED, "layout flags must be zero");
    final ByteBuffer header = ByteBuffer.wrap(frame).order(ByteOrder.LITTLE_ENDIAN);
    final long bodyLength = header.getLong(23);
    if (bodyLength < 0 || bodyLength != frame.length - FRAME_HEADER_BYTES) {
      fail(ErrorCode.LENGTH_MISMATCH, "frame length is inconsistent");
    }
    final byte[] body = Arrays.copyOfRange(frame, FRAME_HEADER_BYTES, frame.length);
    if (header.getLong(31) != crc64(body)) fail(ErrorCode.CHECKSUM_MISMATCH, "frame checksum failed");
    if (body.length < 4) fail(ErrorCode.LENGTH_MISMATCH, "body has no mantissa length");
    final int mantissaLength = ByteBuffer.wrap(body).order(ByteOrder.LITTLE_ENDIAN).getInt();
    if (mantissaLength < 0 || mantissaLength > MAX_MANTISSA_BYTES) {
      fail(ErrorCode.MANTISSA_OVERFLOW, "mantissa length exceeds 512 bytes");
    }
    final int expected = 4 + mantissaLength + (kind.scaled ? 1 : 0);
    if (expected != body.length) fail(ErrorCode.LENGTH_MISMATCH, "body length is inconsistent");
    final BigInteger mantissa = decodeTwos(Arrays.copyOfRange(body, 4, 4 + mantissaLength));
    if (!kind.scaled) return new Scaled(mantissa, 0);
    final int scale = body[body.length - 1] & 0xFF;
    if (scale > MAX_SCALE) fail(ErrorCode.INVALID_SCALE, "scale exceeds 28");
    if ((mantissa.signum() == 0 && scale != 0)
        || (scale > 0 && mantissa.remainder(BigInteger.TEN).signum() == 0)) {
      fail(ErrorCode.NONCANONICAL_DECIMAL, "scaled value is not canonical");
    }
    if (kind == Kind.QUANTITY && mantissa.signum() < 0) {
      fail(ErrorCode.NEGATIVE_QUANTITY, "quantity cannot be negative");
    }
    return new Scaled(mantissa, scale);
  }

  private static byte[] encodeEnvelope(final Kind kind, final byte[] frame) {
    return ByteBuffer.allocate(ENVELOPE_HEADER_BYTES + frame.length + HASH_BYTES)
        .order(ByteOrder.BIG_ENDIAN)
        .putShort((short) kind.pointerType)
        .put((byte) 1)
        .putInt(frame.length)
        .put(frame)
        .put(Blake2b.digest256(frame))
        .array();
  }

  private static byte[] decodeEnvelope(final Kind kind, final byte[] untrustedEnvelope) {
    final byte[] envelope = requireNonNull(untrustedEnvelope, "envelope");
    if (envelope.length < ENVELOPE_HEADER_BYTES) {
      fail(ErrorCode.TRUNCATED_ENVELOPE, "envelope is truncated");
    }
    final ByteBuffer header = ByteBuffer.wrap(envelope).order(ByteOrder.BIG_ENDIAN);
    final int pointerType = header.getShort() & 0xFFFF;
    if (pointerType < 0x0010 || pointerType > 0x0013) fail(ErrorCode.UNKNOWN_TYPE, "unknown pointer type");
    if (pointerType == 0x0010) fail(ErrorCode.TYPE_NOT_ALLOWED, "retired Amount type is forbidden");
    if (pointerType != kind.pointerType) fail(ErrorCode.WRONG_TYPE, "pointer type does not match");
    if ((header.get() & 0xFF) != 1) fail(ErrorCode.INVALID_ENVELOPE_VERSION, "version must be 1");
    final int frameLength = header.getInt();
    final int maximum = FRAME_HEADER_BYTES + 4 + MAX_MANTISSA_BYTES + (kind.scaled ? 1 : 0);
    if (frameLength < 0 || frameLength > maximum) {
      fail(ErrorCode.OVERSIZED_LENGTH, "declared frame is oversized");
    }
    if (ENVELOPE_HEADER_BYTES + frameLength + HASH_BYTES != envelope.length) {
      fail(ErrorCode.TRUNCATED_ENVELOPE, "envelope length is inconsistent");
    }
    final byte[] frame = Arrays.copyOfRange(envelope, ENVELOPE_HEADER_BYTES, ENVELOPE_HEADER_BYTES + frameLength);
    final byte[] suppliedHash = Arrays.copyOfRange(envelope, ENVELOPE_HEADER_BYTES + frameLength, envelope.length);
    if (!constantTimeEquals(Blake2b.digest256(frame), suppliedHash)) {
      fail(ErrorCode.PAYLOAD_HASH_MISMATCH, "payload hash failed");
    }
    return frame;
  }

  private static byte[] encodeTwos(final BigInteger value) {
    checkedMantissa(value);
    if (value.signum() == 0) return new byte[0];
    final byte[] bigEndian = value.toByteArray();
    final byte[] littleEndian = reverse(bigEndian);
    if (littleEndian.length > MAX_MANTISSA_BYTES) fail(ErrorCode.MANTISSA_OVERFLOW, "mantissa is too wide");
    return littleEndian;
  }

  private static BigInteger decodeTwos(final byte[] bytes) {
    if (bytes.length > MAX_MANTISSA_BYTES) fail(ErrorCode.MANTISSA_OVERFLOW, "mantissa is too wide");
    if (bytes.length == 0) return BigInteger.ZERO;
    final int last = bytes[bytes.length - 1] & 0xFF;
    if (bytes.length == 1 && last == 0) {
      fail(ErrorCode.NONCANONICAL_MANTISSA, "zero must use an empty mantissa");
    }
    if (bytes.length > 1) {
      final int previous = bytes[bytes.length - 2] & 0xFF;
      if ((last == 0 && (previous & 0x80) == 0) || (last == 0xFF && (previous & 0x80) != 0)) {
        fail(ErrorCode.NONCANONICAL_MANTISSA, "mantissa has redundant sign extension");
      }
    }
    return checkedMantissa(new BigInteger(reverse(bytes)));
  }

  private static Scaled parseScaled(final String raw, final boolean quantity) {
    final String value = requireNonNull(raw, "value");
    final Matcher match = EXACT_DECIMAL.matcher(value);
    if (!match.matches() || "-0".equals(value)) {
      fail(ErrorCode.INVALID_TEXT, "value must use exact decimal syntax");
    }
    final String fraction = match.group(3) == null ? "" : match.group(3);
    if (fraction.length() > MAX_SCALE) fail(ErrorCode.INVALID_SCALE, "scale exceeds 28");
    return normalizeScaled(new BigInteger(match.group(1) + match.group(2) + fraction), fraction.length(), quantity);
  }

  private static Scaled normalizeScaled(
      final BigInteger rawMantissa, final int rawScale, final boolean quantity) {
    if (rawScale < 0 || rawScale > MAX_SCALE) fail(ErrorCode.INVALID_SCALE, "scale must be in 0..28");
    BigInteger mantissa = checkedMantissa(rawMantissa);
    int scale = rawScale;
    if (mantissa.signum() == 0) {
      scale = 0;
    } else {
      while (scale > 0 && mantissa.remainder(BigInteger.TEN).signum() == 0) {
        mantissa = mantissa.divide(BigInteger.TEN);
        scale--;
      }
    }
    if (quantity && mantissa.signum() < 0) fail(ErrorCode.NEGATIVE_QUANTITY, "quantity cannot be negative");
    return new Scaled(mantissa, scale);
  }

  private static String scaledText(final BigInteger mantissa, final int scale) {
    if (scale == 0) return mantissa.toString();
    String digits = mantissa.abs().toString();
    final StringBuilder padded = new StringBuilder();
    for (int index = digits.length(); index <= scale; index++) padded.append('0');
    padded.append(digits);
    digits = padded.toString();
    final int split = digits.length() - scale;
    return (mantissa.signum() < 0 ? "-" : "") + digits.substring(0, split) + "." + digits.substring(split);
  }

  private static BigInteger checkedMantissa(final BigInteger value) {
    if (value.compareTo(INT_MIN) < 0 || value.compareTo(INT_MAX) > 0) {
      fail(ErrorCode.MANTISSA_OVERFLOW, "mantissa is outside the signed 4096-bit domain");
    }
    return value;
  }

  private static byte[] reverse(final byte[] value) {
    final byte[] result = new byte[value.length];
    for (int index = 0; index < value.length; index++) result[index] = value[value.length - 1 - index];
    return result;
  }

  private static boolean constantTimeEquals(final byte[] left, final byte[] right) {
    if (left.length != right.length) return false;
    int difference = 0;
    for (int index = 0; index < left.length; index++) difference |= left[index] ^ right[index];
    return difference == 0;
  }

  private static long crc64(final byte[] payload) {
    long crc = -1L;
    for (final byte value : payload) crc = CRC64_TABLE[((int) crc ^ value) & 0xFF] ^ (crc >>> 8);
    return crc ^ -1L;
  }

  private static long[] buildCrc64Table() {
    final long[] table = new long[256];
    for (int index = 0; index < table.length; index++) {
      long crc = index;
      for (int bit = 0; bit < 8; bit++) crc = (crc & 1L) == 0 ? crc >>> 1 : (crc >>> 1) ^ CRC64_POLY;
      table[index] = crc;
    }
    return table;
  }

  private static byte[] hex(final String value) {
    final byte[] result = new byte[value.length() / 2];
    for (int index = 0; index < result.length; index++) {
      result[index] = (byte) Integer.parseInt(value.substring(index * 2, index * 2 + 2), 16);
    }
    return result;
  }

  private static <T> T requireNonNull(final T value, final String name) {
    if (value == null) throw new NullPointerException(name + " must not be null");
    return value;
  }

  private static void fail(final ErrorCode code, final String message) {
    throw new NumericException(code, message);
  }

  private enum Kind {
    INT("07c039457363b9e1d36bbd31d93dec4a", 0x0011, false),
    DECIMAL("ba2ffed52e4d8ee16f17efefe1828524", 0x0012, true),
    QUANTITY("e4769984c81ce0e8b678f2eb06274ee3", 0x0013, true);

    final byte[] schemaHash;
    final int pointerType;
    final boolean scaled;

    Kind(final String schemaHash, final int pointerType, final boolean scaled) {
      this.schemaHash = hex(schemaHash);
      this.pointerType = pointerType;
      this.scaled = scaled;
    }
  }

  private static final class Scaled {
    final BigInteger mantissa;
    final int scale;

    Scaled(final BigInteger mantissa, final int scale) {
      this.mantissa = mantissa;
      this.scale = scale;
    }
  }

  private static final Pattern CANONICAL_INTEGER = Pattern.compile("-?(?:0|[1-9][0-9]*)");
  private static final Pattern EXACT_DECIMAL = Pattern.compile("(-?)(0|[1-9][0-9]*)(?:\\.([0-9]+))?");
  private static final int MAX_MANTISSA_BYTES = 512;
  private static final int FRAME_HEADER_BYTES = 40;
  private static final int ENVELOPE_HEADER_BYTES = 7;
  private static final int HASH_BYTES = 32;
  private static final long CRC64_POLY = 0xC96C5795D7870F42L;
  private static final long[] CRC64_TABLE = buildCrc64Table();
  private static final byte[] MAGIC = {0x4E, 0x52, 0x54, 0x30};
}

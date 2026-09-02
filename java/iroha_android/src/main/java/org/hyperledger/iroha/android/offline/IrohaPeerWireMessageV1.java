package org.hyperledger.iroha.android.offline;

import java.io.ByteArrayOutputStream;
import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import java.util.Objects;
import java.util.zip.DataFormatException;
import java.util.zip.Deflater;
import java.util.zip.Inflater;
import org.hyperledger.iroha.android.crypto.Blake2b;

/** Immutable and fully verified transport-neutral IPM1 message. */
public final class IrohaPeerWireMessageV1 {
  public static final int VERSION = 1;
  public static final int HEADER_LENGTH = 84;
  public static final int MAXIMUM_CANONICAL_BYTES =
      IrohaPeerWireLimitsV1.PEER_V1.maximumCanonicalBytes();
  public static final int MAXIMUM_OFFLINE_CASH_ENCODED_BYTES =
      IrohaPeerWireLimitsV1.PEER_V1.maximumOfflineCashEncodedBytes();

  private static final byte[] MAGIC = "IPM1".getBytes(StandardCharsets.US_ASCII);
  private static final byte[] CANONICAL_DOMAIN =
      "IROHA-PEER-PAYLOAD-V1\0".getBytes(StandardCharsets.UTF_8);
  private static final byte[] MESSAGE_DOMAIN =
      "IROHA-PEER-MESSAGE-V1\0".getBytes(StandardCharsets.UTF_8);

  private final IrohaPeerCanonicalPayload canonicalPayload;
  private final IrohaPeerContentEncodingV1 encoding;
  private final byte[] canonicalHash;
  private final byte[] wireHash;
  private final byte[] encodedBody;

  public IrohaPeerWireMessageV1(final IrohaPeerCanonicalPayload canonicalPayload) {
    this(canonicalPayload, IrohaPeerWireCompressionPolicyV1.DISABLED,
        IrohaPeerWireLimitsV1.PEER_V1);
  }

  public IrohaPeerWireMessageV1(
      final IrohaPeerCanonicalPayload canonicalPayload,
      final IrohaPeerWireCompressionPolicyV1 compressionPolicy) {
    this(canonicalPayload, compressionPolicy, IrohaPeerWireLimitsV1.PEER_V1);
  }

  public IrohaPeerWireMessageV1(
      final IrohaPeerCanonicalPayload canonicalPayload,
      final IrohaPeerWireCompressionPolicyV1 compressionPolicy,
      final IrohaPeerWireLimitsV1 limits) {
    this(encodedParts(canonicalPayload, compressionPolicy, limits));
  }

  private IrohaPeerWireMessageV1(final EncodedParts parts) {
    this(parts.payload, parts.encoding, parts.canonicalHash, parts.wireHash, parts.body);
  }

  private IrohaPeerWireMessageV1(
      final IrohaPeerCanonicalPayload canonicalPayload,
      final IrohaPeerContentEncodingV1 encoding,
      final byte[] canonicalHash,
      final byte[] wireHash,
      final byte[] encodedBody) {
    this.canonicalPayload = canonicalPayload;
    this.encoding = encoding;
    this.canonicalHash = canonicalHash.clone();
    this.wireHash = wireHash.clone();
    this.encodedBody = encodedBody.clone();
  }

  public IrohaPeerCanonicalPayload canonicalPayload() {
    return canonicalPayload;
  }

  public IrohaPeerContentEncodingV1 encoding() {
    return encoding;
  }

  public byte[] canonicalHash() {
    return canonicalHash.clone();
  }

  public byte[] wireHash() {
    return wireHash.clone();
  }

  public byte[] encodedBody() {
    return encodedBody.clone();
  }

  public byte[] streamId() {
    return Arrays.copyOf(wireHash, 16);
  }

  public byte[] encode() {
    final byte[] out = new byte[HEADER_LENGTH + encodedBody.length];
    final byte[] prefix =
        headerPrefix(encoding, canonicalPayload, encodedBody.length, canonicalHash);
    System.arraycopy(prefix, 0, out, 0, prefix.length);
    System.arraycopy(wireHash, 0, out, 52, wireHash.length);
    System.arraycopy(encodedBody, 0, out, HEADER_LENGTH, encodedBody.length);
    return out;
  }

  public static IrohaPeerWireMessageV1 decode(final byte[] data) {
    return decode(data, null, null);
  }

  public static IrohaPeerWireMessageV1 decode(
      final byte[] data,
      final IrohaPeerPayloadProfile expectedProfile,
      final IrohaPeerPayloadKind expectedKind) {
    return decode(data, expectedProfile, expectedKind, IrohaPeerWireLimitsV1.PEER_V1);
  }

  public static IrohaPeerWireMessageV1 decode(
      final byte[] data,
      final IrohaPeerPayloadProfile expectedProfile,
      final IrohaPeerPayloadKind expectedKind,
      final IrohaPeerWireLimitsV1 limits) {
    Objects.requireNonNull(data, "data");
    Objects.requireNonNull(limits, "limits");
    require(data.length >= HEADER_LENGTH && rangeEquals(data, 0, MAGIC), "Malformed IPM1 message");
    require((data[4] & 0xff) == VERSION, "Unsupported peer message version");
    final IrohaPeerContentEncodingV1 encoding =
        IrohaPeerContentEncodingV1.fromCode(data[5] & 0xff);
    require(encoding != null, "Unsupported peer content encoding");
    final IrohaPeerPayloadProfile profile = IrohaPeerPayloadProfile.fromCode(readU16(data, 6));
    require(profile != null, "Invalid peer payload profile");
    final IrohaPeerPayloadKind kind = IrohaPeerPayloadKind.fromCode(data[8] & 0xff);
    require(kind != null, "Invalid peer payload kind");
    require(data[9] == 0 && readU16(data, 10) != 0, "Invalid peer message metadata");
    final int schemaVersion = readU16(data, 10);
    requireProfileSchema(profile, schemaVersion);
    final int canonicalLength = checkedLength(readU32(data, 12), limits.maximumCanonicalBytes());
    final int encodedLength =
        checkedLength(readU32(data, 16), limits.maximumOfflineCashEncodedBytes());
    require(data.length == HEADER_LENGTH + encodedLength, "Peer message length mismatch");
    require(encoding != IrohaPeerContentEncodingV1.ZLIB
            || canonicalZlibLength(canonicalLength, encodedLength),
        "Non-canonical zlib peer body");
    require(expectedProfile == null || expectedProfile == profile, "Unexpected peer payload profile");
    require(expectedKind == null || expectedKind == kind, "Unexpected peer payload kind");

    final byte[] canonicalDigest = Arrays.copyOfRange(data, 20, 52);
    final byte[] messageDigest = Arrays.copyOfRange(data, 52, 84);
    final byte[] body = Arrays.copyOfRange(data, 84, data.length);
    final byte[] computedWire =
        Blake2b.digest256(concat(MESSAGE_DOMAIN, Arrays.copyOfRange(data, 0, 52), body));
    require(Arrays.equals(computedWire, messageDigest), "Peer message wire hash mismatch");

    final byte[] canonicalBytes;
    if (encoding == IrohaPeerContentEncodingV1.NONE) {
      require(encodedLength == canonicalLength, "Peer message length mismatch");
      canonicalBytes = body.clone();
    } else {
      canonicalBytes = inflateBounded(body, canonicalLength);
    }
    final IrohaPeerCanonicalPayload payload =
        new IrohaPeerCanonicalPayload(profile, kind, schemaVersion, canonicalBytes);
    Arrays.fill(canonicalBytes, (byte) 0);
    require(Arrays.equals(canonicalHash(payload), canonicalDigest), "Peer canonical hash mismatch");
    final IrohaPeerWireMessageV1 message =
        new IrohaPeerWireMessageV1(payload, encoding, canonicalDigest, messageDigest, body);
    Arrays.fill(body, (byte) 0);
    return message;
  }

  static Header decodeHeader(final byte[] data) {
    return decodeHeader(data, IrohaPeerWireLimitsV1.PEER_V1);
  }

  static Header decodeHeader(final byte[] data, final IrohaPeerWireLimitsV1 limits) {
    Objects.requireNonNull(data, "data");
    Objects.requireNonNull(limits, "limits");
    require(data.length == HEADER_LENGTH && rangeEquals(data, 0, MAGIC), "Malformed IPM1 header");
    require((data[4] & 0xff) == VERSION, "Malformed IPM1 header");
    final IrohaPeerContentEncodingV1 encoding =
        IrohaPeerContentEncodingV1.fromCode(data[5] & 0xff);
    final IrohaPeerPayloadProfile profile = IrohaPeerPayloadProfile.fromCode(readU16(data, 6));
    final IrohaPeerPayloadKind kind = IrohaPeerPayloadKind.fromCode(data[8] & 0xff);
    require(encoding != null && profile != null && kind != null && data[9] == 0, "Malformed IPM1 header");
    final int schema = readU16(data, 10);
    require(schema != 0, "Malformed IPM1 header");
    requireProfileSchema(profile, schema);
    final int canonicalLength = checkedLength(readU32(data, 12), limits.maximumCanonicalBytes());
    final int encodedLength =
        checkedLength(readU32(data, 16), limits.maximumOfflineCashEncodedBytes());
    require(encoding != IrohaPeerContentEncodingV1.NONE || canonicalLength == encodedLength,
        "Malformed IPM1 header");
    require(encoding != IrohaPeerContentEncodingV1.ZLIB
            || canonicalZlibLength(canonicalLength, encodedLength),
        "Malformed IPM1 header");
    return new Header(
        encoding,
        profile,
        kind,
        schema,
        canonicalLength,
        encodedLength,
        Arrays.copyOfRange(data, 20, 52),
        Arrays.copyOfRange(data, 52, 84),
        data);
  }

  static final class Header {
    final IrohaPeerContentEncodingV1 encoding;
    final IrohaPeerPayloadProfile profile;
    final IrohaPeerPayloadKind kind;
    final int schemaVersion;
    final int canonicalLength;
    final int encodedLength;
    private final byte[] canonicalHash;
    private final byte[] wireHash;
    private final byte[] bytes;

    Header(
        final IrohaPeerContentEncodingV1 encoding,
        final IrohaPeerPayloadProfile profile,
        final IrohaPeerPayloadKind kind,
        final int schemaVersion,
        final int canonicalLength,
        final int encodedLength,
        final byte[] canonicalHash,
        final byte[] wireHash,
        final byte[] bytes) {
      this.encoding = encoding;
      this.profile = profile;
      this.kind = kind;
      this.schemaVersion = schemaVersion;
      this.canonicalLength = canonicalLength;
      this.encodedLength = encodedLength;
      this.canonicalHash = canonicalHash.clone();
      this.wireHash = wireHash.clone();
      this.bytes = bytes.clone();
    }

    byte[] streamId() {
      return Arrays.copyOf(wireHash, 16);
    }

    byte[] bytes() {
      return bytes.clone();
    }

    @Override
    public boolean equals(final Object other) {
      return other instanceof Header that
          && encoding == that.encoding
          && profile == that.profile
          && kind == that.kind
          && schemaVersion == that.schemaVersion
          && canonicalLength == that.canonicalLength
          && encodedLength == that.encodedLength
          && Arrays.equals(canonicalHash, that.canonicalHash)
          && Arrays.equals(wireHash, that.wireHash);
    }

    @Override
    public int hashCode() {
      return 31 * profile.hashCode() + Arrays.hashCode(wireHash);
    }
  }

  @Override
  public boolean equals(final Object other) {
    return other instanceof IrohaPeerWireMessageV1 that
        && canonicalPayload.equals(that.canonicalPayload)
        && encoding == that.encoding
        && Arrays.equals(canonicalHash, that.canonicalHash)
        && Arrays.equals(wireHash, that.wireHash)
        && Arrays.equals(encodedBody, that.encodedBody);
  }

  @Override
  public int hashCode() {
    return 31 * canonicalPayload.hashCode() + Arrays.hashCode(wireHash);
  }

  private static byte[] canonicalHash(final IrohaPeerCanonicalPayload payload) {
    final byte[] metadata = new byte[5];
    writeU16(metadata, 0, payload.profile().code());
    metadata[2] = (byte) payload.kind().code();
    writeU16(metadata, 3, payload.schemaVersion());
    final byte[] bytes = payload.bytes();
    try {
      return Blake2b.digest256(concat(CANONICAL_DOMAIN, metadata, bytes));
    } finally {
      Arrays.fill(bytes, (byte) 0);
    }
  }

  private static EncodedParts encodedParts(
      final IrohaPeerCanonicalPayload payload,
      final IrohaPeerWireCompressionPolicyV1 policy,
      final IrohaPeerWireLimitsV1 limits) {
    Objects.requireNonNull(payload, "canonicalPayload");
    Objects.requireNonNull(policy, "compressionPolicy");
    Objects.requireNonNull(limits, "limits");
    require(payload.byteCount() <= limits.maximumCanonicalBytes(),
        "Peer canonical payload exceeds its bound");
    final int maximumEncoded = limits.maximumOfflineCashEncodedBytes();
    final byte[] canonical = payload.bytes();
    final byte[] compressed = policy == IrohaPeerWireCompressionPolicyV1.PEER_OPTIMIZED
        ? deflateZlib(canonical) : null;
    final boolean useCompressed = compressed != null
        && canonical.length - compressed.length >= 32
        && shardCount(compressed.length) < shardCount(canonical.length)
        && compressed.length <= maximumEncoded;
    final IrohaPeerContentEncodingV1 selectedEncoding = useCompressed
        ? IrohaPeerContentEncodingV1.ZLIB : IrohaPeerContentEncodingV1.NONE;
    final byte[] body = useCompressed ? compressed : canonical.clone();
    if (!useCompressed && compressed != null) Arrays.fill(compressed, (byte) 0);
    Arrays.fill(canonical, (byte) 0);
    require(body.length <= maximumEncoded, "Peer encoded payload exceeds its profile bound");
    final byte[] digest = canonicalHash(payload);
    final byte[] prefix = headerPrefix(selectedEncoding, payload, body.length, digest);
    final byte[] messageHash = Blake2b.digest256(concat(MESSAGE_DOMAIN, prefix, body));
    return new EncodedParts(payload, selectedEncoding, digest, messageHash, body);
  }

  private static void requireProfileSchema(
      final IrohaPeerPayloadProfile profile, final int schemaVersion) {
    require(
        schemaVersion == profile.requiredSchemaVersion(),
        "Peer payload profile "
            + profile
            + " requires schema "
            + profile.requiredSchemaVersion()
            + ", received "
            + schemaVersion);
  }

  private static byte[] headerPrefix(
      final IrohaPeerContentEncodingV1 encoding,
      final IrohaPeerCanonicalPayload payload,
      final int encodedLength,
      final byte[] canonicalHash) {
    final byte[] out = new byte[52];
    System.arraycopy(MAGIC, 0, out, 0, MAGIC.length);
    out[4] = VERSION;
    out[5] = (byte) encoding.code();
    writeU16(out, 6, payload.profile().code());
    out[8] = (byte) payload.kind().code();
    out[9] = 0;
    writeU16(out, 10, payload.schemaVersion());
    writeU32(out, 12, payload.byteCount());
    writeU32(out, 16, encodedLength);
    System.arraycopy(canonicalHash, 0, out, 20, canonicalHash.length);
    return out;
  }

  private static byte[] inflateBounded(final byte[] encoded, final int expectedLength) {
    require(encoded.length >= 6 && encoded[0] == 0x78 && encoded[1] == (byte) 0x9c,
        "Invalid zlib peer body");
    final Inflater inflater = new Inflater(false);
    final ByteArrayOutputStream output = new ByteArrayOutputStream(expectedLength);
    final byte[] buffer = new byte[Math.min(16 * 1024, Math.max(1, expectedLength))];
    try {
      inflater.setInput(encoded);
      while (!inflater.finished()) {
        final int count;
        try {
          count = inflater.inflate(buffer);
        } catch (final DataFormatException failure) {
          throw new IllegalArgumentException("Invalid zlib peer body", failure);
        }
        if (count == 0) {
          require(!inflater.needsDictionary() && !inflater.needsInput(), "Invalid zlib peer body");
        } else {
          require(output.size() <= expectedLength - count, "Invalid zlib peer body");
          output.write(buffer, 0, count);
        }
      }
      require(inflater.getRemaining() == 0 && output.size() == expectedLength, "Invalid zlib peer body");
      return output.toByteArray();
    } finally {
      Arrays.fill(buffer, (byte) 0);
      inflater.end();
    }
  }

  private static byte[] deflateZlib(final byte[] canonical) {
    final Deflater deflater = new Deflater(Deflater.DEFAULT_COMPRESSION, false);
    final ByteArrayOutputStream output = new ByteArrayOutputStream(canonical.length);
    final byte[] buffer = new byte[16 * 1024];
    try {
      deflater.setInput(canonical);
      deflater.finish();
      while (!deflater.finished()) {
        final int count = deflater.deflate(buffer);
        require(count > 0, "Unable to encode zlib peer body");
        output.write(buffer, 0, count);
      }
      return output.toByteArray();
    } finally {
      Arrays.fill(buffer, (byte) 0);
      deflater.end();
    }
  }

  private static int checkedLength(final long value, final int maximum) {
    require(value >= 1 && value <= maximum, "Peer payload exceeds its bound");
    return (int) value;
  }

  private static boolean canonicalZlibLength(
      final int canonicalLength, final int encodedLength) {
    return canonicalLength > 0 && encodedLength > 0
        && canonicalLength - encodedLength >= 32
        && (encodedLength + 255) / 256 < (canonicalLength + 255) / 256;
  }

  private static int shardCount(final int byteCount) {
    return (byteCount + 255) / 256;
  }

  private static final class EncodedParts {
    final IrohaPeerCanonicalPayload payload;
    final IrohaPeerContentEncodingV1 encoding;
    final byte[] canonicalHash;
    final byte[] wireHash;
    final byte[] body;

    EncodedParts(
        final IrohaPeerCanonicalPayload payload,
        final IrohaPeerContentEncodingV1 encoding,
        final byte[] canonicalHash,
        final byte[] wireHash,
        final byte[] body) {
      this.payload = payload;
      this.encoding = encoding;
      this.canonicalHash = canonicalHash;
      this.wireHash = wireHash;
      this.body = body;
    }
  }

  private static byte[] concat(final byte[]... values) {
    int length = 0;
    for (final byte[] value : values) length = Math.addExact(length, value.length);
    final byte[] out = new byte[length];
    int offset = 0;
    for (final byte[] value : values) {
      System.arraycopy(value, 0, out, offset, value.length);
      offset += value.length;
    }
    return out;
  }

  private static boolean rangeEquals(final byte[] value, final int offset, final byte[] expected) {
    if (offset < 0 || offset + expected.length > value.length) return false;
    for (int index = 0; index < expected.length; index++) {
      if (value[offset + index] != expected[index]) return false;
    }
    return true;
  }

  static void writeU16(final byte[] out, final int offset, final int value) {
    out[offset] = (byte) (value >>> 8);
    out[offset + 1] = (byte) value;
  }

  static void writeU32(final byte[] out, final int offset, final long value) {
    out[offset] = (byte) (value >>> 24);
    out[offset + 1] = (byte) (value >>> 16);
    out[offset + 2] = (byte) (value >>> 8);
    out[offset + 3] = (byte) value;
  }

  static int readU16(final byte[] value, final int offset) {
    return ((value[offset] & 0xff) << 8) | (value[offset + 1] & 0xff);
  }

  static long readU32(final byte[] value, final int offset) {
    return ((long) (value[offset] & 0xff) << 24)
        | ((long) (value[offset + 1] & 0xff) << 16)
        | ((long) (value[offset + 2] & 0xff) << 8)
        | (long) (value[offset + 3] & 0xff);
  }

  static void require(final boolean condition, final String message) {
    if (!condition) throw new IllegalArgumentException(message);
  }
}

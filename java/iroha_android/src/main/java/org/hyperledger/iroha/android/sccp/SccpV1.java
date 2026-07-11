package org.hyperledger.iroha.android.sccp;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.List;
import org.bouncycastle.crypto.digests.KeccakDigest;
import org.hyperledger.iroha.android.address.AccountIdLiteral;
import org.hyperledger.iroha.android.crypto.Blake2b;

/** Consensus-compatible exact-lane hashing and fixed layouts for SCCP V1. */
public final class SccpV1 {
  /** Exact I105 discriminant used by the public SORA Taira SCCP endpoint. */
  public static final int TAIRA_I105_DISCRIMINANT_V1 = 369;

  private static final byte[] LANE_HASH_PREFIX =
      "sccp:lane-id:v1".getBytes(StandardCharsets.UTF_8);
  private static final byte[] MESSAGE_ID_PREFIX =
      "sccp:lane-message-id:v1".getBytes(StandardCharsets.UTF_8);
  private static final byte[] PAYLOAD_HASH_PREFIX =
      "sccp:payload:v1".getBytes(StandardCharsets.UTF_8);
  private static final byte[] LEAF_HASH_PREFIX =
      "sccp:hub:leaf:v1".getBytes(StandardCharsets.UTF_8);
  private static final byte[] SOURCE_EVENT_DIGEST_PREFIX =
      "sccp:source:event:v1".getBytes(StandardCharsets.UTF_8);

  private SccpV1() {}

  /** Canonical profile bytes independent of Java enum layout. */
  public static byte[] canonicalNetworkBytes(final SccpNetworkV1 network) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    out.write(network.tag());
    writeU32(out, network.domainId());
    switch (network) {
      case SORA_TAIRA -> write(out, decodeLowerHex("809574f5fee75e69bfcf52451e42d50f"));
      case ETHEREUM_MAINNET -> writeUnsignedLe(out, BigInteger.ONE, 8);
      case ETHEREUM_SEPOLIA -> writeUnsignedLe(out, BigInteger.valueOf(11_155_111L), 8);
      case BSC_MAINNET -> writeUnsignedLe(out, BigInteger.valueOf(56), 8);
      case BSC_TESTNET -> writeUnsignedLe(out, BigInteger.valueOf(97), 8);
      case TRON_MAINNET -> writeU32Bits(out, 0x2b6653dcL);
      case TRON_NILE -> writeU32Bits(out, 0xcd8690dcL);
      case TRON_SHASTA -> writeU32Bits(out, 0x94a9059eL);
    }
    return out.toByteArray();
  }

  /** Canonical exact-lane bytes. */
  public static byte[] canonicalLaneBytes(final SccpLaneIdV1 lane) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    writeBytes(out, canonicalNetworkBytes(lane.source()));
    writeBytes(out, canonicalNetworkBytes(lane.target()));
    return out.toByteArray();
  }

  /** Blake2b-256 of the domain-separated exact lane. */
  public static byte[] laneHash(final SccpLaneIdV1 lane) {
    return prefixedBlake2b(LANE_HASH_PREFIX, canonicalLaneBytes(lane));
  }

  /** Lane-bound identity. Destination deployment binding is deliberately excluded. */
  public static byte[] messageId(final SccpLaneIdV1 lane, final SccpPayloadV1 payload) {
    if (lane.source().domainId() != payload.sourceDomain()
        || lane.target().domainId() != payload.targetDomain()) {
      throw new IllegalArgumentException("payload domains do not match exact SCCP lane");
    }
    final ByteArrayOutputStream body = new ByteArrayOutputStream();
    body.write(1);
    writeBytes(body, canonicalLaneBytes(lane));
    writeBytes(body, payload.canonicalBytes());
    final byte[] result = prefixedKeccak(MESSAGE_ID_PREFIX, body.toByteArray());
    if (allZero(result)) {
      throw new IllegalArgumentException("messageId must be nonzero");
    }
    return result;
  }

  /** Hash the exact canonical payload. */
  public static byte[] payloadHash(final SccpPayloadV1 payload) {
    return prefixedBlake2b(PAYLOAD_HASH_PREFIX, payload.canonicalBytes());
  }

  /** Decode exactly one canonical transfer payload and reject retired or trailing forms. */
  public static SccpTransferPayloadV1 decodeCanonicalPayload(final byte[] bytes) {
    final Cursor cursor = new Cursor(bytes);
    if (cursor.u8() != 2) {
      throw new IllegalArgumentException("unsupported or retired SCCP payload discriminant");
    }
    if (cursor.u8() != 1) {
      throw new IllegalArgumentException("unsupported SCCP transfer version");
    }
    final int source = cursor.u32Domain("source");
    final int destination = cursor.u32Domain("destination");
    final BigInteger nonce = cursor.unsigned(8);
    final long routeRevision = cursor.u32();
    final int assetHomeDomain = cursor.u32Domain("assetHomeDomain");
    final int assetCodec = cursor.u8();
    final byte[] assetId = cursor.bytes();
    final BigInteger amount = cursor.unsigned(16);
    final int senderCodec = cursor.u8();
    final byte[] sender = cursor.bytes();
    final int recipientCodec = cursor.u8();
    final byte[] recipient = cursor.bytes();
    final int routeCodec = cursor.u8();
    final byte[] routeId = cursor.bytes();
    if (!cursor.finished()) {
      throw new IllegalArgumentException("canonical SCCP payload must not contain trailing bytes");
    }
    final SccpTransferPayloadV1 payload =
        new SccpTransferPayloadV1(
            source,
            destination,
            nonce,
            routeRevision,
            assetHomeDomain,
            assetCodec,
            assetId,
            amount,
            senderCodec,
            sender,
            recipientCodec,
            recipient,
            routeCodec,
            routeId);
    if (!Arrays.equals(payload.canonicalBytes(), bytes)) {
      throw new IllegalArgumentException("non-canonical SCCP payload");
    }
    return payload;
  }

  /** Canonical contract-computable source-event preimage after the domain prefix. */
  public static byte[] canonicalSourceEventBytes(
      final SccpLaneIdV1 lane, final byte[] messageId, final byte[] payloadHash) {
    final List<byte[]> roles = new ArrayList<>();
    roles.add(laneHash(lane));
    roles.add(requireHash(messageId, "messageId"));
    roles.add(requireHash(payloadHash, "payloadHash"));
    requireDistinctHashRoles(roles, "SCCP lane, message, and payload hash roles must be distinct");
    final ByteArrayOutputStream out = new ByteArrayOutputStream(97);
    out.write(1);
    for (final byte[] role : roles) write(out, role);
    return out.toByteArray();
  }

  /** Keccak-256 digest committed by every exact native source event. */
  public static byte[] sourceEventDigest(
      final SccpLaneIdV1 lane, final byte[] messageId, final byte[] payloadHash) {
    return prefixedKeccak(
        SOURCE_EVENT_DIGEST_PREFIX, canonicalSourceEventBytes(lane, messageId, payloadHash));
  }

  /** Construct a role-separated exact outbound commitment. */
  public static SccpHubCommitmentV1 commitment(
      final SccpOutboundMessageContextV1 context, final SccpPayloadV1 payload) {
    final List<byte[]> roles = new ArrayList<>();
    roles.add(laneHash(context.lane()));
    roles.add(context.destinationBindingHash());
    roles.add(context.routeConfigurationHash());
    roles.add(messageId(context.lane(), payload));
    roles.add(payloadHash(payload));
    requireDistinctHashRoles(roles);
    return new SccpHubCommitmentV1(payload.kind(), context, roles.get(3), roles.get(4));
  }

  /** Fixed V1 commitment bytes with four independently governed/hash roles. */
  public static byte[] canonicalCommitmentBytes(final SccpHubCommitmentV1 commitment) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    out.write(commitment.kind().tag());
    out.write(commitment.context().lane().source().tag());
    out.write(commitment.context().lane().target().tag());
    write(out, commitment.context().destinationBindingHash());
    write(out, commitment.context().routeConfigurationHash());
    write(out, commitment.messageId());
    write(out, commitment.payloadHash());
    return out.toByteArray();
  }

  /** Decode and canonically re-encode a fixed V1 commitment. */
  public static SccpHubCommitmentV1 decodeCanonicalCommitment(final byte[] bytes) {
    if (bytes == null || bytes.length != 132) {
      throw new IllegalArgumentException("canonical SCCP commitment must contain 132 bytes");
    }
    if ((bytes[0] & 0xff) != 1) {
      throw new IllegalArgumentException("unsupported SCCP commitment version");
    }
    final SccpHubMessageKindV1 kind = SccpHubMessageKindV1.fromTag(bytes[1] & 0xff);
    final SccpNetworkV1 source = SccpNetworkV1.fromTag(bytes[2] & 0xff);
    final SccpNetworkV1 target = SccpNetworkV1.fromTag(bytes[3] & 0xff);
    if (kind == null || source == null || target == null) {
      throw new IllegalArgumentException("unknown SCCP commitment tag");
    }
    final SccpOutboundMessageContextV1 context =
        new SccpOutboundMessageContextV1(
            new SccpLaneIdV1(source, target),
            Arrays.copyOfRange(bytes, 4, 36),
            Arrays.copyOfRange(bytes, 36, 68));
    final SccpHubCommitmentV1 result =
        new SccpHubCommitmentV1(
            kind,
            context,
            Arrays.copyOfRange(bytes, 68, 100),
            Arrays.copyOfRange(bytes, 100, 132));
    requireDistinctHashRoles(
        Arrays.asList(
            laneHash(context.lane()),
            context.destinationBindingHash(),
            context.routeConfigurationHash(),
            result.messageId(),
            result.payloadHash()));
    if (!Arrays.equals(canonicalCommitmentBytes(result), bytes)) {
      throw new IllegalArgumentException("non-canonical SCCP commitment");
    }
    return result;
  }

  /** Domain-separated leaf/root for an empty Merkle path. */
  public static byte[] commitmentRoot(final SccpHubCommitmentV1 commitment) {
    return prefixedBlake2b(LEAF_HASH_PREFIX, canonicalCommitmentBytes(commitment));
  }

  /** Strict lowercase, prefixless hexadecimal decoder used by shared vector fixtures. */
  public static byte[] decodeLowerHex(final String value) {
    if (value == null || (value.length() & 1) != 0) {
      throw new IllegalArgumentException("hex must be canonical lowercase without 0x");
    }
    final byte[] result = new byte[value.length() / 2];
    for (int i = 0; i < result.length; i++) {
      final char high = value.charAt(i * 2);
      final char low = value.charAt(i * 2 + 1);
      if (!isLowerHex(high) || !isLowerHex(low)) {
        throw new IllegalArgumentException("hex must be canonical lowercase without 0x");
      }
      result[i] = (byte) ((Character.digit(high, 16) << 4) | Character.digit(low, 16));
    }
    return result;
  }

  /** Lowercase, prefixless hexadecimal encoder. */
  public static String encodeLowerHex(final byte[] value) {
    final StringBuilder out = new StringBuilder(value.length * 2);
    for (final byte item : value) {
      out.append(String.format("%02x", item & 0xff));
    }
    return out.toString();
  }

  static void requireDomain(final int value, final String field) {
    if (value != 0 && value != 1 && value != 2 && value != 5) {
      throw new IllegalArgumentException(field + " must be a supported SCCP domain");
    }
  }

  static void requireExternalDomain(final int value, final String field) {
    requireDomain(value, field);
    if (value == 0) {
      throw new IllegalArgumentException(field + " must be external");
    }
  }

  static int accountCodec(final int domain) {
    return switch (domain) {
      case 0 -> 1;
      case 1, 2 -> 2;
      case 5 -> 5;
      default -> throw new IllegalArgumentException("unsupported SCCP domain");
    };
  }

  static byte[] requireHash(final byte[] value, final String field) {
    if (value == null || value.length != 32) {
      throw new IllegalArgumentException(field + " must contain 32 bytes");
    }
    if (allZero(value)) {
      throw new IllegalArgumentException(field + " must be nonzero");
    }
    return Arrays.copyOf(value, value.length);
  }

  static BigInteger requireUnsigned(
      final BigInteger value, final int bits, final String field) {
    if (value == null || value.signum() < 0 || value.bitLength() > bits) {
      throw new IllegalArgumentException(field + " must fit u" + bits);
    }
    return value;
  }

  static byte[] requireFixedAscii(final byte[] value, final String field) {
    if (value == null || value.length != 32) {
      throw new IllegalArgumentException(field + " must contain 32 bytes");
    }
    int end = 0;
    while (end < value.length && value[end] != 0) {
      if ((value[end] & 0x80) != 0) {
        throw new IllegalArgumentException(field + " must be ASCII");
      }
      end++;
    }
    if (end == 0) {
      throw new IllegalArgumentException(field + " must be nonempty");
    }
    for (int i = end; i < value.length; i++) {
      if ((value[i] & 0x80) != 0) {
        throw new IllegalArgumentException(field + " must be ASCII");
      }
    }
    return Arrays.copyOf(value, value.length);
  }

  static byte[] requireCodecValue(final int codec, final byte[] value, final String field) {
    if (value == null || value.length == 0) {
      throw new IllegalArgumentException(field + " must be nonempty");
    }
    boolean valid;
    switch (codec) {
      case 1 -> {
        valid = isCanonicalText(value, field);
      }
      case 2 -> valid = value.length == 20 && !allZero(value);
      case 5 ->
          valid =
              value.length == 21
                  && (value[0] & 0xff) == 0x41
                  && !allZero(Arrays.copyOfRange(value, 1, value.length));
      default -> valid = false;
    }
    if (!valid) {
      throw new IllegalArgumentException(field + " does not match SCCP codec " + codec);
    }
    return Arrays.copyOf(value, value.length);
  }

  private static boolean isCanonicalText(final byte[] value, final String field) {
    if (value.length > 256) return false;
    boolean ascii = true;
    for (final byte item : value) {
      final int octet = item & 0xff;
      ascii &= octet >= 0x21 && octet <= 0x7e;
    }
    if (ascii) return true;

    final String literal = new String(value, StandardCharsets.UTF_8);
    if (!Arrays.equals(value, literal.getBytes(StandardCharsets.UTF_8))) return false;
    try {
      AccountIdLiteral.requireCanonicalI105Address(literal, field);
      return true;
    } catch (final IllegalArgumentException ignored) {
      return false;
    }
  }

  static void writeU32(final ByteArrayOutputStream out, final int value) {
    if (value < 0) {
      throw new IllegalArgumentException("u32 value must be non-negative");
    }
    writeU32Bits(out, value);
  }

  static void writeU32Bits(final ByteArrayOutputStream out, final long value) {
    if (value < 0 || value > 0xffff_ffffL) {
      throw new IllegalArgumentException("value must fit u32");
    }
    for (int shift = 0; shift < 4; shift++) {
      out.write((int) ((value >>> (shift * 8)) & 0xff));
    }
  }

  static void writeUnsignedLe(
      final ByteArrayOutputStream out, final BigInteger value, final int size) {
    if (value.signum() < 0 || value.bitLength() > size * 8) {
      throw new IllegalArgumentException("unsigned integer does not fit");
    }
    final byte[] bigEndian = value.toByteArray();
    for (int index = 0; index < size; index++) {
      final int source = bigEndian.length - 1 - index;
      out.write(source >= 0 ? bigEndian[source] & 0xff : 0);
    }
  }

  static void writeBytes(final ByteArrayOutputStream out, final byte[] value) {
    writeU32Bits(out, value.length);
    write(out, value);
  }

  static void write(final ByteArrayOutputStream out, final byte[] value) {
    out.write(value, 0, value.length);
  }

  private static byte[] prefixedBlake2b(final byte[] prefix, final byte[] payload) {
    final byte[] preimage = new byte[prefix.length + payload.length];
    System.arraycopy(prefix, 0, preimage, 0, prefix.length);
    System.arraycopy(payload, 0, preimage, prefix.length, payload.length);
    return Blake2b.digest256(preimage);
  }

  private static byte[] prefixedKeccak(final byte[] prefix, final byte[] payload) {
    final KeccakDigest digest = new KeccakDigest(256);
    digest.update(prefix, 0, prefix.length);
    digest.update(payload, 0, payload.length);
    final byte[] result = new byte[32];
    digest.doFinal(result, 0);
    return result;
  }

  private static void requireDistinctHashRoles(final List<byte[]> roles) {
    requireDistinctHashRoles(
        roles, "SCCP lane, binding, message, and payload hash roles must be distinct");
  }

  private static void requireDistinctHashRoles(
      final List<byte[]> roles, final String collisionMessage) {
    for (int left = 0; left < roles.size(); left++) {
      if (allZero(roles.get(left))) {
        throw new IllegalArgumentException("SCCP hash roles must be nonzero");
      }
      for (int right = left + 1; right < roles.size(); right++) {
        if (Arrays.equals(roles.get(left), roles.get(right))) {
          throw new IllegalArgumentException(collisionMessage);
        }
      }
    }
  }

  private static boolean allZero(final byte[] value) {
    for (final byte item : value) {
      if (item != 0) {
        return false;
      }
    }
    return true;
  }

  private static boolean isLowerHex(final char value) {
    return (value >= '0' && value <= '9') || (value >= 'a' && value <= 'f');
  }

  private static final class Cursor {
    private final byte[] input;
    private int offset;

    private Cursor(final byte[] input) {
      if (input == null) {
        throw new IllegalArgumentException("canonical SCCP bytes must not be null");
      }
      this.input = input;
    }

    private int u8() {
      return exact(1)[0] & 0xff;
    }

    private long u32() {
      final byte[] value = exact(4);
      return (value[0] & 0xffL)
          | ((value[1] & 0xffL) << 8)
          | ((value[2] & 0xffL) << 16)
          | ((value[3] & 0xffL) << 24);
    }

    private int u32Domain(final String field) {
      final long value = u32();
      if (value > Integer.MAX_VALUE) {
        throw new IllegalArgumentException(field + " is outside the closed domain inventory");
      }
      final int result = (int) value;
      requireDomain(result, field);
      return result;
    }

    private BigInteger unsigned(final int size) {
      final byte[] littleEndian = exact(size);
      final byte[] bigEndian = new byte[size];
      for (int index = 0; index < size; index++) {
        bigEndian[index] = littleEndian[size - index - 1];
      }
      return new BigInteger(1, bigEndian);
    }

    private byte[] bytes() {
      final long size = u32();
      if (size > Integer.MAX_VALUE) {
        throw new IllegalArgumentException("SCCP byte vector is too large");
      }
      return exact((int) size);
    }

    private boolean finished() {
      return offset == input.length;
    }

    private byte[] exact(final int size) {
      if (size < 0 || offset > input.length - size) {
        throw new IllegalArgumentException("truncated SCCP canonical bytes");
      }
      final byte[] value = Arrays.copyOfRange(input, offset, offset + size);
      offset += size;
      return value;
    }
  }

}

package org.hyperledger.iroha.android.sccp;

import java.io.ByteArrayOutputStream;
import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import org.hyperledger.iroha.android.model.instructions.TransferWirePayloadEncoder;

/** Canonical SHA-256 sparse-Merkle replay hashing for SCCP final V1. */
public final class SccpReplayV1 {
  public static final int DEPTH = 248;
  private static final byte[] MAGIC = "SCCP-REPLAY-SMT-V1".getBytes(StandardCharsets.US_ASCII);
  private static final BigInteger MAX_U128 = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE);

  /** Closed replay-operation tags. */
  public enum Boundary {
    SORA_OUTBOUND_LOCK(0x01),
    SORA_INBOUND_RELEASE(0x02),
    EVM_SOURCE_BURN(0x10),
    EVM_DESTINATION_MINT(0x11),
    TRON_SOURCE_BURN(0x20),
    TRON_DESTINATION_MINT(0x21),
    TON_BRIDGE_INBOUND_MINT(0x30),
    TON_BRIDGE_OUTBOUND_BURN(0x31),
    TON_MASTER_MINT(0x32),
    TON_MASTER_BURN(0x33),
    TON_WALLET_MINT_CREDIT(0x34),
    TON_WALLET_BURN_DEBIT(0x35),
    TON_WALLET_REFUND_DEBIT(0x36),
    TON_WALLET_REFUND_CREDIT(0x37);

    private final int tag;

    Boundary(final int tag) {
      this.tag = tag;
    }

    public int tag() {
      return tag;
    }
  }

  /** Immutable canonical contract identity. */
  public static final class Actor {
    private final int kind;
    private final byte[] bytes;

    private Actor(final int kind, final byte[] bytes) {
      this.kind = kind;
      this.bytes = bytes.clone();
    }

    public static Actor route() {
      return new Actor(0, new byte[0]);
    }

    public static Actor evm(final byte[] address) {
      return new Actor(1, exact(address, 20, "EVM replay actor", true));
    }

    public static Actor tron(final byte[] address) {
      return new Actor(2, exact(address, 20, "TRON replay actor", true));
    }

    public static Actor ton(final int workchain, final byte[] account) {
      return new Actor(3, concat(i32be(workchain), exact(account, 32, "TON replay actor", true)));
    }
  }

  /** Immutable canonical economic principal. */
  public static final class Principal {
    private final int kind;
    private final byte[] bytes;

    private Principal(final int kind, final byte[] bytes) {
      if (bytes.length == 0 || bytes.length > 0xffff) {
        throw new IllegalArgumentException("invalid replay principal length");
      }
      this.kind = kind;
      this.bytes = bytes.clone();
    }

    /** Construct from the exact canonical Norito AccountId bytes. */
    public static Principal soraAccount(final byte[] canonicalAccountId) {
      return new Principal(0, canonicalSoraAccountId(canonicalAccountId));
    }

    public static Principal evm(final byte[] address) {
      return new Principal(1, exact(address, 20, "EVM replay principal", true));
    }

    public static Principal tron(final byte[] address) {
      return new Principal(2, exact(address, 20, "TRON replay principal", true));
    }

    public static Principal ton(final int workchain, final byte[] account) {
      return new Principal(
          3, concat(i32be(workchain), exact(account, 32, "TON replay principal", true)));
    }
  }

  /** Strict compressed sparse-Merkle witness. */
  public static final class Witness {
    private final byte[] expectedRoot;
    private final byte[] priorDigest;
    private final byte[] bitmap;
    private final List<byte[]> siblings;

    public Witness(
        final byte[] expectedRoot,
        final byte[] priorDigest,
        final byte[] bitmap,
        final List<byte[]> siblings) {
      this.expectedRoot = exact(expectedRoot, 32, "expected shard root", true);
      this.priorDigest = exact(priorDigest, 32, "prior record digest", false);
      this.bitmap = exact(bitmap, 32, "sibling bitmap", false);
      final List<byte[]> copied = new ArrayList<>(siblings.size());
      for (int index = 0; index < siblings.size(); index++) {
        copied.add(exact(siblings.get(index), 32, "sibling", true));
      }
      this.siblings = Collections.unmodifiableList(copied);
    }
  }

  /** Reconstructed witness root and shard. */
  public static final class WitnessRoot {
    private final byte[] root;
    private final byte[] expected;
    private final int shard;

    private WitnessRoot(final byte[] root, final byte[] expected, final int shard) {
      this.root = root.clone();
      this.expected = expected.clone();
      this.shard = shard;
    }

    public byte[] root() {
      return root.clone();
    }

    public byte[] expectedRoot() {
      return expected.clone();
    }

    public boolean matchesExpectedRoot() {
      return Arrays.equals(root, expected);
    }

    public int shard() {
      return shard;
    }
  }

  private SccpReplayV1() {}

  /** Hash one exact production replay domain. */
  public static byte[] domainHash(
      final SccpNetworkV1 source,
      final SccpNetworkV1 target,
      final Boundary boundary,
      final long routeRevision,
      final byte[] routeConfigurationHash,
      final Actor actor) {
    if (!source.isProduction() || !target.isProduction()) {
      throw new IllegalArgumentException("replay domains admit production networks only");
    }
    if (routeRevision <= 0 || routeRevision > 0xffff_ffffL) {
      throw new IllegalArgumentException("route revision must be a nonzero u32");
    }
    if (!validDirection(source, target, boundary, actor.kind)) {
      throw new IllegalArgumentException("invalid replay boundary, direction, or actor");
    }
    return hash(
        MAGIC,
        new byte[] {0},
        unsignedBe(BigInteger.valueOf(source.tag()), 4, "source tag"),
        unsignedBe(BigInteger.valueOf(target.tag()), 4, "target tag"),
        new byte[] {(byte) boundary.tag()},
        unsignedBe(BigInteger.valueOf(routeRevision), 4, "route revision"),
        exact(routeConfigurationHash, 32, "route configuration hash", true),
        new byte[] {(byte) actor.kind},
        unsignedBe(BigInteger.valueOf(actor.bytes.length), 2, "actor length"),
        actor.bytes);
  }

  /** Derive the full replay key; byte zero selects the shard. */
  public static byte[] replayKey(final byte[] domainHash, final byte[] replayId) {
    return hash(
        MAGIC,
        new byte[] {1},
        exact(domainHash, 32, "domain hash", true),
        exact(replayId, 32, "replay id", true));
  }

  /** Hash one occupied leaf record with a canonical scale-9 u128 amount. */
  public static byte[] recordDigest(
      final Boundary operation,
      final byte[] replayId,
      final byte[] payloadSha256,
      final BigInteger amountScale9,
      final Principal principal,
      final byte[] auxiliaryIdentitySha256) {
    if (amountScale9.signum() <= 0 || amountScale9.compareTo(MAX_U128) > 0) {
      throw new IllegalArgumentException("replay amount must be a positive u128");
    }
    final byte[] principalDigest =
        hash(
            MAGIC,
            new byte[] {3, (byte) principal.kind},
            unsignedBe(BigInteger.valueOf(principal.bytes.length), 2, "principal length"),
            principal.bytes);
    final byte[] auxiliary =
        hash(
            MAGIC,
            new byte[] {4, (byte) operation.tag()},
            exact(auxiliaryIdentitySha256, 32, "auxiliary identity SHA-256", true));
    return hash(
        MAGIC,
        new byte[] {2, (byte) operation.tag()},
        exact(replayId, 32, "replay id", true),
        exact(payloadSha256, 32, "payload SHA-256", true),
        unsignedBe(amountScale9, 16, "scale-9 amount"),
        principalDigest,
        auxiliary);
  }

  /** Return all 249 canonical empty hashes in leaf-up order. */
  public static List<byte[]> emptyHashes() {
    final List<byte[]> result = new ArrayList<>(DEPTH + 1);
    result.add(hash(MAGIC, new byte[] {0x10}));
    for (int level = 0; level < DEPTH; level++) {
      result.add(parent(level, result.get(level), result.get(level)));
    }
    final List<byte[]> copied = new ArrayList<>(result.size());
    for (final byte[] value : result) copied.add(value.clone());
    return Collections.unmodifiableList(copied);
  }

  /** Strictly reconstruct one canonical compressed witness. */
  public static WitnessRoot rootFromWitness(
      final byte[] keyValue, final byte[] recordDigest, final Witness witness) {
    final byte[] key = exact(keyValue, 32, "replay key", true);
    if (witness.bitmap[0] != 0) {
      throw new IllegalArgumentException("witness bitmap has reserved high bits");
    }
    int setBits = 0;
    for (final byte item : witness.bitmap) setBits += Integer.bitCount(item & 0xff);
    if (setBits != witness.siblings.size() || setBits > DEPTH) {
      throw new IllegalArgumentException("witness sibling count does not match bitmap");
    }
    final List<byte[]> empty = emptyHashes();
    byte[] current;
    if (recordDigest == null) {
      if (!allZero(witness.priorDigest)) {
        throw new IllegalArgumentException("non-membership witness has an occupied digest");
      }
      current = empty.get(0);
    } else {
      final byte[] digest = exact(recordDigest, 32, "record digest", true);
      if (!Arrays.equals(digest, witness.priorDigest)) {
        throw new IllegalArgumentException("membership witness record digest mismatch");
      }
      current = hash(MAGIC, new byte[] {0x11}, key, digest);
    }
    int supplied = 0;
    for (int level = 0; level < DEPTH; level++) {
      byte[] sibling = empty.get(level);
      if (bit(witness.bitmap, level)) {
        sibling = witness.siblings.get(supplied++);
        if (Arrays.equals(sibling, empty.get(level))) {
          throw new IllegalArgumentException("witness explicitly encodes a default sibling");
        }
      }
      current = bit(key, level) ? parent(level, sibling, current) : parent(level, current, sibling);
    }
    return new WitnessRoot(current, witness.expectedRoot, key[0] & 0xff);
  }

  private static boolean validDirection(
      final SccpNetworkV1 source,
      final SccpNetworkV1 target,
      final Boundary boundary,
      final int actorKind) {
    switch (boundary) {
      case SORA_OUTBOUND_LOCK:
        return source == SccpNetworkV1.SORA_TAIRA && target.isExternal() && actorKind == 0;
      case SORA_INBOUND_RELEASE:
        return source.isExternal() && target == SccpNetworkV1.SORA_TAIRA && actorKind == 0;
      case EVM_SOURCE_BURN:
        return isEvm(source) && target == SccpNetworkV1.SORA_TAIRA && actorKind == 1;
      case EVM_DESTINATION_MINT:
        return source == SccpNetworkV1.SORA_TAIRA && isEvm(target) && actorKind == 1;
      case TRON_SOURCE_BURN:
        return source == SccpNetworkV1.TRON_MAINNET
            && target == SccpNetworkV1.SORA_TAIRA
            && actorKind == 2;
      case TRON_DESTINATION_MINT:
        return source == SccpNetworkV1.SORA_TAIRA
            && target == SccpNetworkV1.TRON_MAINNET
            && actorKind == 2;
      case TON_BRIDGE_INBOUND_MINT:
      case TON_MASTER_MINT:
      case TON_WALLET_MINT_CREDIT:
      case TON_WALLET_REFUND_DEBIT:
      case TON_WALLET_REFUND_CREDIT:
        return source == SccpNetworkV1.SORA_TAIRA
            && target == SccpNetworkV1.TON_MAINNET
            && actorKind == 3;
      case TON_BRIDGE_OUTBOUND_BURN:
      case TON_MASTER_BURN:
      case TON_WALLET_BURN_DEBIT:
        return source == SccpNetworkV1.TON_MAINNET
            && target == SccpNetworkV1.SORA_TAIRA
            && actorKind == 3;
      default:
        return false;
    }
  }

  private static boolean isEvm(final SccpNetworkV1 network) {
    return network == SccpNetworkV1.ETHEREUM_MAINNET || network == SccpNetworkV1.BSC_MAINNET;
  }

  private static byte[] parent(final int level, final byte[] left, final byte[] right) {
    return hash(
        MAGIC,
        new byte[] {0x12},
        unsignedBe(BigInteger.valueOf(level), 2, "tree level"),
        left,
        right);
  }

  private static boolean bit(final byte[] value, final int level) {
    return (value[31 - level / 8] & (1 << (level % 8))) != 0;
  }

  private static byte[] exact(
      final byte[] value, final int length, final String label, final boolean nonzero) {
    if (value == null || value.length != length || (nonzero && allZero(value))) {
      throw new IllegalArgumentException(
          label + " must be " + (nonzero ? "nonzero " : "") + length + " bytes");
    }
    return value.clone();
  }

  private static boolean allZero(final byte[] value) {
    for (final byte item : value) if (item != 0) return false;
    return true;
  }

  private static byte[] canonicalSoraAccountId(final byte[] payload) {
    Objects.requireNonNull(payload, "canonicalAccountId");
    if (payload.length == 0 || payload.length > 0xffff) {
      throw new IllegalArgumentException(
          "SORA replay principal must be canonical nonempty AccountId bytes");
    }
    final String rendered;
    try {
      rendered =
          TransferWirePayloadEncoder.decodeAccountIdPayload(
              payload, SccpV1.TAIRA_I105_DISCRIMINANT_V1);
    } catch (RuntimeException error) {
      throw new IllegalArgumentException(
          "SORA replay principal is not a canonical AccountId", error);
    }
    final byte[] canonical;
    try {
      canonical = TransferWirePayloadEncoder.encodeAccountIdPayload(rendered);
    } catch (RuntimeException error) {
      throw new IllegalArgumentException(
          "SORA replay principal is not a canonical AccountId", error);
    }
    if (!Arrays.equals(canonical, payload)) {
      throw new IllegalArgumentException(
          "SORA replay principal is not the canonical AccountId encoding");
    }
    return payload.clone();
  }

  private static byte[] i32be(final int value) {
    return new byte[] {
      (byte) (value >>> 24), (byte) (value >>> 16), (byte) (value >>> 8), (byte) value
    };
  }

  private static byte[] unsignedBe(
      final BigInteger value, final int width, final String label) {
    if (value.signum() < 0 || value.bitLength() > width * 8) {
      throw new IllegalArgumentException(label + " exceeds u" + (width * 8));
    }
    final byte[] source = value.toByteArray();
    final byte[] result = new byte[width];
    final int count = Math.min(width, source.length);
    System.arraycopy(source, source.length - count, result, width - count, count);
    return result;
  }

  private static byte[] concat(final byte[]... parts) {
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    for (final byte[] part : parts) out.write(part, 0, part.length);
    return out.toByteArray();
  }

  private static byte[] hash(final byte[]... parts) {
    try {
      final MessageDigest digest = MessageDigest.getInstance("SHA-256");
      for (final byte[] part : parts) digest.update(part);
      return digest.digest();
    } catch (final NoSuchAlgorithmException impossible) {
      throw new AssertionError("JVM lacks mandatory SHA-256", impossible);
    }
  }
}

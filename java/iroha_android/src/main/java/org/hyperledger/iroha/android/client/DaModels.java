package org.hyperledger.iroha.android.client;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.address.AccountIdLiteral;

/** Typed first-release models for Torii DA commitment and pin-intent proofs. */
public final class DaModels {
  static final BigInteger U32_MAX = new BigInteger("4294967295");
  static final BigInteger U64_MAX = new BigInteger("18446744073709551615");

  private DaModels() {}

  /** Canonical transparent Norito JSON wrapper around a 32-byte DA digest. */
  public static final class Digest32 {
    private final byte[] bytes;

    private Digest32(final byte[] bytes) {
      if (bytes == null || bytes.length != 32) {
        throw new IllegalArgumentException("DA digest must contain exactly 32 bytes");
      }
      this.bytes = bytes.clone();
    }

    public static Digest32 fromBytes(final byte[] bytes) {
      return new Digest32(bytes);
    }

    public static Digest32 fromHex(final String hex) {
      Objects.requireNonNull(hex, "hex");
      String body = hex.trim();
      if (body.startsWith("0x") || body.startsWith("0X")) {
        body = body.substring(2);
      }
      if (body.length() != 64 || !body.matches("[0-9a-fA-F]{64}")) {
        throw new IllegalArgumentException("DA digest must be a 32-byte hex string");
      }
      final byte[] decoded = new byte[32];
      for (int index = 0; index < decoded.length; index++) {
        decoded[index] =
            (byte) Integer.parseInt(body.substring(index * 2, index * 2 + 2), 16);
      }
      return new Digest32(decoded);
    }

    static Digest32 fromJson(final Object value, final String field) {
      final List<?> outer = DaJson.list(value, field);
      if (outer.size() != 1) {
        throw new IllegalArgumentException(
            field + " must contain one transparent-wrapper item");
      }
      final List<?> inner = DaJson.list(outer.get(0), field + "[0]");
      if (inner.size() != 32) {
        throw new IllegalArgumentException(field + " must contain exactly 32 bytes");
      }
      final byte[] decoded = new byte[32];
      for (int index = 0; index < decoded.length; index++) {
        decoded[index] = (byte) DaJson.u8(inner.get(index), field + "[0][" + index + "]");
      }
      return new Digest32(decoded);
    }

    List<List<Integer>> toJsonValue() {
      final List<Integer> inner = new ArrayList<>(32);
      for (final byte value : bytes) {
        inner.add(value & 0xff);
      }
      return Collections.singletonList(Collections.unmodifiableList(inner));
    }

    public byte[] bytes() {
      return bytes.clone();
    }

    public String hex() {
      final StringBuilder value = new StringBuilder(64);
      for (final byte item : bytes) {
        value.append(String.format("%02x", item & 0xff));
      }
      return value.toString();
    }

    @Override
    public boolean equals(final Object other) {
      return other instanceof Digest32
          && Arrays.equals(bytes, ((Digest32) other).bytes);
    }

    @Override
    public int hashCode() {
      return Arrays.hashCode(bytes);
    }

    @Override
    public String toString() {
      return hex();
    }
  }

  /** Query pagination using the complete unsigned 64-bit wire range. */
  public static final class Pagination {
    private final BigInteger limit;
    private final BigInteger offset;

    public Pagination(final BigInteger limit, final BigInteger offset) {
      if (limit != null
          && (limit.signum() <= 0 || limit.compareTo(U64_MAX) > 0)) {
        throw new IllegalArgumentException(
            "DA pagination limit must be in 1..u64::MAX");
      }
      requireU64(Objects.requireNonNull(offset, "offset"), "offset");
      this.limit = limit;
      this.offset = offset;
    }

    public Pagination() {
      this(null, BigInteger.ZERO);
    }

    Map<String, Object> toJsonValue() {
      final Map<String, Object> value = new LinkedHashMap<>();
      if (limit != null) {
        value.put("limit", limit);
      }
      value.put("offset", offset);
      return value;
    }

    public BigInteger limit() {
      return limit;
    }

    public BigInteger offset() {
      return offset;
    }
  }

  /** Commitment list/prove request. */
  public static final class CommitmentQuery {
    private final Digest32 manifestHash;
    private final Long laneId;
    private final BigInteger epoch;
    private final BigInteger sequence;
    private final Pagination pagination;

    public CommitmentQuery(
        final Digest32 manifestHash,
        final Long laneId,
        final BigInteger epoch,
        final BigInteger sequence,
        final Pagination pagination) {
      if (laneId != null) {
        requireU32(BigInteger.valueOf(laneId), "laneId");
      }
      if (epoch != null) {
        requireU64(epoch, "epoch");
      }
      if (sequence != null) {
        requireU64(sequence, "sequence");
      }
      this.manifestHash = manifestHash;
      this.laneId = laneId;
      this.epoch = epoch;
      this.sequence = sequence;
      this.pagination = pagination;
    }

    public CommitmentQuery() {
      this(null, null, null, null, null);
    }

    byte[] toJsonBytes() {
      final Map<String, Object> value = new LinkedHashMap<>();
      if (manifestHash != null) {
        value.put("manifest_hash", manifestHash.toJsonValue());
      }
      if (laneId != null) {
        value.put("lane_id", laneId);
      }
      if (epoch != null) {
        value.put("epoch", epoch);
      }
      if (sequence != null) {
        value.put("sequence", sequence);
      }
      if (pagination != null) {
        value.put("pagination", pagination.toJsonValue());
      }
      return DaJson.encode(value);
    }

    public Digest32 manifestHash() {
      return manifestHash;
    }

    public Long laneId() {
      return laneId;
    }

    public BigInteger epoch() {
      return epoch;
    }

    public BigInteger sequence() {
      return sequence;
    }

    public Pagination pagination() {
      return pagination;
    }
  }

  /** Pin-intent list/prove request. */
  public static final class PinIntentQuery {
    private final Digest32 manifestHash;
    private final Digest32 storageTicket;
    private final String alias;
    private final Long laneId;
    private final BigInteger epoch;
    private final BigInteger sequence;
    private final Pagination pagination;

    public PinIntentQuery(
        final Digest32 manifestHash,
        final Digest32 storageTicket,
        final String alias,
        final Long laneId,
        final BigInteger epoch,
        final BigInteger sequence,
        final Pagination pagination) {
      if (alias != null) {
        requirePinIntentAlias(alias, "DA pin-intent alias");
      }
      if (laneId != null) {
        requireU32(BigInteger.valueOf(laneId), "laneId");
      }
      if (epoch != null) {
        requireU64(epoch, "epoch");
      }
      if (sequence != null) {
        requireU64(sequence, "sequence");
      }
      this.manifestHash = manifestHash;
      this.storageTicket = storageTicket;
      this.alias = alias;
      this.laneId = laneId;
      this.epoch = epoch;
      this.sequence = sequence;
      this.pagination = pagination;
    }

    public PinIntentQuery() {
      this(null, null, null, null, null, null, null);
    }

    byte[] toJsonBytes() {
      final Map<String, Object> value = new LinkedHashMap<>();
      if (manifestHash != null) {
        value.put("manifest_hash", manifestHash.toJsonValue());
      }
      if (storageTicket != null) {
        value.put("storage_ticket", storageTicket.toJsonValue());
      }
      if (alias != null) {
        value.put("alias", alias);
      }
      if (laneId != null) {
        value.put("lane_id", laneId);
      }
      if (epoch != null) {
        value.put("epoch", epoch);
      }
      if (sequence != null) {
        value.put("sequence", sequence);
      }
      if (pagination != null) {
        value.put("pagination", pagination.toJsonValue());
      }
      return DaJson.encode(value);
    }

    public Digest32 manifestHash() {
      return manifestHash;
    }

    public Digest32 storageTicket() {
      return storageTicket;
    }

    public String alias() {
      return alias;
    }

    public Long laneId() {
      return laneId;
    }

    public BigInteger epoch() {
      return epoch;
    }

    public BigInteger sequence() {
      return sequence;
    }

    public Pagination pagination() {
      return pagination;
    }
  }

  public enum ProofScheme {
    MERKLE_SHA256("MerkleSha256");

    private final String wireName;

    ProofScheme(final String wireName) {
      this.wireName = wireName;
    }

    static ProofScheme fromJson(final Object value, final String field) {
      final String type =
          DaJson.taggedUnit(
              value,
              field,
              "type",
              Collections.singleton("MerkleSha256"));
      return MERKLE_SHA256;
    }

    public String wireName() {
      return wireName;
    }
  }

  public enum StorageClass {
    HOT("Hot"),
    WARM("Warm"),
    COLD("Cold");

    private final String wireName;

    StorageClass(final String wireName) {
      this.wireName = wireName;
    }

    static StorageClass fromJson(final Object value, final String field) {
      final String type =
          DaJson.taggedUnit(
              value,
              field,
              "type",
              new java.util.LinkedHashSet<>(Arrays.asList("Hot", "Warm", "Cold")));
      for (final StorageClass storageClass : values()) {
        if (storageClass.wireName.equals(type)) {
          return storageClass;
        }
      }
      throw new IllegalArgumentException(field + " has an unsupported storage class");
    }

    Map<String, Object> toJsonValue() {
      final Map<String, Object> value = new LinkedHashMap<>();
      value.put("type", wireName);
      value.put("value", null);
      return value;
    }

    public String wireName() {
      return wireName;
    }
  }

  public static final class RetentionPolicy {
    private final BigInteger hotRetentionSeconds;
    private final BigInteger coldRetentionSeconds;
    private final int requiredReplicas;
    private final StorageClass storageClass;
    private final String governanceTag;

    public RetentionPolicy(
        final BigInteger hotRetentionSeconds,
        final BigInteger coldRetentionSeconds,
        final int requiredReplicas,
        final StorageClass storageClass,
        final String governanceTag) {
      requireU64(hotRetentionSeconds, "hotRetentionSeconds");
      requireU64(coldRetentionSeconds, "coldRetentionSeconds");
      if (requiredReplicas < 0 || requiredReplicas > 65535) {
        throw new IllegalArgumentException("requiredReplicas must fit u16");
      }
      this.hotRetentionSeconds = hotRetentionSeconds;
      this.coldRetentionSeconds = coldRetentionSeconds;
      this.requiredReplicas = requiredReplicas;
      this.storageClass = Objects.requireNonNull(storageClass, "storageClass");
      this.governanceTag = Objects.requireNonNull(governanceTag, "governanceTag");
    }

    Map<String, Object> toJsonValue() {
      final Map<String, Object> value = new LinkedHashMap<>();
      value.put("hot_retention_secs", hotRetentionSeconds);
      value.put("cold_retention_secs", coldRetentionSeconds);
      value.put("required_replicas", requiredReplicas);
      value.put("storage_class", storageClass.toJsonValue());
      value.put("governance_tag", Collections.singletonList(governanceTag));
      return value;
    }

    public BigInteger hotRetentionSeconds() {
      return hotRetentionSeconds;
    }

    public BigInteger coldRetentionSeconds() {
      return coldRetentionSeconds;
    }

    public int requiredReplicas() {
      return requiredReplicas;
    }

    public StorageClass storageClass() {
      return storageClass;
    }

    public String governanceTag() {
      return governanceTag;
    }
  }

  public static final class CommitmentRecord {
    private final long laneId;
    private final BigInteger epoch;
    private final BigInteger sequence;
    private final Digest32 clientBlobId;
    private final Digest32 manifestHash;
    private final ProofScheme proofScheme;
    private final String chunkRoot;
    private final String proofDigest;
    private final RetentionPolicy retentionClass;
    private final Digest32 storageTicket;
    private final String acknowledgementSignature;

    public CommitmentRecord(
        final long laneId,
        final BigInteger epoch,
        final BigInteger sequence,
        final Digest32 clientBlobId,
        final Digest32 manifestHash,
        final ProofScheme proofScheme,
        final String chunkRoot,
        final String proofDigest,
        final RetentionPolicy retentionClass,
        final Digest32 storageTicket,
        final String acknowledgementSignature) {
      requireU32(BigInteger.valueOf(laneId), "laneId");
      requireU64(epoch, "epoch");
      requireU64(sequence, "sequence");
      DaJson.requireHash(chunkRoot, "chunkRoot");
      if (proofDigest != null) {
        DaJson.requireHash(proofDigest, "proofDigest");
      }
      if (acknowledgementSignature == null
          || !acknowledgementSignature.matches("[0-9A-F]{128}")) {
        throw new IllegalArgumentException(
            "acknowledgementSignature must contain 64 canonical uppercase bytes");
      }
      this.laneId = laneId;
      this.epoch = epoch;
      this.sequence = sequence;
      this.clientBlobId = Objects.requireNonNull(clientBlobId, "clientBlobId");
      this.manifestHash = Objects.requireNonNull(manifestHash, "manifestHash");
      this.proofScheme = Objects.requireNonNull(proofScheme, "proofScheme");
      this.chunkRoot = chunkRoot;
      this.proofDigest = proofDigest;
      this.retentionClass = Objects.requireNonNull(retentionClass, "retentionClass");
      this.storageTicket = Objects.requireNonNull(storageTicket, "storageTicket");
      this.acknowledgementSignature = acknowledgementSignature;
    }

    Map<String, Object> toJsonValue() {
      final Map<String, Object> value = new LinkedHashMap<>();
      value.put("lane_id", laneId);
      value.put("epoch", epoch);
      value.put("sequence", sequence);
      value.put("client_blob_id", clientBlobId.toJsonValue());
      value.put("manifest_hash", manifestHash.toJsonValue());
      final Map<String, Object> scheme = new LinkedHashMap<>();
      scheme.put("type", proofScheme.wireName);
      scheme.put("value", null);
      value.put("proof_scheme", scheme);
      value.put("chunk_root", chunkRoot);
      value.put("proof_digest", proofDigest);
      value.put("retention_class", retentionClass.toJsonValue());
      value.put("storage_ticket", storageTicket.toJsonValue());
      value.put("acknowledgement_sig", acknowledgementSignature);
      return value;
    }

    public long laneId() {
      return laneId;
    }

    public BigInteger epoch() {
      return epoch;
    }

    public BigInteger sequence() {
      return sequence;
    }

    public Digest32 clientBlobId() {
      return clientBlobId;
    }

    public Digest32 manifestHash() {
      return manifestHash;
    }

    public ProofScheme proofScheme() {
      return proofScheme;
    }

    public String chunkRoot() {
      return chunkRoot;
    }

    public String proofDigest() {
      return proofDigest;
    }

    public RetentionPolicy retentionClass() {
      return retentionClass;
    }

    public Digest32 storageTicket() {
      return storageTicket;
    }

    public String acknowledgementSignature() {
      return acknowledgementSignature;
    }
  }

  public static final class PinIntent {
    private final long laneId;
    private final BigInteger epoch;
    private final BigInteger sequence;
    private final Digest32 storageTicket;
    private final Digest32 manifestHash;
    private final String alias;
    private final String owner;

    public PinIntent(
        final long laneId,
        final BigInteger epoch,
        final BigInteger sequence,
        final Digest32 storageTicket,
        final Digest32 manifestHash,
        final String alias,
        final String owner) {
      requireU32(BigInteger.valueOf(laneId), "laneId");
      requireU64(epoch, "epoch");
      requireU64(sequence, "sequence");
      if (alias != null) {
        requirePinIntentAlias(alias, "DA pin alias");
      }
      if (owner != null) {
        AccountIdLiteral.requireCanonicalI105Address(owner, "owner");
      }
      this.laneId = laneId;
      this.epoch = epoch;
      this.sequence = sequence;
      this.storageTicket = Objects.requireNonNull(storageTicket, "storageTicket");
      this.manifestHash = Objects.requireNonNull(manifestHash, "manifestHash");
      this.alias = alias;
      this.owner = owner;
    }

    Map<String, Object> toJsonValue() {
      final Map<String, Object> value = new LinkedHashMap<>();
      value.put("lane_id", laneId);
      value.put("epoch", epoch);
      value.put("sequence", sequence);
      value.put("storage_ticket", storageTicket.toJsonValue());
      value.put("manifest_hash", manifestHash.toJsonValue());
      value.put("alias", alias);
      value.put("owner", owner);
      return value;
    }

    public long laneId() {
      return laneId;
    }

    public BigInteger epoch() {
      return epoch;
    }

    public BigInteger sequence() {
      return sequence;
    }

    public Digest32 storageTicket() {
      return storageTicket;
    }

    public Digest32 manifestHash() {
      return manifestHash;
    }

    public String alias() {
      return alias;
    }

    public String owner() {
      return owner;
    }
  }

  /** Active lane proof policy. */
  public static final class ProofPolicy {
    private final long laneId;
    private final BigInteger dataspaceId;
    private final String alias;
    private final ProofScheme proofScheme;

    public ProofPolicy(
        final long laneId,
        final BigInteger dataspaceId,
        final String alias,
        final ProofScheme proofScheme) {
      requireU32(BigInteger.valueOf(laneId), "laneId");
      requireU64(dataspaceId, "dataspaceId");
      if (alias == null || alias.trim().isEmpty() || !alias.equals(alias.trim())) {
        throw new IllegalArgumentException(
            "DA policy alias must be exact and non-blank");
      }
      this.laneId = laneId;
      this.dataspaceId = dataspaceId;
      this.alias = alias;
      this.proofScheme = Objects.requireNonNull(proofScheme, "proofScheme");
    }

    public long laneId() {
      return laneId;
    }

    public BigInteger dataspaceId() {
      return dataspaceId;
    }

    public String alias() {
      return alias;
    }

    public ProofScheme proofScheme() {
      return proofScheme;
    }
  }

  /** Versioned active proof-policy bundle. */
  public static final class ProofPolicyBundle {
    private final int version;
    private final String policyHash;
    private final List<ProofPolicy> policies;

    public ProofPolicyBundle(
        final int version, final String policyHash, final List<ProofPolicy> policies) {
      if (version != 1) {
        throw new IllegalArgumentException("only DA proof-policy bundle V1 is supported");
      }
      DaJson.requireHash(policyHash, "policy_hash");
      this.version = version;
      this.policyHash = policyHash;
      this.policies =
          Collections.unmodifiableList(new ArrayList<>(Objects.requireNonNull(policies, "policies")));
    }

    public int version() {
      return version;
    }

    public String policyHash() {
      return policyHash;
    }

    public List<ProofPolicy> policies() {
      return policies;
    }
  }

  /** Location of a record in an on-chain DA bundle. */
  public static final class Location {
    private final BigInteger blockHeight;
    private final long indexInBundle;

    public Location(final BigInteger blockHeight, final long indexInBundle) {
      requireU64(blockHeight, "blockHeight");
      if (blockHeight.signum() == 0) {
        throw new IllegalArgumentException("DA block height must be nonzero");
      }
      requireU32(BigInteger.valueOf(indexInBundle), "indexInBundle");
      this.blockHeight = blockHeight;
      this.indexInBundle = indexInBundle;
    }

    Map<String, Object> toJsonValue() {
      final Map<String, Object> value = new LinkedHashMap<>();
      value.put("block_height", blockHeight);
      value.put("index_in_bundle", indexInBundle);
      return value;
    }

    public BigInteger blockHeight() {
      return blockHeight;
    }

    public long indexInBundle() {
      return indexInBundle;
    }
  }

  public enum MerkleDirection {
    LEFT("Left"),
    RIGHT("Right");

    private final String wireName;

    MerkleDirection(final String wireName) {
      this.wireName = wireName;
    }

    Map<String, Object> toJsonValue() {
      final Map<String, Object> value = new LinkedHashMap<>();
      value.put("direction", wireName);
      value.put("value", null);
      return value;
    }

    public String wireName() {
      return wireName;
    }
  }

  public static final class MerklePathItem {
    private final String sibling;
    private final MerkleDirection direction;

    public MerklePathItem(final String sibling, final MerkleDirection direction) {
      DaJson.requireHash(sibling, "sibling");
      this.sibling = sibling;
      this.direction = Objects.requireNonNull(direction, "direction");
    }

    Map<String, Object> toJsonValue() {
      final Map<String, Object> value = new LinkedHashMap<>();
      value.put("sibling", sibling);
      value.put("direction", direction.toJsonValue());
      return value;
    }

    public String sibling() {
      return sibling;
    }

    public MerkleDirection direction() {
      return direction;
    }
  }

  /** Typed commitment proof envelope. */
  public static final class CommitmentProof {
    private final CommitmentRecord commitment;
    private final Location location;
    /** Header commitment to the V1 tree version, leaf count, and Merkle root. */
    private final String bundleHash;
    private final long bundleLength;
    private final String root;
    private final List<MerklePathItem> path;

    public CommitmentProof(
        final CommitmentRecord commitment,
        final Location location,
        final String bundleHash,
        final long bundleLength,
        final String root,
        final List<MerklePathItem> path) {
      this.commitment = Objects.requireNonNull(commitment, "commitment");
      this.location = Objects.requireNonNull(location, "location");
      DaJson.requireHash(bundleHash, "bundle_hash");
      requireU32(BigInteger.valueOf(bundleLength), "bundleLength");
      if (bundleLength == 0) {
        throw new IllegalArgumentException("DA proof bundle length must be nonzero");
      }
      DaJson.requireHash(root, "root");
      if (path.size() > 32) {
        throw new IllegalArgumentException(
            "DA Merkle path exceeds the u32 bundle depth");
      }
      this.bundleHash = bundleHash;
      this.bundleLength = bundleLength;
      this.root = root;
      this.path = Collections.unmodifiableList(new ArrayList<>(path));
      if (!merklePathMatchesLocation(this.path, location.indexInBundle, bundleLength)) {
        throw new IllegalArgumentException(
            "DA Merkle path shape does not match its bundle location");
      }
    }

    public byte[] toJsonBytes() {
      final Map<String, Object> value = new LinkedHashMap<>();
      value.put("commitment", commitment.toJsonValue());
      value.put("location", location.toJsonValue());
      value.put("bundle_hash", bundleHash);
      value.put("bundle_len", bundleLength);
      value.put("root", root);
      final List<Object> encodedPath = new ArrayList<>(path.size());
      for (final MerklePathItem item : path) {
        encodedPath.add(item.toJsonValue());
      }
      value.put("path", encodedPath);
      return DaJson.encode(value);
    }

    public CommitmentRecord commitment() {
      return commitment;
    }

    public Location location() {
      return location;
    }

    public String bundleHash() {
      return bundleHash;
    }

    public long bundleLength() {
      return bundleLength;
    }

    public String root() {
      return root;
    }

    public List<MerklePathItem> path() {
      return path;
    }
  }

  /** Typed pin-intent proof envelope. */
  public static final class PinIntentProof {
    private final PinIntent intent;
    private final Location location;
    /** Header commitment to the V1 tree version, leaf count, and Merkle root. */
    private final String bundleHash;
    private final long bundleLength;
    private final String root;
    private final List<MerklePathItem> path;

    public PinIntentProof(
        final PinIntent intent,
        final Location location,
        final String bundleHash,
        final long bundleLength,
        final String root,
        final List<MerklePathItem> path) {
      this.intent = Objects.requireNonNull(intent, "intent");
      this.location = Objects.requireNonNull(location, "location");
      DaJson.requireHash(bundleHash, "bundle_hash");
      requireU32(BigInteger.valueOf(bundleLength), "bundleLength");
      if (bundleLength == 0) {
        throw new IllegalArgumentException("DA proof bundle length must be nonzero");
      }
      DaJson.requireHash(root, "root");
      if (path.size() > 32) {
        throw new IllegalArgumentException(
            "DA Merkle path exceeds the u32 bundle depth");
      }
      this.bundleHash = bundleHash;
      this.bundleLength = bundleLength;
      this.root = root;
      this.path = Collections.unmodifiableList(new ArrayList<>(path));
      if (!merklePathMatchesLocation(this.path, location.indexInBundle, bundleLength)) {
        throw new IllegalArgumentException(
            "DA Merkle path shape does not match its bundle location");
      }
    }

    public byte[] toJsonBytes() {
      final Map<String, Object> value = new LinkedHashMap<>();
      value.put("intent", intent.toJsonValue());
      value.put("location", location.toJsonValue());
      value.put("bundle_hash", bundleHash);
      value.put("bundle_len", bundleLength);
      value.put("root", root);
      final List<Object> encodedPath = new ArrayList<>(path.size());
      for (final MerklePathItem item : path) {
        encodedPath.add(item.toJsonValue());
      }
      value.put("path", encodedPath);
      return DaJson.encode(value);
    }

    public PinIntent intent() {
      return intent;
    }

    public Location location() {
      return location;
    }

    public String bundleHash() {
      return bundleHash;
    }

    public long bundleLength() {
      return bundleLength;
    }

    public String root() {
      return root;
    }

    public List<MerklePathItem> path() {
      return path;
    }
  }

  public static final class CommitmentWithLocation {
    private final CommitmentRecord commitment;
    private final Location location;

    public CommitmentWithLocation(
        final CommitmentRecord commitment, final Location location) {
      this.commitment = Objects.requireNonNull(commitment, "commitment");
      this.location = Objects.requireNonNull(location, "location");
    }

    public CommitmentRecord commitment() {
      return commitment;
    }

    public Location location() {
      return location;
    }
  }

  public static final class PinIntentWithLocation {
    private final PinIntent intent;
    private final Location location;

    public PinIntentWithLocation(final PinIntent intent, final Location location) {
      this.intent = Objects.requireNonNull(intent, "intent");
      this.location = Objects.requireNonNull(location, "location");
    }

    public PinIntent intent() {
      return intent;
    }

    public Location location() {
      return location;
    }
  }

  public static final class CommitmentListResponse {
    private final ProofPolicyBundle policies;
    private final List<CommitmentWithLocation> commitments;

    CommitmentListResponse(
        final ProofPolicyBundle policies,
        final List<CommitmentWithLocation> commitments) {
      this.policies = Objects.requireNonNull(policies, "policies");
      this.commitments =
          Collections.unmodifiableList(new ArrayList<>(commitments));
    }

    public ProofPolicyBundle policies() {
      return policies;
    }

    public List<CommitmentWithLocation> commitments() {
      return commitments;
    }
  }

  public static final class CommitmentProofResponse {
    private final ProofPolicyBundle policies;
    private final CommitmentProof proof;

    CommitmentProofResponse(
        final ProofPolicyBundle policies, final CommitmentProof proof) {
      this.policies = Objects.requireNonNull(policies, "policies");
      this.proof = Objects.requireNonNull(proof, "proof");
    }

    public ProofPolicyBundle policies() {
      return policies;
    }

    public CommitmentProof proof() {
      return proof;
    }
  }

  public static final class VerifyResponse {
    private final boolean valid;
    private final String error;

    VerifyResponse(final boolean valid, final String error) {
      if (valid != (error == null)) {
        throw new IllegalArgumentException(
            "DA verify response must omit errors only for valid proofs");
      }
      if (error != null && error.isEmpty()) {
        throw new IllegalArgumentException("DA verification error must be non-empty");
      }
      this.valid = valid;
      this.error = error;
    }

    public boolean valid() {
      return valid;
    }

    public String error() {
      return error;
    }
  }

  static void requireU32(final BigInteger value, final String field) {
    if (value == null
        || value.signum() < 0
        || value.compareTo(U32_MAX) > 0) {
      throw new IllegalArgumentException(field + " must fit u32");
    }
  }

  static void requireU64(final BigInteger value, final String field) {
    if (value == null
        || value.signum() < 0
        || value.compareTo(U64_MAX) > 0) {
      throw new IllegalArgumentException(field + " must fit u64");
    }
  }

  private static void requirePinIntentAlias(
      final String value, final String field) {
    if (value.getBytes(StandardCharsets.UTF_8).length > 256) {
      throw new IllegalArgumentException(
          field + " must contain at most 256 UTF-8 bytes");
    }
  }

  private static boolean merklePathMatchesLocation(
      final List<MerklePathItem> path,
      final long initialIndex,
      final long initialWidth) {
    if (initialWidth <= 0 || initialIndex < 0 || initialIndex >= initialWidth) {
      return false;
    }
    long index = initialIndex;
    long width = initialWidth;
    int pathIndex = 0;
    while (width > 1) {
      final MerkleDirection expected;
      if (index % 2 == 1) {
        expected = MerkleDirection.LEFT;
      } else if (index + 1 < width) {
        expected = MerkleDirection.RIGHT;
      } else {
        expected = null;
      }
      if (expected != null) {
        if (pathIndex >= path.size() || path.get(pathIndex).direction != expected) {
          return false;
        }
        pathIndex++;
      }
      index /= 2;
      width = width / 2 + width % 2;
    }
    return pathIndex == path.size();
  }
}

// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.consensus;

import java.io.ByteArrayOutputStream;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.charset.CharacterCodingException;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import java.util.function.Function;
import org.hyperledger.iroha.android.crypto.IrohaHash;

/** Canonical compact-length bare-Norito models for the Sumeragi v2 wire protocol. */
public final class SumeragiV2Wire {
  /** Live Sumeragi protocol revision. */
  public static final int PROTOCOL_VERSION = 3;
  /** Maximum number of real Kagemusha top-up leaves committed by one block. */
  public static final long MAX_KAGEMUSHA_TOPUP_ANCHORS_PER_BLOCK = 16;
  private static final byte[] KAGEMUSHA_TOPUP_POST_STATE_ROOT_DOMAIN =
      "iroha:kagemusha:v2:post-state-root".getBytes(StandardCharsets.UTF_8);

  private SumeragiV2Wire() {}

  /** Immutable 32-byte Iroha hash. */
  public static final class Hash32 {
    private final byte[] value;

    public Hash32(byte[] value) {
      require(value != null && value.length == 32, "Iroha hashes must contain 32 bytes");
      require((value[31] & 1) == 1, "Iroha hash low bit must be set");
      this.value = value.clone();
    }

    public byte[] bytes() {
      return value.clone();
    }

    @Override
    public boolean equals(Object other) {
      return other instanceof Hash32 && Arrays.equals(value, ((Hash32) other).value);
    }

    @Override
    public int hashCode() {
      return Arrays.hashCode(value);
    }
  }

  /** Arbitrary 32-byte protocol value without Iroha hash bit constraints. */
  public static final class Bytes32 {
    private final byte[] value;
    public Bytes32(byte[] value) {
      require(value != null && value.length == 32, "Protocol value must contain 32 bytes");
      this.value = value.clone();
    }
    public byte[] bytes() { return value.clone(); }
    @Override public boolean equals(Object other) {
      return other instanceof Bytes32 && Arrays.equals(value, ((Bytes32) other).value);
    }
    @Override public int hashCode() { return Arrays.hashCode(value); }
  }

  /** Exact bare-Norito payload of an Iroha `PeerId`. */
  public static final class PeerIdPayload {
    private final byte[] value;

    public PeerIdPayload(byte[] value) {
      require(value != null && value.length != 0, "PeerId payload must not be empty");
      this.value = value.clone();
    }

    public byte[] bytes() {
      return value.clone();
    }

    @Override
    public boolean equals(Object other) {
      return other instanceof PeerIdPayload
          && Arrays.equals(value, ((PeerIdPayload) other).value);
    }

    @Override
    public int hashCode() {
      return Arrays.hashCode(value);
    }
  }

  /** Canonical Norito representation of an Iroha chain identifier. */
  public static final class ChainId extends WireValue {
    public final String value;

    public ChainId(String value) {
      this.value = nonNull(value, "value");
      requireWellFormedUtf16(value, "chain ID");
    }

    @Override
    public byte[] encode() {
      return struct(string(value));
    }

    static ChainId decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      ChainId value = new ChainId(reader.field("chain ID value", SumeragiV2Wire::decodeString));
      reader.finish("chain ID");
      return value;
    }
  }

  /** Base equality contract for immutable wire values. */
  public abstract static class WireValue {
    public abstract byte[] encode();

    @Override
    public final boolean equals(Object other) {
      return other != null
          && getClass() == other.getClass()
          && Arrays.equals(encode(), ((WireValue) other).encode());
    }

    @Override
    public final int hashCode() {
      return 31 * getClass().hashCode() + Arrays.hashCode(encode());
    }
  }

  /** Typed identifier of a frozen height context. */
  public static final class HeightContextId extends WireValue {
    public final Hash32 hash;

    public HeightContextId(Hash32 hash) {
      this.hash = nonNull(hash, "hash");
    }

    @Override
    public byte[] encode() {
      return struct(hash.bytes());
    }

    static HeightContextId decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      HeightContextId value = new HeightContextId(new Hash32(reader.field("context hash", SumeragiV2Wire::decodeHash)));
      reader.finish("height context id");
      return value;
    }
  }

  /** Consensus round identity. */
  public static final class ConsensusRound extends WireValue {
    public final HeightContextId contextId;
    public final long height;
    public final long view;

    public ConsensusRound(HeightContextId contextId, long height, long view) {
      this.contextId = nonNull(contextId, "contextId");
      this.height = height;
      this.view = view;
    }

    @Override
    public byte[] encode() {
      return struct(contextId.encode(), u64(height), u64(view));
    }

    static ConsensusRound decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      ConsensusRound value =
          new ConsensusRound(
              reader.field("round context", HeightContextId::decode),
              reader.field("round height", SumeragiV2Wire::decodeU64),
              reader.field("round view", SumeragiV2Wire::decodeU64));
      reader.finish("round");
      return value;
    }
  }

  /** Global Prepare and Commit phases. */
  public enum GlobalPhase {
    PREPARE(1),
    COMMIT(2);

    public final long discriminant;

    GlobalPhase(long discriminant) {
      this.discriminant = discriminant;
    }

    byte[] encode() {
      return u32(discriminant);
    }

    static GlobalPhase decode(byte[] bytes) {
      long tag = decodeU32(bytes);
      for (GlobalPhase value : values()) {
        if (value.discriminant == tag) return value;
      }
      throw new IllegalArgumentException("Unknown Sumeragi v2 global phase: " + tag);
    }
  }

  /** Exact block and payload subject. */
  public static final class BlockSubject extends WireValue {
    public final Hash32 parentBlockHash;
    public final Hash32 blockHash;
    public final Hash32 payloadHash;

    public BlockSubject(Hash32 parentBlockHash, Hash32 blockHash, Hash32 payloadHash) {
      this.parentBlockHash = parentBlockHash;
      this.blockHash = nonNull(blockHash, "blockHash");
      this.payloadHash = nonNull(payloadHash, "payloadHash");
    }

    @Override
    public byte[] encode() {
      return struct(
          option(parentBlockHash == null ? null : parentBlockHash.bytes()),
          blockHash.bytes(),
          payloadHash.bytes());
    }

    static BlockSubject decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      BlockSubject value =
          new BlockSubject(
              reader.field("subject parent", payload -> decodeOption(payload, data -> new Hash32(decodeHash(data)))),
              new Hash32(reader.field("subject block", SumeragiV2Wire::decodeHash)),
              new Hash32(reader.field("subject payload", SumeragiV2Wire::decodeHash)));
      reader.finish("block subject");
      return value;
    }
  }

  /** Deterministic state-transition result authenticated by votes and certificates. */
  public static final class ExecutionCommitment extends WireValue {
    public final Hash32 parentStateRoot;
    public final Hash32 postStateRoot;
    public final Hash32 ordinaryWritesRoot;
    public final Hash32 topupAnchorRoot;
    public final long topupAnchorCount;
    public final Hash32 executedBlockWireHash;

    public ExecutionCommitment(
        Hash32 parentStateRoot,
        Hash32 postStateRoot,
        Hash32 ordinaryWritesRoot,
        Hash32 topupAnchorRoot,
        long topupAnchorCount,
        Hash32 executedBlockWireHash) {
      this.parentStateRoot = nonNull(parentStateRoot, "parentStateRoot");
      this.postStateRoot = nonNull(postStateRoot, "postStateRoot");
      this.ordinaryWritesRoot = nonNull(ordinaryWritesRoot, "ordinaryWritesRoot");
      requireU32(topupAnchorCount, "topupAnchorCount");
      this.topupAnchorRoot = topupAnchorRoot;
      this.topupAnchorCount = topupAnchorCount;
      this.executedBlockWireHash = nonNull(executedBlockWireHash, "executedBlockWireHash");
      if (topupAnchorCount == 0) {
        require(topupAnchorRoot == null, "zero top-up count must not carry an anchor root");
      } else {
        require(topupAnchorRoot != null, "non-zero top-up count requires an anchor root");
        require(
            topupAnchorCount <= MAX_KAGEMUSHA_TOPUP_ANCHORS_PER_BLOCK,
            "top-up anchor count exceeds the consensus bound");
        require(
            postStateRoot.equals(
                topupPostStateRoot(topupAnchorCount, ordinaryWritesRoot, topupAnchorRoot)),
            "post-state root does not bind the top-up anchor projection");
      }
    }

    /** Construct an execution commitment for a block with no Kagemusha top-ups. */
    public static ExecutionCommitment withoutTopups(
        Hash32 parentStateRoot,
        Hash32 postStateRoot,
        Hash32 ordinaryWritesRoot,
        Hash32 executedBlockWireHash) {
      return new ExecutionCommitment(
          parentStateRoot, postStateRoot, ordinaryWritesRoot, null, 0, executedBlockWireHash);
    }

    /** Derive the canonical post-state root for a non-empty top-up tree. */
    public static Hash32 topupPostStateRoot(
        long topupAnchorCount, Hash32 ordinaryWritesRoot, Hash32 topupAnchorRoot) {
      require(
          topupAnchorCount > 0
              && topupAnchorCount <= MAX_KAGEMUSHA_TOPUP_ANCHORS_PER_BLOCK,
          "top-up anchor count must fit the non-empty consensus bound");
      nonNull(ordinaryWritesRoot, "ordinaryWritesRoot");
      nonNull(topupAnchorRoot, "topupAnchorRoot");
      ByteArrayOutputStream preimage = new ByteArrayOutputStream();
      append(preimage, KAGEMUSHA_TOPUP_POST_STATE_ROOT_DOMAIN);
      preimage.write(0);
      append(preimage, u32(topupAnchorCount));
      append(preimage, ordinaryWritesRoot.bytes());
      append(preimage, topupAnchorRoot.bytes());
      return new Hash32(IrohaHash.prehash(preimage.toByteArray()));
    }

    @Override
    public byte[] encode() {
      return struct(
          parentStateRoot.bytes(),
          postStateRoot.bytes(),
          ordinaryWritesRoot.bytes(),
          option(topupAnchorRoot == null ? null : topupAnchorRoot.bytes()),
          u32(topupAnchorCount),
          executedBlockWireHash.bytes());
    }

    static ExecutionCommitment decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      ExecutionCommitment value =
          new ExecutionCommitment(
              new Hash32(reader.field("execution parent state", SumeragiV2Wire::decodeHash)),
              new Hash32(reader.field("execution post state", SumeragiV2Wire::decodeHash)),
              new Hash32(reader.field("execution ordinary writes", SumeragiV2Wire::decodeHash)),
              reader.field(
                  "execution top-up root",
                  payload -> decodeOption(payload, data -> new Hash32(decodeHash(data)))),
              reader.field("execution top-up count", SumeragiV2Wire::decodeU32),
              new Hash32(
                  reader.field("execution executed block wire hash", SumeragiV2Wire::decodeHash)));
      reader.finish("execution commitment");
      return value;
    }
  }

  /** Prepare or Commit vote. */
  public static final class Vote extends WireValue {
    public final ConsensusRound round;
    public final GlobalPhase phase;
    public final BlockSubject subject;
    public final ExecutionCommitment executionCommitment;
    public final long signer;
    private final byte[] signature;

    public Vote(
        ConsensusRound round,
        GlobalPhase phase,
        BlockSubject subject,
        ExecutionCommitment executionCommitment,
        long signer,
        byte[] signature) {
      this.round = nonNull(round, "round");
      this.phase = nonNull(phase, "phase");
      this.subject = nonNull(subject, "subject");
      this.executionCommitment = nonNull(executionCommitment, "executionCommitment");
      requireU32(signer, "signer");
      this.signer = signer;
      this.signature = copy(signature, "signature");
    }

    public byte[] signature() {
      return signature.clone();
    }

    @Override
    public byte[] encode() {
      return struct(
          round.encode(),
          phase.encode(),
          subject.encode(),
          executionCommitment.encode(),
          u32(signer),
          byteVector(signature));
    }

    static Vote decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      Vote value =
          new Vote(
              reader.field("vote round", ConsensusRound::decode),
              reader.field("vote phase", GlobalPhase::decode),
              reader.field("vote subject", BlockSubject::decode),
              reader.field("vote execution commitment", ExecutionCommitment::decode),
              reader.field("vote signer", SumeragiV2Wire::decodeU32),
              reader.field("vote signature", SumeragiV2Wire::decodeByteVector));
      reader.finish("vote");
      return value;
    }
  }

  /** Stable reference to a full quorum certificate. */
  public static final class QuorumCertificateRef extends WireValue {
    public final ConsensusRound round;
    public final GlobalPhase phase;
    public final BlockSubject subject;
    public final ExecutionCommitment executionCommitment;

    public QuorumCertificateRef(
        ConsensusRound round,
        GlobalPhase phase,
        BlockSubject subject,
        ExecutionCommitment executionCommitment) {
      this.round = nonNull(round, "round");
      this.phase = nonNull(phase, "phase");
      this.subject = nonNull(subject, "subject");
      this.executionCommitment = nonNull(executionCommitment, "executionCommitment");
    }

    @Override
    public byte[] encode() {
      return struct(round.encode(), phase.encode(), subject.encode(), executionCommitment.encode());
    }

    static QuorumCertificateRef decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      QuorumCertificateRef value =
          new QuorumCertificateRef(
              reader.field("qc ref round", ConsensusRound::decode),
              reader.field("qc ref phase", GlobalPhase::decode),
              reader.field("qc ref subject", BlockSubject::decode),
              reader.field("qc ref execution commitment", ExecutionCommitment::decode));
      reader.finish("quorum certificate ref");
      return value;
    }
  }

  /** Aggregate Prepare or Commit certificate. */
  public static final class QuorumCertificate extends WireValue {
    public final ConsensusRound round;
    public final GlobalPhase phase;
    public final BlockSubject subject;
    public final ExecutionCommitment executionCommitment;
    public final List<Long> signers;
    private final byte[] aggregateSignature;

    public QuorumCertificate(
        ConsensusRound round,
        GlobalPhase phase,
        BlockSubject subject,
        ExecutionCommitment executionCommitment,
        List<Long> signers,
        byte[] aggregateSignature) {
      this.round = nonNull(round, "round");
      this.phase = nonNull(phase, "phase");
      this.subject = nonNull(subject, "subject");
      this.executionCommitment = nonNull(executionCommitment, "executionCommitment");
      this.signers = immutable(signers, "signers");
      requireIncreasing(this.signers, "quorum certificate signers");
      this.aggregateSignature = copy(aggregateSignature, "aggregateSignature");
    }

    public byte[] aggregateSignature() {
      return aggregateSignature.clone();
    }

    public QuorumCertificateRef reference() {
      return new QuorumCertificateRef(round, phase, subject, executionCommitment);
    }

    @Override
    public byte[] encode() {
      return struct(
          round.encode(),
          phase.encode(),
          subject.encode(),
          executionCommitment.encode(),
          vector(signers, SumeragiV2Wire::u32),
          byteVector(aggregateSignature));
    }

    static QuorumCertificate decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      QuorumCertificate value =
          new QuorumCertificate(
              reader.field("qc round", ConsensusRound::decode),
              reader.field("qc phase", GlobalPhase::decode),
              reader.field("qc subject", BlockSubject::decode),
              reader.field("qc execution commitment", ExecutionCommitment::decode),
              reader.field("qc signers", data -> decodeVector(data, SumeragiV2Wire::decodeU32)),
              reader.field("qc signature", SumeragiV2Wire::decodeByteVector));
      reader.finish("quorum certificate");
      return value;
    }
  }

  /** One durable timeout vote. */
  public static final class TimeoutVote extends WireValue {
    public final ConsensusRound round;
    public final QuorumCertificate highestPrepareQc;
    public final long signer;
    private final byte[] signature;

    public TimeoutVote(
        ConsensusRound round, QuorumCertificate highestPrepareQc, long signer, byte[] signature) {
      this.round = nonNull(round, "round");
      this.highestPrepareQc = highestPrepareQc;
      requireU32(signer, "signer");
      this.signer = signer;
      this.signature = copy(signature, "signature");
    }

    public byte[] signature() {
      return signature.clone();
    }

    @Override
    public byte[] encode() {
      return struct(
          round.encode(),
          option(highestPrepareQc == null ? null : highestPrepareQc.encode()),
          u32(signer),
          byteVector(signature));
    }

    static TimeoutVote decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      TimeoutVote value =
          new TimeoutVote(
              reader.field("timeout vote round", ConsensusRound::decode),
              reader.field("timeout vote high qc", data -> decodeOption(data, QuorumCertificate::decode)),
              reader.field("timeout vote signer", SumeragiV2Wire::decodeU32),
              reader.field("timeout vote signature", SumeragiV2Wire::decodeByteVector));
      reader.finish("timeout vote");
      return value;
    }
  }

  /** Aggregate timeout signatures sharing one high QC. */
  public static final class TimeoutVoteGroup extends WireValue {
    public final QuorumCertificate highestPrepareQc;
    public final List<Long> signers;
    private final byte[] aggregateSignature;

    public TimeoutVoteGroup(
        QuorumCertificate highestPrepareQc, List<Long> signers, byte[] aggregateSignature) {
      this.highestPrepareQc = highestPrepareQc;
      this.signers = immutable(signers, "signers");
      require(!this.signers.isEmpty(), "timeout group must contain a signer");
      requireIncreasing(this.signers, "timeout group signers");
      this.aggregateSignature = copy(aggregateSignature, "aggregateSignature");
    }

    public byte[] aggregateSignature() {
      return aggregateSignature.clone();
    }

    @Override
    public byte[] encode() {
      return struct(
          option(highestPrepareQc == null ? null : highestPrepareQc.encode()),
          vector(signers, SumeragiV2Wire::u32),
          byteVector(aggregateSignature));
    }

    static TimeoutVoteGroup decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      TimeoutVoteGroup value =
          new TimeoutVoteGroup(
              reader.field("timeout group high qc", data -> decodeOption(data, QuorumCertificate::decode)),
              reader.field("timeout group signers", data -> decodeVector(data, SumeragiV2Wire::decodeU32)),
              reader.field("timeout group signature", SumeragiV2Wire::decodeByteVector));
      reader.finish("timeout vote group");
      return value;
    }
  }

  /** Certified transition out of one view. */
  public static final class TimeoutCertificate extends WireValue {
    public final ConsensusRound round;
    public final List<TimeoutVoteGroup> groups;

    public TimeoutCertificate(ConsensusRound round, List<TimeoutVoteGroup> groups) {
      this.round = nonNull(round, "round");
      this.groups = immutable(groups, "groups");
      require(!this.groups.isEmpty(), "timeout certificate must contain a group");
      Set<Long> seen = new HashSet<>();
      for (TimeoutVoteGroup group : this.groups) {
        for (Long signer : group.signers) {
          require(seen.add(signer), "timeout certificate signer groups overlap");
        }
      }
    }

    @Override
    public byte[] encode() {
      return struct(round.encode(), vector(groups, TimeoutVoteGroup::encode));
    }

    static TimeoutCertificate decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      TimeoutCertificate value =
          new TimeoutCertificate(
              reader.field("tc round", ConsensusRound::decode),
              reader.field("tc groups", data -> decodeVector(data, TimeoutVoteGroup::decode)));
      reader.finish("timeout certificate");
      return value;
    }
  }

  /** Stable reference to a timeout certificate. */
  public static final class TimeoutCertificateRef extends WireValue {
    public final ConsensusRound round;
    public final QuorumCertificateRef highestPrepareQc;
    public final Hash32 certificateHash;

    public TimeoutCertificateRef(
        ConsensusRound round, QuorumCertificateRef highestPrepareQc, Hash32 certificateHash) {
      this.round = nonNull(round, "round");
      this.highestPrepareQc = highestPrepareQc;
      this.certificateHash = nonNull(certificateHash, "certificateHash");
    }

    @Override
    public byte[] encode() {
      return struct(
          round.encode(),
          option(highestPrepareQc == null ? null : highestPrepareQc.encode()),
          certificateHash.bytes());
    }

    static TimeoutCertificateRef decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      TimeoutCertificateRef value =
          new TimeoutCertificateRef(
              reader.field("tc ref round", ConsensusRound::decode),
              reader.field("tc ref high qc", data -> decodeOption(data, QuorumCertificateRef::decode)),
              new Hash32(reader.field("tc ref hash", SumeragiV2Wire::decodeHash)));
      reader.finish("timeout certificate ref");
      return value;
    }
  }

  /** Parent CommitQC proposal justification. */
  public static final class ParentCommitJustification extends WireValue {
    public final QuorumCertificate certificate;

    public ParentCommitJustification(QuorumCertificate certificate) {
      this.certificate = certificate;
    }

    @Override
    public byte[] encode() {
      return struct(option(certificate == null ? null : certificate.encode()));
    }

    static ParentCommitJustification decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      ParentCommitJustification value =
          new ParentCommitJustification(
              reader.field("parent certificate", data -> decodeOption(data, QuorumCertificate::decode)));
      reader.finish("parent justification");
      return value;
    }
  }

  /** Later-view timeout proposal justification. */
  public static final class TimeoutJustification extends WireValue {
    public final TimeoutCertificate timeoutCertificate;
    public final QuorumCertificate highestPrepareQc;

    public TimeoutJustification(
        TimeoutCertificate timeoutCertificate, QuorumCertificate highestPrepareQc) {
      this.timeoutCertificate = nonNull(timeoutCertificate, "timeoutCertificate");
      this.highestPrepareQc = highestPrepareQc;
    }

    @Override
    public byte[] encode() {
      return struct(
          timeoutCertificate.encode(),
          option(highestPrepareQc == null ? null : highestPrepareQc.encode()));
    }

    static TimeoutJustification decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      TimeoutJustification value =
          new TimeoutJustification(
              reader.field("timeout justification tc", TimeoutCertificate::decode),
              reader.field("timeout justification high qc", data -> decodeOption(data, QuorumCertificate::decode)));
      reader.finish("timeout justification");
      return value;
    }
  }

  /** Proposal justification union. */
  public abstract static class ProposalJustification extends WireValue {
    public static final class Parent extends ProposalJustification {
      public final ParentCommitJustification value;

      public Parent(ParentCommitJustification value) {
        this.value = nonNull(value, "value");
      }

      @Override
      public byte[] encode() {
        return enumPayload(0, value.encode());
      }
    }

    public static final class Timeout extends ProposalJustification {
      public final TimeoutJustification value;

      public Timeout(TimeoutJustification value) {
        this.value = nonNull(value, "value");
      }

      @Override
      public byte[] encode() {
        return enumPayload(1, value.encode());
      }
    }

    static ProposalJustification decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      long tag = reader.u32("proposal justification");
      byte[] payload = reader.compactField("proposal justification payload");
      reader.finish("proposal justification");
      if (tag == 0) return new Parent(ParentCommitJustification.decode(payload));
      if (tag == 1) return new Timeout(TimeoutJustification.decode(payload));
      throw new IllegalArgumentException("Unknown proposal justification: " + tag);
    }
  }

  /** Deterministic payload encoding. */
  public enum PayloadEncoding {
    PLAIN(0),
    REED_SOLOMON_16(1);

    public final long discriminant;

    PayloadEncoding(long discriminant) {
      this.discriminant = discriminant;
    }

    byte[] encode() {
      return u32(discriminant);
    }

    static PayloadEncoding decode(byte[] bytes) {
      long tag = decodeU32(bytes);
      for (PayloadEncoding value : values()) {
        if (value.discriminant == tag) return value;
      }
      throw new IllegalArgumentException("Unknown payload encoding: " + tag);
    }
  }

  /** Payload chunking limits frozen for one height. */
  public static final class DataAvailabilityLayout extends WireValue {
    public final PayloadEncoding encoding;
    public final long chunkSizeBytes;
    public final int dataShards;
    public final int parityShards;
    public final long maxPayloadSizeBytes;
    public final long maxChunkCount;

    public DataAvailabilityLayout(
        PayloadEncoding encoding,
        long chunkSizeBytes,
        int dataShards,
        int parityShards,
        long maxPayloadSizeBytes,
        long maxChunkCount) {
      this.encoding = nonNull(encoding, "encoding");
      requireU32(chunkSizeBytes, "chunkSizeBytes");
      requireU16(dataShards, "dataShards");
      requireU16(parityShards, "parityShards");
      requireU32(maxChunkCount, "maxChunkCount");
      this.chunkSizeBytes = chunkSizeBytes;
      this.dataShards = dataShards;
      this.parityShards = parityShards;
      this.maxPayloadSizeBytes = maxPayloadSizeBytes;
      this.maxChunkCount = maxChunkCount;
    }

    @Override
    public byte[] encode() {
      return struct(
          encoding.encode(),
          u32(chunkSizeBytes),
          u16(dataShards),
          u16(parityShards),
          u64(maxPayloadSizeBytes),
          u32(maxChunkCount));
    }

    static DataAvailabilityLayout decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      DataAvailabilityLayout value =
          new DataAvailabilityLayout(
              reader.field("da encoding", PayloadEncoding::decode),
              reader.field("da chunk size", SumeragiV2Wire::decodeU32),
              reader.field("da data shards", SumeragiV2Wire::decodeU16),
              reader.field("da parity shards", SumeragiV2Wire::decodeU16),
              reader.field("da max payload", SumeragiV2Wire::decodeU64),
              reader.field("da max chunks", SumeragiV2Wire::decodeU32));
      reader.finish("data availability layout");
      return value;
    }
  }

  /** Manifest committing to one exact canonical body. */
  public static final class PayloadManifest extends WireValue {
    public final ConsensusRound round;
    public final BlockSubject subject;
    public final long payloadSizeBytes;
    public final DataAvailabilityLayout layout;
    public final List<Hash32> chunkHashes;
    public final Hash32 chunkRoot;

    public PayloadManifest(
        ConsensusRound round,
        BlockSubject subject,
        long payloadSizeBytes,
        DataAvailabilityLayout layout,
        List<Hash32> chunkHashes,
        Hash32 chunkRoot) {
      this.round = nonNull(round, "round");
      this.subject = nonNull(subject, "subject");
      this.payloadSizeBytes = payloadSizeBytes;
      this.layout = nonNull(layout, "layout");
      this.chunkHashes = immutable(chunkHashes, "chunkHashes");
      require(!this.chunkHashes.isEmpty(), "payload manifest must contain a chunk hash");
      this.chunkRoot = nonNull(chunkRoot, "chunkRoot");
    }

    @Override
    public byte[] encode() {
      return struct(
          round.encode(),
          subject.encode(),
          u64(payloadSizeBytes),
          layout.encode(),
          vector(chunkHashes, Hash32::bytes),
          chunkRoot.bytes());
    }

    static PayloadManifest decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      PayloadManifest value =
          new PayloadManifest(
              reader.field("manifest round", ConsensusRound::decode),
              reader.field("manifest subject", BlockSubject::decode),
              reader.field("manifest size", SumeragiV2Wire::decodeU64),
              reader.field("manifest layout", DataAvailabilityLayout::decode),
              reader.field("manifest hashes", data -> decodeVector(data, hash -> new Hash32(decodeHash(hash)))),
              new Hash32(reader.field("manifest root", SumeragiV2Wire::decodeHash)));
      reader.finish("payload manifest");
      return value;
    }
  }

  /** One authenticated encoded payload chunk. */
  public static final class PayloadChunk extends WireValue {
    public final Hash32 manifestHash;
    public final long index;
    private final byte[] bytes;
    public final long sender;
    private final byte[] signature;

    public PayloadChunk(
        Hash32 manifestHash, long index, byte[] bytes, long sender, byte[] signature) {
      this.manifestHash = nonNull(manifestHash, "manifestHash");
      requireU32(index, "index");
      requireU32(sender, "sender");
      this.index = index;
      this.bytes = copy(bytes, "bytes");
      this.sender = sender;
      this.signature = copy(signature, "signature");
    }

    public byte[] bytes() {
      return bytes.clone();
    }

    public byte[] signature() {
      return signature.clone();
    }

    @Override
    public byte[] encode() {
      return struct(
          manifestHash.bytes(), u32(index), byteVector(bytes), u32(sender), byteVector(signature));
    }

    static PayloadChunk decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      PayloadChunk value =
          new PayloadChunk(
              new Hash32(reader.field("chunk manifest", SumeragiV2Wire::decodeHash)),
              reader.field("chunk index", SumeragiV2Wire::decodeU32),
              reader.field("chunk bytes", SumeragiV2Wire::decodeByteVector),
              reader.field("chunk sender", SumeragiV2Wire::decodeU32),
              reader.field("chunk signature", SumeragiV2Wire::decodeByteVector));
      reader.finish("payload chunk");
      return value;
    }
  }

  /** Signed proposal. */
  public static final class Proposal extends WireValue {
    public final ConsensusRound round;
    public final long proposer;
    public final BlockSubject subject;
    public final PayloadManifest manifest;
    public final ProposalJustification justification;
    private final byte[] signature;

    public Proposal(
        ConsensusRound round,
        long proposer,
        BlockSubject subject,
        PayloadManifest manifest,
        ProposalJustification justification,
        byte[] signature) {
      this.round = nonNull(round, "round");
      requireU32(proposer, "proposer");
      this.proposer = proposer;
      this.subject = nonNull(subject, "subject");
      this.manifest = nonNull(manifest, "manifest");
      this.justification = nonNull(justification, "justification");
      this.signature = copy(signature, "signature");
    }

    public byte[] signature() {
      return signature.clone();
    }

    @Override
    public byte[] encode() {
      return struct(
          round.encode(),
          u32(proposer),
          subject.encode(),
          manifest.encode(),
          justification.encode(),
          byteVector(signature));
    }

    static Proposal decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      Proposal value =
          new Proposal(
              reader.field("proposal round", ConsensusRound::decode),
              reader.field("proposal proposer", SumeragiV2Wire::decodeU32),
              reader.field("proposal subject", BlockSubject::decode),
              reader.field("proposal manifest", PayloadManifest::decode),
              reader.field("proposal justification", ProposalJustification::decode),
              reader.field("proposal signature", SumeragiV2Wire::decodeByteVector));
      reader.finish("proposal");
      return value;
    }
  }

  /** Authenticated certified-body request. */
  public static final class CertifiedBodyRequest extends WireValue {
    public final ConsensusRound round;
    public final BlockSubject subject;
    public final QuorumCertificate certificate;
    public final PeerIdPayload requester;
    private final byte[] signature;

    public CertifiedBodyRequest(
        ConsensusRound round,
        BlockSubject subject,
        QuorumCertificate certificate,
        PeerIdPayload requester,
        byte[] signature) {
      this.round = nonNull(round, "round");
      this.subject = nonNull(subject, "subject");
      this.certificate = nonNull(certificate, "certificate");
      this.requester = nonNull(requester, "requester");
      this.signature = copy(signature, "signature");
    }

    public byte[] signature() {
      return signature.clone();
    }

    @Override
    public byte[] encode() {
      return struct(
          round.encode(), subject.encode(), certificate.encode(), requester.bytes(), byteVector(signature));
    }

    static CertifiedBodyRequest decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      CertifiedBodyRequest value =
          new CertifiedBodyRequest(
              reader.field("body request round", ConsensusRound::decode),
              reader.field("body request subject", BlockSubject::decode),
              reader.field("body request certificate", QuorumCertificate::decode),
              new PeerIdPayload(reader.compactField("body request requester")),
              reader.field("body request signature", SumeragiV2Wire::decodeByteVector));
      reader.finish("certified body request");
      return value;
    }
  }

  /** Certified body response. */
  public static final class CertifiedBodyResponse extends WireValue {
    public final Hash32 requestHash;
    public final PayloadManifest manifest;
    private final byte[] body;
    public final long responder;
    private final byte[] signature;

    public CertifiedBodyResponse(
        Hash32 requestHash,
        PayloadManifest manifest,
        byte[] body,
        long responder,
        byte[] signature) {
      this.requestHash = nonNull(requestHash, "requestHash");
      this.manifest = nonNull(manifest, "manifest");
      this.body = copy(body, "body");
      requireU32(responder, "responder");
      this.responder = responder;
      this.signature = copy(signature, "signature");
    }

    public byte[] body() {
      return body.clone();
    }

    public byte[] signature() {
      return signature.clone();
    }

    @Override
    public byte[] encode() {
      return struct(
          requestHash.bytes(), manifest.encode(), byteVector(body), u32(responder), byteVector(signature));
    }

    static CertifiedBodyResponse decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      CertifiedBodyResponse value =
          new CertifiedBodyResponse(
              new Hash32(reader.field("body response request hash", SumeragiV2Wire::decodeHash)),
              reader.field("body response manifest", PayloadManifest::decode),
              reader.field("body response body", SumeragiV2Wire::decodeByteVector),
              reader.field("body response responder", SumeragiV2Wire::decodeU32),
              reader.field("body response signature", SumeragiV2Wire::decodeByteVector));
      reader.finish("certified body response");
      return value;
    }
  }

  /** Signed request for the durable CommitQC of one exact height context. */
  public static final class CommitCertificateRequest extends WireValue {
    private static final byte[] SIGNATURE_DOMAIN =
        "iroha:sumeragi:v2:commit-certificate-request".getBytes(StandardCharsets.UTF_8);

    public final int protocolVersion;
    public final ChainId chainId;
    public final HeightContextId contextId;
    public final long height;
    public final PeerIdPayload requester;
    private final byte[] signature;

    public CommitCertificateRequest(
        ChainId chainId,
        HeightContextId contextId,
        long height,
        PeerIdPayload requester,
        byte[] signature) {
      this(PROTOCOL_VERSION, chainId, contextId, height, requester, signature);
    }

    public CommitCertificateRequest(
        int protocolVersion,
        ChainId chainId,
        HeightContextId contextId,
        long height,
        PeerIdPayload requester,
        byte[] signature) {
      require(
          protocolVersion == PROTOCOL_VERSION,
          "Unsupported commit-certificate request protocol version " + protocolVersion);
      this.protocolVersion = protocolVersion;
      this.chainId = nonNull(chainId, "chainId");
      this.contextId = nonNull(contextId, "contextId");
      this.height = height;
      this.requester = nonNull(requester, "requester");
      this.signature = copy(signature, "signature");
      require(this.signature.length != 0, "Commit-certificate request signature is missing");
    }

    public byte[] signature() {
      return signature.clone();
    }

    @Override
    public byte[] encode() {
      return struct(
          u16(protocolVersion),
          chainId.encode(),
          contextId.encode(),
          u64(height),
          requester.bytes(),
          byteVector(signature));
    }

    /** Exact domain-separated bytes authenticated by the requester. */
    public byte[] signaturePreimage() {
      return concat(
          SIGNATURE_DOMAIN,
          struct(
              u16(protocolVersion),
              chainId.encode(),
              contextId.encode(),
              u64(height),
              requester.bytes(),
              byteVector(new byte[0])));
    }

    /** Iroha hash identifying this exact signed request. */
    public Hash32 requestHash() {
      return new Hash32(IrohaHash.prehash(encode()));
    }

    static CommitCertificateRequest decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      CommitCertificateRequest value =
          new CommitCertificateRequest(
              reader.field("commit request protocol", SumeragiV2Wire::decodeU16),
              reader.field("commit request chain ID", ChainId::decode),
              reader.field("commit request context", HeightContextId::decode),
              reader.field("commit request height", SumeragiV2Wire::decodeU64),
              new PeerIdPayload(reader.compactField("commit request requester")),
              reader.field("commit request signature", SumeragiV2Wire::decodeByteVector));
      reader.finish("commit-certificate request");
      return value;
    }
  }

  /** Signed response carrying the durable CommitQC for an exact request. */
  public static final class CommitCertificateResponse extends WireValue {
    private static final byte[] SIGNATURE_DOMAIN =
        "iroha:sumeragi:v2:commit-certificate-response".getBytes(StandardCharsets.UTF_8);

    public final Hash32 requestHash;
    public final QuorumCertificate certificate;
    public final PeerIdPayload responder;
    private final byte[] signature;

    public CommitCertificateResponse(
        Hash32 requestHash,
        QuorumCertificate certificate,
        PeerIdPayload responder,
        byte[] signature) {
      this.requestHash = nonNull(requestHash, "requestHash");
      this.certificate = nonNull(certificate, "certificate");
      require(certificate.phase == GlobalPhase.COMMIT,
          "Commit-certificate response must carry a CommitQC");
      this.responder = nonNull(responder, "responder");
      this.signature = copy(signature, "signature");
      require(this.signature.length != 0, "Commit-certificate response signature is missing");
    }

    public byte[] signature() {
      return signature.clone();
    }

    @Override
    public byte[] encode() {
      return struct(
          requestHash.bytes(), certificate.encode(), responder.bytes(), byteVector(signature));
    }

    /** Exact domain-separated bytes authenticated by the responder. */
    public byte[] signaturePreimage() {
      return concat(
          SIGNATURE_DOMAIN,
          struct(
              u16(PROTOCOL_VERSION),
              requestHash.bytes(),
              certificate.encode(),
              responder.bytes()));
    }

    /**
     * Fail closed unless this response answers the exact request under its height context.
     * Responder and aggregate-signature verification remains the caller's responsibility.
     */
    public void validateAgainst(CommitCertificateRequest request) {
      nonNull(request, "request");
      require(
          requestHash.equals(request.requestHash()),
          "Commit-certificate response does not answer the exact signed request");
      require(
          certificate.round.contextId.equals(request.contextId),
          "Commit-certificate response uses a different height context");
      require(
          certificate.round.height == request.height,
          "Commit-certificate response uses a different height");
    }

    static CommitCertificateResponse decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      CommitCertificateResponse value =
          new CommitCertificateResponse(
              new Hash32(reader.field("commit response request hash", SumeragiV2Wire::decodeHash)),
              reader.field("commit response certificate", QuorumCertificate::decode),
              new PeerIdPayload(reader.compactField("commit response responder")),
              reader.field("commit response signature", SumeragiV2Wire::decodeByteVector));
      reader.finish("commit-certificate response");
      return value;
    }
  }

  /** Canonical network payload union, in Rust declaration order. */
  public abstract static class ConsensusPayload extends WireValue {
    public static final class ProposalMessage extends ConsensusPayload {
      public final Proposal value;
      public ProposalMessage(Proposal value) { this.value = nonNull(value, "value"); }
      @Override public byte[] encode() { return enumPayload(0, value.encode()); }
    }

    public static final class VoteMessage extends ConsensusPayload {
      public final Vote value;
      public VoteMessage(Vote value) { this.value = nonNull(value, "value"); }
      @Override public byte[] encode() { return enumPayload(1, value.encode()); }
    }

    public static final class QuorumCertificateMessage extends ConsensusPayload {
      public final QuorumCertificate value;
      public QuorumCertificateMessage(QuorumCertificate value) { this.value = nonNull(value, "value"); }
      @Override public byte[] encode() { return enumPayload(2, value.encode()); }
    }

    public static final class TimeoutVoteMessage extends ConsensusPayload {
      public final TimeoutVote value;
      public TimeoutVoteMessage(TimeoutVote value) { this.value = nonNull(value, "value"); }
      @Override public byte[] encode() { return enumPayload(3, value.encode()); }
    }

    public static final class TimeoutCertificateMessage extends ConsensusPayload {
      public final TimeoutCertificate value;
      public TimeoutCertificateMessage(TimeoutCertificate value) { this.value = nonNull(value, "value"); }
      @Override public byte[] encode() { return enumPayload(4, value.encode()); }
    }

    public static final class PayloadManifestMessage extends ConsensusPayload {
      public final PayloadManifest value;
      public PayloadManifestMessage(PayloadManifest value) { this.value = nonNull(value, "value"); }
      @Override public byte[] encode() { return enumPayload(5, value.encode()); }
    }

    public static final class PayloadChunkMessage extends ConsensusPayload {
      public final PayloadChunk value;
      public PayloadChunkMessage(PayloadChunk value) { this.value = nonNull(value, "value"); }
      @Override public byte[] encode() { return enumPayload(6, value.encode()); }
    }

    public static final class CertifiedBodyRequestMessage extends ConsensusPayload {
      public final CertifiedBodyRequest value;
      public CertifiedBodyRequestMessage(CertifiedBodyRequest value) { this.value = nonNull(value, "value"); }
      @Override public byte[] encode() { return enumPayload(7, value.encode()); }
    }

    public static final class CertifiedBodyResponseMessage extends ConsensusPayload {
      public final CertifiedBodyResponse value;
      public CertifiedBodyResponseMessage(CertifiedBodyResponse value) { this.value = nonNull(value, "value"); }
      @Override public byte[] encode() { return enumPayload(8, value.encode()); }
    }

    public static final class CommitCertificateRequestMessage extends ConsensusPayload {
      public final CommitCertificateRequest value;
      public CommitCertificateRequestMessage(CommitCertificateRequest value) {
        this.value = nonNull(value, "value");
      }
      @Override public byte[] encode() { return enumPayload(9, value.encode()); }
    }

    public static final class CommitCertificateResponseMessage extends ConsensusPayload {
      public final CommitCertificateResponse value;
      public CommitCertificateResponseMessage(CommitCertificateResponse value) {
        this.value = nonNull(value, "value");
      }
      @Override public byte[] encode() { return enumPayload(10, value.encode()); }
    }

    static ConsensusPayload decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      long tag = reader.u32("consensus payload");
      byte[] payload = reader.compactField("consensus payload value");
      reader.finish("consensus payload");
      switch ((int) tag) {
        case 0: return new ProposalMessage(Proposal.decode(payload));
        case 1: return new VoteMessage(Vote.decode(payload));
        case 2: return new QuorumCertificateMessage(QuorumCertificate.decode(payload));
        case 3: return new TimeoutVoteMessage(TimeoutVote.decode(payload));
        case 4: return new TimeoutCertificateMessage(TimeoutCertificate.decode(payload));
        case 5: return new PayloadManifestMessage(PayloadManifest.decode(payload));
        case 6: return new PayloadChunkMessage(PayloadChunk.decode(payload));
        case 7: return new CertifiedBodyRequestMessage(CertifiedBodyRequest.decode(payload));
        case 8: return new CertifiedBodyResponseMessage(CertifiedBodyResponse.decode(payload));
        case 9: return new CommitCertificateRequestMessage(CommitCertificateRequest.decode(payload));
        case 10: return new CommitCertificateResponseMessage(CommitCertificateResponse.decode(payload));
        default: throw new IllegalArgumentException("Unknown Sumeragi v2 payload: " + tag);
      }
    }
  }

  /** Explicitly versioned live-consensus envelope. */
  public static final class ConsensusMessageV2 extends WireValue {
    public final int protocolVersion;
    public final ConsensusPayload payload;

    public ConsensusMessageV2(ConsensusPayload payload) {
      this(PROTOCOL_VERSION, payload);
    }

    public ConsensusMessageV2(int protocolVersion, ConsensusPayload payload) {
      require(protocolVersion == PROTOCOL_VERSION,
          "Unsupported Sumeragi protocol version " + protocolVersion);
      this.protocolVersion = protocolVersion;
      this.payload = nonNull(payload, "payload");
    }

    @Override
    public byte[] encode() {
      return struct(u16(protocolVersion), payload.encode());
    }

    /** Decode a canonical compact-length bare-Norito envelope and reject v1/live mismatch. */
    public static ConsensusMessageV2 decodeCanonical(byte[] bytes) {
      Reader reader = new Reader(bytes);
      ConsensusMessageV2 value =
          new ConsensusMessageV2(
              reader.field("message protocol", SumeragiV2Wire::decodeU16),
              reader.field("message payload", ConsensusPayload::decode));
      reader.finish("consensus message");
      require(Arrays.equals(bytes, value.encode()), "ConsensusMessageV2 is not canonical");
      return value;
    }
  }

  /** Compact status reducer phase. */
  public enum StatusPhase {
    AWAITING_PROPOSAL(0),
    RECONSTRUCTING_PAYLOAD(1),
    VALIDATING_PAYLOAD(2),
    PREPARE(3),
    COMMIT(4),
    PENDING_APPLY(5);

    public final long discriminant;
    StatusPhase(long discriminant) { this.discriminant = discriminant; }
    byte[] encode() { return u32(discriminant); }
    static StatusPhase decode(byte[] bytes) {
      long tag = decodeU32(bytes);
      for (StatusPhase value : values()) if (value.discriminant == tag) return value;
      throw new IllegalArgumentException("Unknown Sumeragi v2 status phase: " + tag);
    }
  }

  /** Compact status body state. */
  public enum BodyState {
    MISSING(0),
    RECONSTRUCTING(1),
    STORED(2),
    VALIDATED(3),
    PENDING_APPLY(4),
    APPLIED(5);

    public final long discriminant;
    BodyState(long discriminant) { this.discriminant = discriminant; }
    byte[] encode() { return u32(discriminant); }
    static BodyState decode(byte[] bytes) {
      long tag = decodeU32(bytes);
      for (BodyState value : values()) if (value.discriminant == tag) return value;
      throw new IllegalArgumentException("Unknown Sumeragi v2 body state: " + tag);
    }
  }

  /** Consensus mode frozen in the status height context. */
  public enum ConsensusMode {
    PERMISSIONED(0), NPOS(1);
    public final long discriminant;
    ConsensusMode(long discriminant) { this.discriminant = discriminant; }
    byte[] encode() { return u32(discriminant); }
    static ConsensusMode decode(byte[] bytes) {
      long tag = decodeU32(bytes);
      for (ConsensusMode value : values()) if (value.discriminant == tag) return value;
      throw new IllegalArgumentException("Unknown Sumeragi v2 consensus mode: " + tag);
    }
  }

  /** Canonical count-and-power quorum frozen in a status context. */
  public static final class DualQuorum extends WireValue {
    public final long minSigners;
    public final long totalPower;
    public DualQuorum(long minSigners, long totalPower) {
      requireU32(minSigners, "minSigners");
      this.minSigners = minSigners;
      this.totalPower = totalPower;
    }
    @Override public byte[] encode() { return struct(u32(minSigners), u64(totalPower)); }
    static DualQuorum decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      DualQuorum value = new DualQuorum(
          reader.field("status quorum min signers", SumeragiV2Wire::decodeU32),
          reader.field("status quorum total power", SumeragiV2Wire::decodeU64));
      reader.finish("status dual quorum");
      return value;
    }
  }

  /** Frozen election context accompanying authoritative v2 status. */
  public static final class HeightContextStatus extends WireValue {
    public final long epoch;
    public final long epochEndHeight;
    public final ConsensusMode mode;
    public final Bytes32 epochSeed;
    public final long validatorCount;
    public final DualQuorum quorum;
    public HeightContextStatus(long epoch, long epochEndHeight, ConsensusMode mode,
        Bytes32 epochSeed, long validatorCount, DualQuorum quorum) {
      requireU32(validatorCount, "validatorCount");
      this.epoch = epoch;
      this.epochEndHeight = epochEndHeight;
      this.mode = nonNull(mode, "mode");
      this.epochSeed = nonNull(epochSeed, "epochSeed");
      this.validatorCount = validatorCount;
      this.quorum = nonNull(quorum, "quorum");
    }
    @Override public byte[] encode() {
      return struct(u64(epoch), u64(epochEndHeight), mode.encode(), epochSeed.bytes(),
          u32(validatorCount), quorum.encode());
    }
    static HeightContextStatus decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      HeightContextStatus value = new HeightContextStatus(
          reader.field("status context epoch", SumeragiV2Wire::decodeU64),
          reader.field("status context epoch end", SumeragiV2Wire::decodeU64),
          reader.field("status context mode", ConsensusMode::decode),
          new Bytes32(reader.field("status context epoch seed", SumeragiV2Wire::decodeHash)),
          reader.field("status context validator count", SumeragiV2Wire::decodeU32),
          reader.field("status context quorum", DualQuorum::decode));
      reader.finish("status height context");
      return value;
    }
  }

  /** Power-aware summary of the latest durable CommitQC. */
  public static final class CommitQcStatus extends WireValue {
    public final QuorumCertificateRef certificate;
    public final long validatorCount;
    public final long signerCount;
    public final long minSigners;
    public final long signedPower;
    public final long totalPower;
    public CommitQcStatus(QuorumCertificateRef certificate, long validatorCount,
        long signerCount, long minSigners, long signedPower, long totalPower) {
      requireU32(validatorCount, "validatorCount");
      requireU32(signerCount, "signerCount");
      requireU32(minSigners, "minSigners");
      this.certificate = nonNull(certificate, "certificate");
      this.validatorCount = validatorCount;
      this.signerCount = signerCount;
      this.minSigners = minSigners;
      this.signedPower = signedPower;
      this.totalPower = totalPower;
    }
    @Override public byte[] encode() {
      return struct(certificate.encode(), u32(validatorCount), u32(signerCount),
          u32(minSigners), u64(signedPower), u64(totalPower));
    }
    static CommitQcStatus decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      CommitQcStatus value = new CommitQcStatus(
          reader.field("status commit certificate", QuorumCertificateRef::decode),
          reader.field("status commit validator count", SumeragiV2Wire::decodeU32),
          reader.field("status commit signer count", SumeragiV2Wire::decodeU32),
          reader.field("status commit min signers", SumeragiV2Wire::decodeU32),
          reader.field("status commit signed power", SumeragiV2Wire::decodeU64),
          reader.field("status commit total power", SumeragiV2Wire::decodeU64));
      reader.finish("status commit QC");
      return value;
    }
  }

  /** Partial dual-quorum state for one exact proposal round. */
  public static final class VoteQuorumStatus extends WireValue {
    public final ConsensusRound round;
    public final BlockSubject subject;
    public final ExecutionCommitment executionCommitment;
    public final long signerCount;
    public final long signedPower;
    public final long minSigners;
    public final long totalPower;
    public VoteQuorumStatus(ConsensusRound round, BlockSubject subject,
        ExecutionCommitment executionCommitment, long signerCount, long signedPower,
        long minSigners, long totalPower) {
      requireU32(signerCount, "signerCount");
      requireU32(minSigners, "minSigners");
      this.round = nonNull(round, "round");
      this.subject = nonNull(subject, "subject");
      this.executionCommitment = nonNull(executionCommitment, "executionCommitment");
      this.signerCount = signerCount;
      this.signedPower = signedPower;
      this.minSigners = minSigners;
      this.totalPower = totalPower;
    }
    @Override public byte[] encode() {
      return struct(round.encode(), subject.encode(), executionCommitment.encode(),
          u32(signerCount), u64(signedPower), u32(minSigners), u64(totalPower));
    }
    static VoteQuorumStatus decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      VoteQuorumStatus value = new VoteQuorumStatus(
          reader.field("liveness vote round", ConsensusRound::decode),
          reader.field("liveness vote subject", BlockSubject::decode),
          reader.field("liveness vote execution", ExecutionCommitment::decode),
          reader.field("liveness vote signer count", SumeragiV2Wire::decodeU32),
          reader.field("liveness vote signed power", SumeragiV2Wire::decodeU64),
          reader.field("liveness vote min signers", SumeragiV2Wire::decodeU32),
          reader.field("liveness vote total power", SumeragiV2Wire::decodeU64));
      reader.finish("liveness vote quorum");
      return value;
    }
  }

  /** Partial timeout quorum state for one exact round. */
  public static final class TimeoutQuorumStatus extends WireValue {
    public final ConsensusRound round;
    public final long signerCount;
    public final long signedPower;
    public final long minSigners;
    public final long totalPower;
    public final boolean certificateFormed;
    public TimeoutQuorumStatus(ConsensusRound round, long signerCount, long signedPower,
        long minSigners, long totalPower, boolean certificateFormed) {
      requireU32(signerCount, "signerCount");
      requireU32(minSigners, "minSigners");
      this.round = nonNull(round, "round");
      this.signerCount = signerCount;
      this.signedPower = signedPower;
      this.minSigners = minSigners;
      this.totalPower = totalPower;
      this.certificateFormed = certificateFormed;
    }
    @Override public byte[] encode() {
      return struct(round.encode(), u32(signerCount), u64(signedPower), u32(minSigners),
          u64(totalPower), bool(certificateFormed));
    }
    static TimeoutQuorumStatus decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      TimeoutQuorumStatus value = new TimeoutQuorumStatus(
          reader.field("liveness timeout round", ConsensusRound::decode),
          reader.field("liveness timeout signer count", SumeragiV2Wire::decodeU32),
          reader.field("liveness timeout signed power", SumeragiV2Wire::decodeU64),
          reader.field("liveness timeout min signers", SumeragiV2Wire::decodeU32),
          reader.field("liveness timeout total power", SumeragiV2Wire::decodeU64),
          reader.field("liveness timeout formed", SumeragiV2Wire::decodeBool));
      reader.finish("liveness timeout quorum");
      return value;
    }
  }

  /** Durable outbound protocol role retained for fair service. */
  public enum OutboundIntentKind {
    PROPOSAL(0), PREPARE_VOTE(1), COMMIT_VOTE(2), PREPARE_QC(3), COMMIT_QC(4),
    TIMEOUT_VOTE(5), TIMEOUT_CERTIFICATE(6);
    public final long discriminant;
    OutboundIntentKind(long discriminant) { this.discriminant = discriminant; }
    byte[] encode() { return u32(discriminant); }
    static OutboundIntentKind decode(byte[] bytes) {
      long tag = decodeU32(bytes);
      for (OutboundIntentKind value : values()) if (value.discriminant == tag) return value;
      throw new IllegalArgumentException("Unknown outbound intent kind: " + tag);
    }
  }

  /** Current delivery stage of a durable outbound intent. */
  public enum OutboundIntentStage {
    PENDING_PERSISTENCE(0), PENDING_SIGNATURE(1), QUEUED(2), SENT(3);
    public final long discriminant;
    OutboundIntentStage(long discriminant) { this.discriminant = discriminant; }
    byte[] encode() { return u32(discriminant); }
    static OutboundIntentStage decode(byte[] bytes) {
      long tag = decodeU32(bytes);
      for (OutboundIntentStage value : values()) if (value.discriminant == tag) return value;
      throw new IllegalArgumentException("Unknown outbound intent stage: " + tag);
    }
  }

  /** Exact durable outbound intent visible to liveness diagnostics. */
  public static final class OutboundIntentStatus extends WireValue {
    public final OutboundIntentKind kind;
    public final ConsensusRound round;
    public final BlockSubject subject;
    public final ExecutionCommitment executionCommitment;
    public final OutboundIntentStage stage;
    public OutboundIntentStatus(OutboundIntentKind kind, ConsensusRound round,
        BlockSubject subject, ExecutionCommitment executionCommitment, OutboundIntentStage stage) {
      this.kind = nonNull(kind, "kind");
      this.round = nonNull(round, "round");
      this.subject = subject;
      this.executionCommitment = executionCommitment;
      this.stage = nonNull(stage, "stage");
    }
    @Override public byte[] encode() {
      return struct(kind.encode(), round.encode(), option(subject == null ? null : subject.encode()),
          option(executionCommitment == null ? null : executionCommitment.encode()), stage.encode());
    }
    static OutboundIntentStatus decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      OutboundIntentStatus value = new OutboundIntentStatus(
          reader.field("liveness outbound kind", OutboundIntentKind::decode),
          reader.field("liveness outbound round", ConsensusRound::decode),
          reader.field("liveness outbound subject", data -> decodeOption(data, BlockSubject::decode)),
          reader.field("liveness outbound execution", data -> decodeOption(data, ExecutionCommitment::decode)),
          reader.field("liveness outbound stage", OutboundIntentStage::decode));
      reader.finish("liveness outbound intent");
      return value;
    }
  }

  /** State of one terminating local-work stage. */
  public enum LocalWorkStage {
    IDLE(0), QUEUED(1), RUNNING(2), COMPLETE(3);
    public final long discriminant;
    LocalWorkStage(long discriminant) { this.discriminant = discriminant; }
    byte[] encode() { return u32(discriminant); }
    static LocalWorkStage decode(byte[] bytes) {
      long tag = decodeU32(bytes);
      for (LocalWorkStage value : values()) if (value.discriminant == tag) return value;
      throw new IllegalArgumentException("Unknown local work stage: " + tag);
    }
  }

  /** Local body, validation, application, and handoff pipeline. */
  public static final class WorkStatus extends WireValue {
    public final LocalWorkStage candidate;
    public final LocalWorkStage bodyRecovery;
    public final LocalWorkStage bodyStore;
    public final LocalWorkStage validation;
    public final LocalWorkStage application;
    public final LocalWorkStage successorHeight;
    public WorkStatus(LocalWorkStage candidate, LocalWorkStage bodyRecovery,
        LocalWorkStage bodyStore, LocalWorkStage validation, LocalWorkStage application,
        LocalWorkStage successorHeight) {
      this.candidate = nonNull(candidate, "candidate");
      this.bodyRecovery = nonNull(bodyRecovery, "bodyRecovery");
      this.bodyStore = nonNull(bodyStore, "bodyStore");
      this.validation = nonNull(validation, "validation");
      this.application = nonNull(application, "application");
      this.successorHeight = nonNull(successorHeight, "successorHeight");
    }
    @Override public byte[] encode() {
      return struct(candidate.encode(), bodyRecovery.encode(), bodyStore.encode(),
          validation.encode(), application.encode(), successorHeight.encode());
    }
    static WorkStatus decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      WorkStatus value = new WorkStatus(
          reader.field("liveness candidate work", LocalWorkStage::decode),
          reader.field("liveness recovery work", LocalWorkStage::decode),
          reader.field("liveness store work", LocalWorkStage::decode),
          reader.field("liveness validation work", LocalWorkStage::decode),
          reader.field("liveness application work", LocalWorkStage::decode),
          reader.field("liveness successor work", LocalWorkStage::decode));
      reader.finish("liveness work");
      return value;
    }
  }

  /** Identity of a bounded local progress queue. */
  public enum QueueKind {
    INGRESS(0), DEFERRED_NORMAL(1), DEFERRED_PROGRESS(2), DEFERRED_COMPLETION(3),
    RUNTIME_NORMAL(4), RUNTIME_PROGRESS(5), RUNTIME_COMPLETION(6), EFFECT_COMPLETION(7),
    NETWORK_INGRESS(8);
    public final long discriminant;
    QueueKind(long discriminant) { this.discriminant = discriminant; }
    byte[] encode() { return u32(discriminant); }
    static QueueKind decode(byte[] bytes) {
      long tag = decodeU32(bytes);
      for (QueueKind value : values()) if (value.discriminant == tag) return value;
      throw new IllegalArgumentException("Unknown liveness queue kind: " + tag);
    }
  }

  /** Occupancy and accumulated oldest-item service debt for one bounded queue. */
  public static final class QueueStatus extends WireValue {
    public final QueueKind queue;
    public final long depth;
    public final long capacity;
    public final Long oldestAgeMs;
    public final long serviceDebt;
    public QueueStatus(QueueKind queue, long depth, long capacity, Long oldestAgeMs,
        long serviceDebt) {
      requireU32(depth, "depth");
      requireU32(capacity, "capacity");
      this.queue = nonNull(queue, "queue");
      this.depth = depth;
      this.capacity = capacity;
      this.oldestAgeMs = oldestAgeMs;
      this.serviceDebt = serviceDebt;
    }
    @Override public byte[] encode() {
      return struct(queue.encode(), u32(depth), u32(capacity),
          option(oldestAgeMs == null ? null : u64(oldestAgeMs)), u64(serviceDebt));
    }
    static QueueStatus decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      QueueStatus value = new QueueStatus(
          reader.field("liveness queue kind", QueueKind::decode),
          reader.field("liveness queue depth", SumeragiV2Wire::decodeU32),
          reader.field("liveness queue capacity", SumeragiV2Wire::decodeU32),
          reader.field("liveness queue oldest age", data -> decodeOption(data, SumeragiV2Wire::decodeU64)),
          reader.field("liveness queue service debt", SumeragiV2Wire::decodeU64));
      reader.finish("liveness queue");
      return value;
    }
  }

  /** Diagnostic reducer transition; timeout churn does not reset height-level no-progress age. */
  public enum ProgressTransition {
    PROPOSAL_ADMITTED(0), BODY_AVAILABLE(1), BODY_STORED(2), BODY_VALIDATED(3),
    PREPARE_VOTE_ADMITTED(4), COMMIT_VOTE_ADMITTED(5), TIMEOUT_VOTE_ADMITTED(6),
    PREPARE_QUORUM(7), LOCK_INSTALLED(8), COMMIT_QUORUM(9),
    TIMEOUT_CERTIFICATE_INSTALLED(10), DECISION_PERSISTED(11), APPLIED(12),
    SUCCESSOR_HEIGHT_ACTIVATED(13), RECOVERY_REPLAYED(14);
    public final long discriminant;
    ProgressTransition(long discriminant) { this.discriminant = discriminant; }
    byte[] encode() { return u32(discriminant); }
    static ProgressTransition decode(byte[] bytes) {
      long tag = decodeU32(bytes);
      for (ProgressTransition value : values()) if (value.discriminant == tag) return value;
      throw new IllegalArgumentException("Unknown progress transition: " + tag);
    }
  }

  /** Last tracked reducer transition and its local age. */
  public static final class ProgressTransitionStatus extends WireValue {
    public final long generation;
    public final ConsensusRound round;
    public final ProgressTransition transition;
    public final long ageMs;
    public ProgressTransitionStatus(long generation, ConsensusRound round,
        ProgressTransition transition, long ageMs) {
      this.generation = generation;
      this.round = nonNull(round, "round");
      this.transition = nonNull(transition, "transition");
      this.ageMs = ageMs;
    }
    @Override public byte[] encode() {
      return struct(u64(generation), round.encode(), transition.encode(), u64(ageMs));
    }
    static ProgressTransitionStatus decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      ProgressTransitionStatus value = new ProgressTransitionStatus(
          reader.field("liveness progress generation", SumeragiV2Wire::decodeU64),
          reader.field("liveness progress round", ConsensusRound::decode),
          reader.field("liveness progress transition", ProgressTransition::decode),
          reader.field("liveness progress age", SumeragiV2Wire::decodeU64));
      reader.finish("liveness progress");
      return value;
    }
  }

  /** Classified cause of an active no-progress interval. */
  public enum LivenessBlocker {
    MISSING_PROPOSAL(0), BODY_UNAVAILABLE(1), PREPARE_QUORUM_MISSING(2),
    COMMIT_QUORUM_MISSING(3), TIMEOUT_CERTIFICATE_MISSING(4),
    SCHEDULER_STARVATION(5), APPLICATION_PENDING(6);
    public final long discriminant;
    LivenessBlocker(long discriminant) { this.discriminant = discriminant; }
    byte[] encode() { return u32(discriminant); }
    static LivenessBlocker decode(byte[] bytes) {
      long tag = decodeU32(bytes);
      for (LivenessBlocker value : values()) if (value.discriminant == tag) return value;
      throw new IllegalArgumentException("Unknown liveness blocker: " + tag);
    }
  }

  /** Closed reducer reason for safely ignoring an input. */
  public enum IgnoreReason {
    WRONG_HEIGHT(0), WRONG_VIEW(1), STALE_GENERATION(2), BUSY(3), DUPLICATE(4),
    NO_MATCHING_WORK(5), OBSERVER(6), VIEW_CLOSED(7), ALREADY_DECIDED(8),
    RECOVERY_PENDING(9), IRRELEVANT_VIEW(10);
    public final long discriminant;
    IgnoreReason(long discriminant) { this.discriminant = discriminant; }
    byte[] encode() { return u32(discriminant); }
    static IgnoreReason decode(byte[] bytes) {
      long tag = decodeU32(bytes);
      for (IgnoreReason value : values()) if (value.discriminant == tag) return value;
      throw new IllegalArgumentException("Unknown liveness ignore reason: " + tag);
    }
  }

  /** Per-height counter for one input-ignore reason. */
  public static final class IgnoreCount extends WireValue {
    public final IgnoreReason reason;
    public final long count;
    public IgnoreCount(IgnoreReason reason, long count) {
      this.reason = nonNull(reason, "reason");
      this.count = count;
    }
    @Override public byte[] encode() { return struct(reason.encode(), u64(count)); }
    static IgnoreCount decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      IgnoreCount value = new IgnoreCount(
          reader.field("liveness ignore reason", IgnoreReason::decode),
          reader.field("liveness ignore count", SumeragiV2Wire::decodeU64));
      reader.finish("liveness ignore count");
      return value;
    }
  }

  /** Authoritative progress diagnostics for the active height. */
  public static final class LivenessStatus extends WireValue {
    public final long generation;
    public final List<VoteQuorumStatus> prepareQuorums;
    public final List<VoteQuorumStatus> commitQuorums;
    public final List<TimeoutQuorumStatus> timeoutQuorums;
    public final List<OutboundIntentStatus> outboundIntents;
    public final WorkStatus work;
    public final List<QueueStatus> queues;
    public final ProgressTransitionStatus lastProgress;
    public final long noProgressAgeMs;
    public final LivenessBlocker blocker;
    public final List<IgnoreCount> ignoreCounts;
    public LivenessStatus(long generation, List<VoteQuorumStatus> prepareQuorums,
        List<VoteQuorumStatus> commitQuorums, List<TimeoutQuorumStatus> timeoutQuorums,
        List<OutboundIntentStatus> outboundIntents, WorkStatus work, List<QueueStatus> queues,
        ProgressTransitionStatus lastProgress, long noProgressAgeMs, LivenessBlocker blocker,
        List<IgnoreCount> ignoreCounts) {
      this.generation = generation;
      this.prepareQuorums = immutable(prepareQuorums, "prepareQuorums");
      this.commitQuorums = immutable(commitQuorums, "commitQuorums");
      this.timeoutQuorums = immutable(timeoutQuorums, "timeoutQuorums");
      this.outboundIntents = immutable(outboundIntents, "outboundIntents");
      this.work = nonNull(work, "work");
      this.queues = immutable(queues, "queues");
      this.lastProgress = lastProgress;
      this.noProgressAgeMs = noProgressAgeMs;
      this.blocker = blocker;
      this.ignoreCounts = immutable(ignoreCounts, "ignoreCounts");
    }
    @Override public byte[] encode() {
      return struct(u64(generation), vector(prepareQuorums, VoteQuorumStatus::encode),
          vector(commitQuorums, VoteQuorumStatus::encode),
          vector(timeoutQuorums, TimeoutQuorumStatus::encode),
          vector(outboundIntents, OutboundIntentStatus::encode), work.encode(),
          vector(queues, QueueStatus::encode),
          option(lastProgress == null ? null : lastProgress.encode()), u64(noProgressAgeMs),
          option(blocker == null ? null : blocker.encode()), vector(ignoreCounts, IgnoreCount::encode));
    }
    static LivenessStatus decode(byte[] bytes) {
      Reader reader = new Reader(bytes);
      LivenessStatus value = new LivenessStatus(
          reader.field("liveness generation", SumeragiV2Wire::decodeU64),
          reader.field("liveness prepare", data -> decodeVector(data, VoteQuorumStatus::decode)),
          reader.field("liveness commit", data -> decodeVector(data, VoteQuorumStatus::decode)),
          reader.field("liveness timeout", data -> decodeVector(data, TimeoutQuorumStatus::decode)),
          reader.field("liveness outbound", data -> decodeVector(data, OutboundIntentStatus::decode)),
          reader.field("liveness work", WorkStatus::decode),
          reader.field("liveness queues", data -> decodeVector(data, QueueStatus::decode)),
          reader.field("liveness last progress", data -> decodeOption(data, ProgressTransitionStatus::decode)),
          reader.field("liveness no progress age", SumeragiV2Wire::decodeU64),
          reader.field("liveness blocker", data -> decodeOption(data, LivenessBlocker::decode)),
          reader.field("liveness ignore counts", data -> decodeVector(data, IgnoreCount::decode)));
      reader.finish("liveness status");
      return value;
    }
  }

  /** Compact protocol-v2-only `/v1/sumeragi/status` payload. */
  public static final class SumeragiV2Status extends WireValue {
    public final int protocolVersion;
    public final Hash32 nodeFingerprint;
    public final Hash32 buildFingerprint;
    public final Hash32 configFingerprint;
    public final boolean restartRequired;
    public final HeightContextId heightContextId;
    public final long height;
    public final long view;
    public final StatusPhase phase;
    public final long leader;
    public final QuorumCertificateRef lockedPrepareQc;
    public final QuorumCertificateRef highestPrepareQc;
    public final TimeoutCertificateRef lastTimeoutCertificate;
    public final BodyState bodyState;
    public final Long pendingPersistenceId;
    public final long lastCommittedHeight;
    public final BlockSubject lastCommittedSubject;
    public final HeightContextStatus heightContext;
    public final CommitQcStatus lastCommitQc;
    public final LivenessStatus liveness;

    public SumeragiV2Status(
        int protocolVersion,
        Hash32 nodeFingerprint,
        Hash32 buildFingerprint,
        Hash32 configFingerprint,
        boolean restartRequired,
        HeightContextId heightContextId,
        long height,
        long view,
        StatusPhase phase,
        long leader,
        QuorumCertificateRef lockedPrepareQc,
        QuorumCertificateRef highestPrepareQc,
        TimeoutCertificateRef lastTimeoutCertificate,
        BodyState bodyState,
        Long pendingPersistenceId,
        long lastCommittedHeight,
        BlockSubject lastCommittedSubject,
        HeightContextStatus heightContext,
        CommitQcStatus lastCommitQc,
        LivenessStatus liveness) {
      require(protocolVersion == PROTOCOL_VERSION,
          "Unsupported Sumeragi status protocol version " + protocolVersion);
      requireU32(leader, "leader");
      this.protocolVersion = protocolVersion;
      this.nodeFingerprint = nonNull(nodeFingerprint, "nodeFingerprint");
      this.buildFingerprint = nonNull(buildFingerprint, "buildFingerprint");
      this.configFingerprint = nonNull(configFingerprint, "configFingerprint");
      this.restartRequired = restartRequired;
      this.heightContextId = nonNull(heightContextId, "heightContextId");
      this.height = height;
      this.view = view;
      this.phase = nonNull(phase, "phase");
      this.leader = leader;
      this.lockedPrepareQc = lockedPrepareQc;
      this.highestPrepareQc = highestPrepareQc;
      this.lastTimeoutCertificate = lastTimeoutCertificate;
      this.bodyState = nonNull(bodyState, "bodyState");
      this.pendingPersistenceId = pendingPersistenceId;
      this.lastCommittedHeight = lastCommittedHeight;
      this.lastCommittedSubject = lastCommittedSubject;
      this.heightContext = nonNull(heightContext, "heightContext");
      this.lastCommitQc = lastCommitQc;
      this.liveness = nonNull(liveness, "liveness");
    }

    @Override
    public byte[] encode() {
      return struct(
          u16(protocolVersion),
          nodeFingerprint.bytes(),
          buildFingerprint.bytes(),
          configFingerprint.bytes(),
          bool(restartRequired),
          heightContextId.encode(),
          u64(height),
          u64(view),
          phase.encode(),
          u32(leader),
          option(lockedPrepareQc == null ? null : lockedPrepareQc.encode()),
          option(highestPrepareQc == null ? null : highestPrepareQc.encode()),
          option(lastTimeoutCertificate == null ? null : lastTimeoutCertificate.encode()),
          bodyState.encode(),
          option(pendingPersistenceId == null ? null : u64(pendingPersistenceId)),
          u64(lastCommittedHeight),
          option(lastCommittedSubject == null ? null : lastCommittedSubject.encode()),
          heightContext.encode(),
          option(lastCommitQc == null ? null : lastCommitQc.encode()),
          liveness.encode());
    }

    /** Decode a canonical compact-length bare-Norito status payload. */
    public static SumeragiV2Status decodeCanonical(byte[] bytes) {
      Reader reader = new Reader(bytes);
      SumeragiV2Status value =
          new SumeragiV2Status(
              reader.field("status protocol", SumeragiV2Wire::decodeU16),
              new Hash32(reader.field("status node", SumeragiV2Wire::decodeHash)),
              new Hash32(reader.field("status build", SumeragiV2Wire::decodeHash)),
              new Hash32(reader.field("status config", SumeragiV2Wire::decodeHash)),
              reader.field("status restart required", SumeragiV2Wire::decodeBool),
              reader.field("status context", HeightContextId::decode),
              reader.field("status height", SumeragiV2Wire::decodeU64),
              reader.field("status view", SumeragiV2Wire::decodeU64),
              reader.field("status phase", StatusPhase::decode),
              reader.field("status leader", SumeragiV2Wire::decodeU32),
              reader.field("status lock", data -> decodeOption(data, QuorumCertificateRef::decode)),
              reader.field("status high qc", data -> decodeOption(data, QuorumCertificateRef::decode)),
              reader.field("status last tc", data -> decodeOption(data, TimeoutCertificateRef::decode)),
              reader.field("status body", BodyState::decode),
              reader.field("status persistence", data -> decodeOption(data, SumeragiV2Wire::decodeU64)),
              reader.field("status committed height", SumeragiV2Wire::decodeU64),
              reader.field("status committed subject", data -> decodeOption(data, BlockSubject::decode)),
              reader.field("status height context", HeightContextStatus::decode),
              reader.field("status last commit qc", data -> decodeOption(data, CommitQcStatus::decode)),
              reader.field("status liveness", LivenessStatus::decode));
      reader.finish("Sumeragi v2 status");
      require(Arrays.equals(bytes, value.encode()), "SumeragiV2Status is not canonical");
      return value;
    }
  }

  private static final class Reader {
    private final byte[] bytes;
    private int offset;

    Reader(byte[] bytes) {
      this.bytes = copy(bytes, "bytes");
    }

    int u8(String label) {
      ensure(1, label);
      return bytes[offset++] & 0xff;
    }

    int u16(String label) {
      ensure(2, label);
      int value = ByteBuffer.wrap(bytes, offset, 2).order(ByteOrder.LITTLE_ENDIAN).getShort() & 0xffff;
      offset += 2;
      return value;
    }

    long u32(String label) {
      ensure(4, label);
      long value = ByteBuffer.wrap(bytes, offset, 4).order(ByteOrder.LITTLE_ENDIAN).getInt() & 0xffff_ffffL;
      offset += 4;
      return value;
    }

    long u64(String label) {
      ensure(8, label);
      long value = ByteBuffer.wrap(bytes, offset, 8).order(ByteOrder.LITTLE_ENDIAN).getLong();
      offset += 8;
      return value;
    }

    byte[] compactField(String label) {
      long length = varint(label);
      require(length >= 0 && length <= Integer.MAX_VALUE, label + " length exceeds JVM range");
      return read((int) length, label);
    }

    <T> T field(String label, Function<byte[], T> decode) {
      return decode.apply(compactField(label));
    }

    void finish(String label) {
      require(offset == bytes.length, label + " contains trailing Norito bytes");
    }

    private byte[] read(int count, String label) {
      ensure(count, label);
      byte[] value = Arrays.copyOfRange(bytes, offset, offset + count);
      offset += count;
      return value;
    }

    private void ensure(int count, String label) {
      require(count >= 0 && offset <= bytes.length - count, label + " is truncated");
    }

    private long varint(String label) {
      long value = 0;
      int shift = 0;
      int count = 0;
      while (true) {
        int next = u8(label);
        count++;
        require(count <= 10 && shift < 64, label + " varint overflows u64");
        if (shift == 63) require((next & 0x7e) == 0, label + " varint overflows u64");
        value |= ((long) (next & 0x7f)) << shift;
        if ((next & 0x80) == 0) {
          require(varintBytes(value).length == count, label + " uses a non-canonical varint");
          return value;
        }
        shift += 7;
      }
    }
  }

  private static byte[] struct(byte[]... fields) {
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    for (byte[] field : fields) {
      append(out, varintBytes(field.length));
      append(out, field);
    }
    return out.toByteArray();
  }

  private static byte[] enumPayload(long tag, byte[] payload) {
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    append(out, u32(tag));
    append(out, varintBytes(payload.length));
    append(out, payload);
    return out.toByteArray();
  }

  private static byte[] u16(int value) {
    requireU16(value, "u16");
    return ByteBuffer.allocate(2).order(ByteOrder.LITTLE_ENDIAN).putShort((short) value).array();
  }

  private static byte[] u32(long value) {
    requireU32(value, "u32");
    return ByteBuffer.allocate(4).order(ByteOrder.LITTLE_ENDIAN).putInt((int) value).array();
  }

  private static byte[] u64(long value) {
    return ByteBuffer.allocate(8).order(ByteOrder.LITTLE_ENDIAN).putLong(value).array();
  }

  private static byte[] bool(boolean value) {
    return new byte[] {(byte) (value ? 1 : 0)};
  }

  private static byte[] byteVector(byte[] value) {
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    append(out, u64(value.length));
    append(out, value);
    return out.toByteArray();
  }

  private static byte[] string(String value) {
    requireWellFormedUtf16(value, "string");
    byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    append(out, varintBytes(bytes.length));
    append(out, bytes);
    return out.toByteArray();
  }

  private static byte[] option(byte[] value) {
    if (value == null) return new byte[] {0};
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    out.write(1);
    append(out, varintBytes(value.length));
    append(out, value);
    return out.toByteArray();
  }

  private static <T> byte[] vector(List<T> values, Function<T, byte[]> encode) {
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    append(out, u64(values.size()));
    for (T value : values) {
      byte[] payload = encode.apply(value);
      append(out, varintBytes(payload.length));
      append(out, payload);
    }
    return out.toByteArray();
  }

  private static byte[] varintBytes(long value) {
    require(value >= 0, "Norito compact length must be non-negative");
    ByteArrayOutputStream out = new ByteArrayOutputStream();
    long remaining = value;
    while (remaining >= 0x80) {
      out.write((int) ((remaining & 0x7f) | 0x80));
      remaining >>>= 7;
    }
    out.write((int) remaining);
    return out.toByteArray();
  }

  private static int decodeU16(byte[] bytes) {
    Reader reader = new Reader(bytes);
    int value = reader.u16("u16");
    reader.finish("u16");
    return value;
  }

  private static long decodeU32(byte[] bytes) {
    Reader reader = new Reader(bytes);
    long value = reader.u32("u32");
    reader.finish("u32");
    return value;
  }

  private static long decodeU64(byte[] bytes) {
    Reader reader = new Reader(bytes);
    long value = reader.u64("u64");
    reader.finish("u64");
    return value;
  }

  private static boolean decodeBool(byte[] bytes) {
    require(bytes.length == 1 && (bytes[0] == 0 || bytes[0] == 1),
        "bool must contain one canonical boolean byte");
    return bytes[0] == 1;
  }

  private static byte[] decodeHash(byte[] bytes) {
    require(bytes.length == 32, "hash must contain 32 bytes");
    require((bytes[31] & 1) == 1, "Iroha hash low bit must be set");
    return bytes.clone();
  }

  private static byte[] decodeByteVector(byte[] bytes) {
    Reader reader = new Reader(bytes);
    long length = reader.u64("byte vector length");
    require(length >= 0 && length <= Integer.MAX_VALUE, "byte vector exceeds JVM range");
    byte[] value = reader.read((int) length, "byte vector");
    reader.finish("byte vector");
    return value;
  }

  private static String decodeString(byte[] bytes) {
    Reader reader = new Reader(bytes);
    byte[] encoded = reader.compactField("string bytes");
    reader.finish("string");
    try {
      String value = StandardCharsets.UTF_8.newDecoder().decode(ByteBuffer.wrap(encoded)).toString();
      requireWellFormedUtf16(value, "string");
      return value;
    } catch (CharacterCodingException error) {
      throw new IllegalArgumentException("string is not valid UTF-8", error);
    }
  }

  private static <T> T decodeOption(byte[] bytes, Function<byte[], T> decode) {
    Reader reader = new Reader(bytes);
    int tag = reader.u8("option");
    if (tag == 0) {
      reader.finish("option");
      return null;
    }
    require(tag == 1, "invalid Option tag " + tag);
    T value = decode.apply(reader.compactField("option payload"));
    reader.finish("option");
    return value;
  }

  private static <T> List<T> decodeVector(byte[] bytes, Function<byte[], T> decode) {
    Reader reader = new Reader(bytes);
    long count = reader.u64("vector count");
    require(count >= 0 && count <= Integer.MAX_VALUE, "vector exceeds JVM range");
    List<T> values = new ArrayList<>((int) count);
    for (int i = 0; i < (int) count; i++) {
      values.add(decode.apply(reader.compactField("vector element")));
    }
    reader.finish("vector");
    return Collections.unmodifiableList(values);
  }

  private static void append(ByteArrayOutputStream out, byte[] value) {
    out.write(value, 0, value.length);
  }

  private static byte[] concat(byte[] left, byte[] right) {
    ByteArrayOutputStream out = new ByteArrayOutputStream(left.length + right.length);
    append(out, left);
    append(out, right);
    return out.toByteArray();
  }

  private static byte[] copy(byte[] value, String label) {
    if (value == null) throw new NullPointerException(label);
    return value.clone();
  }

  private static <T> T nonNull(T value, String label) {
    if (value == null) throw new NullPointerException(label);
    return value;
  }

  private static <T> List<T> immutable(List<T> values, String label) {
    if (values == null) throw new NullPointerException(label);
    ArrayList<T> copy = new ArrayList<>(values.size());
    for (T value : values) copy.add(nonNull(value, label + " element"));
    return Collections.unmodifiableList(copy);
  }

  private static void requireIncreasing(List<Long> values, String label) {
    long previous = -1;
    for (Long value : values) {
      requireU32(value, label);
      require(value > previous, label + " must be strictly increasing");
      previous = value;
    }
  }

  private static void requireU16(int value, String label) {
    require(value >= 0 && value <= 0xffff, label + " must fit u16");
  }

  private static void requireU32(long value, String label) {
    require(value >= 0 && value <= 0xffff_ffffL, label + " must fit u32");
  }

  private static void requireWellFormedUtf16(String value, String label) {
    for (int index = 0; index < value.length(); index++) {
      char current = value.charAt(index);
      if (Character.isHighSurrogate(current)) {
        require(
            index + 1 < value.length() && Character.isLowSurrogate(value.charAt(index + 1)),
            label + " contains an unpaired UTF-16 surrogate");
        index++;
      } else {
        require(!Character.isLowSurrogate(current),
            label + " contains an unpaired UTF-16 surrogate");
      }
    }
  }

  private static void require(boolean condition, String message) {
    if (!condition) throw new IllegalArgumentException(message);
  }
}

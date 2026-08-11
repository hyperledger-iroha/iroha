// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.consensus;

import java.math.BigInteger;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.sdk.consensus.NativeAmxV2;

/**
 * Java-facing mirror of the strict Kotlin Native AMX V2 control models.
 *
 * <p>Validation is delegated to the Kotlin core model used by Android clients,
 * so both SDK surfaces reject exactly the same malformed receipts.
 */
public final class NativeAmxV2Models {
  private static final BigInteger U64_MAX =
      BigInteger.ONE.shiftLeft(Long.SIZE).subtract(BigInteger.ONE);

  /** Current coordinated first-release receipt version. */
  public static final int RECEIPT_VERSION = NativeAmxV2.RECEIPT_VERSION;

  /** Maximum grouped source count. */
  public static final int MAX_GROUP_SOURCES = NativeAmxV2.MAX_GROUP_SOURCES;

  /** Maximum participant-leg count. */
  public static final int MAX_RECEIPT_LEGS = NativeAmxV2.MAX_RECEIPT_LEGS;

  /** Maximum Native AMX validator count. */
  public static final int MAX_VALIDATORS = NativeAmxV2.MAX_VALIDATORS;

  /** Exact BLS-Normal proof/signature size. */
  public static final int BLS_PROOF_BYTES = NativeAmxV2.BLS_PROOF_BYTES;

  private NativeAmxV2Models() {}

  /** Prepare/Commit phase decoded from the mandatory tagged object. */
  public enum Phase {
    PREPARE,
    COMMIT;

    private static Phase fromKotlin(final NativeAmxV2.Phase value) {
      return value == NativeAmxV2.Phase.PREPARE ? PREPARE : COMMIT;
    }
  }

  /** Typed raw 32-byte source identity. */
  public static final class SourceId {
    private final NativeAmxV2.SourceId delegate;

    private SourceId(final NativeAmxV2.SourceId delegate) {
      this.delegate = delegate;
    }

    public String value() {
      return delegate.getValue();
    }

    @Override
    public boolean equals(final Object other) {
      return other instanceof SourceId && delegate.equals(((SourceId) other).delegate);
    }

    @Override
    public int hashCode() {
      return delegate.hashCode();
    }

    @Override
    public String toString() {
      return value();
    }
  }

  /** Typed transaction-entrypoint hash. */
  public static final class TransactionEntrypointHash {
    private final NativeAmxV2.TransactionEntrypointHash delegate;

    private TransactionEntrypointHash(
        final NativeAmxV2.TransactionEntrypointHash delegate) {
      this.delegate = delegate;
    }

    public String value() {
      return delegate.getValue();
    }

    @Override
    public boolean equals(final Object other) {
      return other instanceof TransactionEntrypointHash
          && delegate.equals(((TransactionEntrypointHash) other).delegate);
    }

    @Override
    public int hashCode() {
      return delegate.hashCode();
    }

    @Override
    public String toString() {
      return value();
    }
  }

  /** Canonical non-zero Iroha hash literal. */
  public static final class ConsensusHash {
    private final NativeAmxV2.ConsensusHash delegate;

    private ConsensusHash(final NativeAmxV2.ConsensusHash delegate) {
      this.delegate = delegate;
    }

    public String value() {
      return delegate.getValue();
    }

    @Override
    public boolean equals(final Object other) {
      return other instanceof ConsensusHash
          && delegate.equals(((ConsensusHash) other).delegate);
    }

    @Override
    public int hashCode() {
      return delegate.hashCode();
    }

    @Override
    public String toString() {
      return value();
    }
  }

  /** Immutable byte string. */
  public static final class Bytes {
    private final NativeAmxV2.Bytes delegate;

    private Bytes(final NativeAmxV2.Bytes delegate) {
      this.delegate = delegate;
    }

    public byte[] toByteArray() {
      return delegate.toByteArray();
    }

    public int size() {
      return delegate.getSize();
    }

    @Override
    public boolean equals(final Object other) {
      return other instanceof Bytes && delegate.equals(((Bytes) other).delegate);
    }

    @Override
    public int hashCode() {
      return delegate.hashCode();
    }
  }

  /** Frozen global consensus round. */
  public static final class Round {
    private final NativeAmxV2.Round delegate;

    private Round(final NativeAmxV2.Round delegate) {
      this.delegate = delegate;
    }

    public ConsensusHash contextId() {
      return new ConsensusHash(delegate.getContextId());
    }

    public BigInteger height() {
      return unsigned64(delegate.getHeight(), "round.height");
    }

    public BigInteger view() {
      return unsigned64(delegate.getView(), "round.view");
    }

    @Override
    public boolean equals(final Object other) {
      return other instanceof Round && delegate.equals(((Round) other).delegate);
    }

    @Override
    public int hashCode() {
      return delegate.hashCode();
    }
  }

  /** Signed Native AMX V2 participant-attestation body. */
  public static final class AttestationBody {
    private final NativeAmxV2.AttestationBody delegate;

    private AttestationBody(final NativeAmxV2.AttestationBody delegate) {
      this.delegate = delegate;
    }

    public Round round() {
      return new Round(delegate.getRound());
    }

    public BigInteger epoch() {
      return unsigned64(delegate.getEpoch(), "attestation.epoch");
    }

    public NetworkId networkId() {
      return NetworkId.parse(delegate.getNetworkId().getLiteral());
    }

    public SourceId sourceId() {
      return new SourceId(delegate.getSourceId());
    }

    public TransactionEntrypointHash transactionEntrypointHash() {
      return new TransactionEntrypointHash(delegate.getTransactionEntrypointHash());
    }

    public ConsensusHash planDigest() {
      return new ConsensusHash(delegate.getPlanDigest());
    }

    public Phase phase() {
      return Phase.fromKotlin(delegate.getPhase());
    }

    public long coordinatorLaneId() {
      return delegate.getCoordinatorLaneId();
    }

    public BigInteger coordinatorDataspaceId() {
      return unsigned64(
          delegate.getCoordinatorDataspaceId(), "attestation.coordinator_dataspace_id");
    }

    public ConsensusHash coordinatorLaneIncarnation() {
      return new ConsensusHash(delegate.getCoordinatorLaneIncarnation());
    }

    public long participantLaneId() {
      return delegate.getParticipantLaneId();
    }

    public BigInteger participantDataspaceId() {
      return unsigned64(
          delegate.getParticipantDataspaceId(), "attestation.participant_dataspace_id");
    }

    public ConsensusHash participantLaneIncarnation() {
      return new ConsensusHash(delegate.getParticipantLaneIncarnation());
    }

    public BigInteger participantPreviousBlockHeight() {
      return unsigned64(
          delegate.getParticipantPreviousBlockHeight(),
          "attestation.participant_previous_block_height");
    }

    public ConsensusHash participantPreviousBlockDescriptorHash() {
      final NativeAmxV2.ConsensusHash value =
          delegate.getParticipantPreviousBlockDescriptorHash();
      return value == null ? null : new ConsensusHash(value);
    }

    public BigInteger participantLaneBlockHeight() {
      return unsigned64(
          delegate.getParticipantLaneBlockHeight(), "attestation.participant_lane_block_height");
    }

    public BigInteger participantLaneBlockView() {
      return unsigned64(
          delegate.getParticipantLaneBlockView(), "attestation.participant_lane_block_view");
    }

    public ConsensusHash participantProposalHash() {
      return new ConsensusHash(delegate.getParticipantProposalHash());
    }

    public ConsensusHash participantSettlementCommitment() {
      return new ConsensusHash(delegate.getParticipantSettlementCommitment());
    }

    public ConsensusHash participantValidatorSetHash() {
      return new ConsensusHash(delegate.getParticipantValidatorSetHash());
    }

    public int participantValidatorCount() {
      return delegate.getParticipantValidatorCount();
    }

    public int participantMinQuorum() {
      return delegate.getParticipantMinQuorum();
    }

    public BigInteger authorityContextHeight() {
      return unsigned64(
          delegate.getAuthorityContextHeight(), "attestation.authority_context_height");
    }

    public BigInteger plannedCoordinatorBlockHeight() {
      return unsigned64(
          delegate.getPlannedCoordinatorBlockHeight(),
          "attestation.planned_coordinator_block_height");
    }

    public BigInteger coordinatorLaneBlockView() {
      return unsigned64(
          delegate.getCoordinatorLaneBlockView(), "attestation.coordinator_lane_block_view");
    }

    public ConsensusHash coordinatorProposalHash() {
      return new ConsensusHash(delegate.getCoordinatorProposalHash());
    }

    @Override
    public boolean equals(final Object other) {
      return other instanceof AttestationBody
          && delegate.equals(((AttestationBody) other).delegate);
    }

    @Override
    public int hashCode() {
      return delegate.hashCode();
    }
  }

  /** Validator-set proof for one attestation body. */
  public static final class AttestationQc {
    private final NativeAmxV2.AttestationQc delegate;

    private AttestationQc(final NativeAmxV2.AttestationQc delegate) {
      this.delegate = delegate;
    }

    public AttestationBody body() {
      return new AttestationBody(delegate.getBody());
    }

    public int validatorSetHashVersion() {
      return delegate.getValidatorSetHashVersion();
    }

    public ConsensusHash validatorSetHash() {
      return new ConsensusHash(delegate.getValidatorSetHash());
    }

    public List<String> validatorSet() {
      return delegate.getValidatorSet();
    }

    public List<Bytes> validatorSetPops() {
      final ArrayList<Bytes> values = new ArrayList<>();
      for (final NativeAmxV2.Bytes value : delegate.getValidatorSetPops()) {
        values.add(new Bytes(value));
      }
      return Collections.unmodifiableList(values);
    }

    public Bytes signersBitmap() {
      return new Bytes(delegate.getSignersBitmap());
    }

    public Bytes aggregateSignature() {
      return new Bytes(delegate.getAggregateSignature());
    }

    @Override
    public boolean equals(final Object other) {
      return other instanceof AttestationQc
          && delegate.equals(((AttestationQc) other).delegate);
    }

    @Override
    public int hashCode() {
      return delegate.hashCode();
    }
  }

  /** One zero-effect grouped settlement row. */
  public static final class SettlementReceipt {
    private final NativeAmxV2.SettlementReceipt delegate;

    private SettlementReceipt(final NativeAmxV2.SettlementReceipt delegate) {
      this.delegate = delegate;
    }

    public SourceId sourceId() {
      return new SourceId(delegate.getSourceId());
    }

    public String localAmount() {
      return delegate.getLocalAmount();
    }

    public String xorDue() {
      return delegate.getXorDue();
    }

    public String xorAfterHaircut() {
      return delegate.getXorAfterHaircut();
    }

    public String xorVariance() {
      return delegate.getXorVariance();
    }

    public BigInteger timestampMs() {
      return unsigned64(delegate.getTimestampMs(), "participant_settlement.timestamp_ms");
    }

    @Override
    public boolean equals(final Object other) {
      return other instanceof SettlementReceipt
          && delegate.equals(((SettlementReceipt) other).delegate);
    }

    @Override
    public int hashCode() {
      return delegate.hashCode();
    }
  }

  /** Terminal zero-effect participant settlement. */
  public static final class ParticipantSettlement {
    private final NativeAmxV2.ParticipantSettlement delegate;

    private ParticipantSettlement(final NativeAmxV2.ParticipantSettlement delegate) {
      this.delegate = delegate;
    }

    public BigInteger blockHeight() {
      return unsigned64(delegate.getBlockHeight(), "participant_settlement.block_height");
    }

    public long laneId() {
      return delegate.getLaneId();
    }

    public ConsensusHash laneIncarnation() {
      return new ConsensusHash(delegate.getLaneIncarnation());
    }

    public BigInteger dataspaceId() {
      return unsigned64(delegate.getDataspaceId(), "participant_settlement.dataspace_id");
    }

    public long transactionCount() {
      return delegate.getTransactionCount();
    }

    public String totalLocalAmount() {
      return delegate.getTotalLocalAmount();
    }

    public String totalXorDue() {
      return delegate.getTotalXorDue();
    }

    public String totalXorAfterHaircut() {
      return delegate.getTotalXorAfterHaircut();
    }

    public String totalXorVariance() {
      return delegate.getTotalXorVariance();
    }

    public List<SettlementReceipt> receipts() {
      final ArrayList<SettlementReceipt> values = new ArrayList<>();
      for (final NativeAmxV2.SettlementReceipt value : delegate.getReceipts()) {
        values.add(new SettlementReceipt(value));
      }
      return Collections.unmodifiableList(values);
    }

    @Override
    public boolean equals(final Object other) {
      return other instanceof ParticipantSettlement
          && delegate.equals(((ParticipantSettlement) other).delegate);
    }

    @Override
    public int hashCode() {
      return delegate.hashCode();
    }
  }

  /** Exact control-only participant lane-block descriptor. */
  public static final class ParticipantDescriptor {
    private final NativeAmxV2.ParticipantDescriptor delegate;

    private ParticipantDescriptor(final NativeAmxV2.ParticipantDescriptor delegate) {
      this.delegate = delegate;
    }

    public long laneId() {
      return delegate.getLaneId();
    }

    public BigInteger dataspaceId() {
      return unsigned64(delegate.getDataspaceId(), "participant_descriptor.dataspace_id");
    }

    public ConsensusHash laneIncarnation() {
      return new ConsensusHash(delegate.getLaneIncarnation());
    }

    public BigInteger proposalHeight() {
      return unsigned64(delegate.getProposalHeight(), "participant_descriptor.proposal_height");
    }

    public BigInteger previousLaneBlockHeight() {
      return unsigned64(
          delegate.getPreviousLaneBlockHeight(),
          "participant_descriptor.previous_lane_block_height");
    }

    public ConsensusHash previousLaneBlockDescriptorHash() {
      final NativeAmxV2.ConsensusHash value =
          delegate.getPreviousLaneBlockDescriptorHash();
      return value == null ? null : new ConsensusHash(value);
    }

    public BigInteger laneBlockHeight() {
      return unsigned64(
          delegate.getLaneBlockHeight(), "participant_descriptor.lane_block_height");
    }

    public BigInteger laneBlockView() {
      return unsigned64(delegate.getLaneBlockView(), "participant_descriptor.lane_block_view");
    }

    public ConsensusHash subjectHash() {
      return new ConsensusHash(delegate.getSubjectHash());
    }

    public ConsensusHash payloadOwnershipHash() {
      return new ConsensusHash(delegate.getPayloadOwnershipHash());
    }

    public ConsensusHash rbcInstanceHash() {
      return new ConsensusHash(delegate.getRbcInstanceHash());
    }

    public List<BigInteger> acceptedCandidateIndices() {
      final ArrayList<BigInteger> values = new ArrayList<>();
      for (final Object value : delegate.getAcceptedCandidateIndices()) {
        values.add(unsigned64(value, "participant_descriptor.accepted_candidate_indices"));
      }
      return Collections.unmodifiableList(values);
    }

    public List<TransactionEntrypointHash> acceptedTransactionHashes() {
      final ArrayList<TransactionEntrypointHash> values = new ArrayList<>();
      for (final NativeAmxV2.TransactionEntrypointHash value :
          delegate.getAcceptedTransactionHashes()) {
        values.add(new TransactionEntrypointHash(value));
      }
      return Collections.unmodifiableList(values);
    }

    public int validatorSetHashVersion() {
      return delegate.getValidatorSetHashVersion();
    }

    public ConsensusHash validatorSetHash() {
      return new ConsensusHash(delegate.getValidatorSetHash());
    }

    public List<String> validatorSet() {
      return delegate.getValidatorSet();
    }

    public int validatorCount() {
      return delegate.getValidatorCount();
    }

    public int minQuorum() {
      return delegate.getMinQuorum();
    }

    public String qcModeTag() {
      return delegate.getQcModeTag();
    }

    public ConsensusHash descriptorHash() {
      return new ConsensusHash(delegate.getDescriptorHash());
    }

    @Override
    public boolean equals(final Object other) {
      return other instanceof ParticipantDescriptor
          && delegate.equals(((ParticipantDescriptor) other).delegate);
    }

    @Override
    public int hashCode() {
      return delegate.hashCode();
    }
  }

  /** Participant proposal without proposer-local recovery hints. */
  public static final class ParticipantProposal {
    private final NativeAmxV2.ParticipantProposal delegate;

    private ParticipantProposal(final NativeAmxV2.ParticipantProposal delegate) {
      this.delegate = delegate;
    }

    public ParticipantDescriptor descriptor() {
      return new ParticipantDescriptor(delegate.getDescriptor());
    }

    public ConsensusHash proposalHash() {
      return new ConsensusHash(delegate.getProposalHash());
    }

    @Override
    public boolean equals(final Object other) {
      return other instanceof ParticipantProposal
          && delegate.equals(((ParticipantProposal) other).delegate);
    }

    @Override
    public int hashCode() {
      return delegate.hashCode();
    }
  }

  /** Prepare/Commit proof for one participant route. */
  public static final class Leg {
    private final NativeAmxV2.Leg delegate;

    private Leg(final NativeAmxV2.Leg delegate) {
      this.delegate = delegate;
    }

    public long laneId() {
      return delegate.getLaneId();
    }

    public BigInteger dataspaceId() {
      return unsigned64(delegate.getDataspaceId(), "participant_leg.dataspace_id");
    }

    public ConsensusHash laneIncarnation() {
      return new ConsensusHash(delegate.getLaneIncarnation());
    }

    public ParticipantProposal participantProposal() {
      return new ParticipantProposal(delegate.getParticipantProposal());
    }

    public ParticipantSettlement participantSettlement() {
      return new ParticipantSettlement(delegate.getParticipantSettlement());
    }

    public ConsensusHash participantSettlementHash() {
      return new ConsensusHash(delegate.getParticipantSettlementHash());
    }

    public AttestationQc prepareQc() {
      return new AttestationQc(delegate.getPrepareQc());
    }

    public AttestationQc commitQc() {
      return new AttestationQc(delegate.getCommitQc());
    }

    public boolean requiresMixedRoleAnchorValidation() {
      return delegate.getRequiresMixedRoleAnchorValidation();
    }

    @Override
    public boolean equals(final Object other) {
      return other instanceof Leg && delegate.equals(((Leg) other).delegate);
    }

    @Override
    public int hashCode() {
      return delegate.hashCode();
    }
  }

  /** Context-bound Native AMX V2 receipt for one source. */
  public static final class Receipt {
    private final NativeAmxV2.Receipt delegate;

    private Receipt(final NativeAmxV2.Receipt delegate) {
      this.delegate = delegate;
    }

    public int version() {
      return delegate.getVersion();
    }

    public SourceId sourceId() {
      return new SourceId(delegate.getSourceId());
    }

    public NetworkId networkId() {
      return NetworkId.parse(delegate.getNetworkId().getLiteral());
    }

    public ConsensusHash planDigest() {
      return new ConsensusHash(delegate.getPlanDigest());
    }

    public long laneId() {
      return delegate.getLaneId();
    }

    public BigInteger dataspaceId() {
      return unsigned64(delegate.getDataspaceId(), "receipt.dataspace_id");
    }

    public ConsensusHash laneIncarnation() {
      return new ConsensusHash(delegate.getLaneIncarnation());
    }

    public BigInteger authorityContextHeight() {
      return unsigned64(delegate.getAuthorityContextHeight(), "receipt.authority_context_height");
    }

    public BigInteger laneBlockHeight() {
      return unsigned64(delegate.getLaneBlockHeight(), "receipt.lane_block_height");
    }

    public BigInteger laneBlockView() {
      return unsigned64(delegate.getLaneBlockView(), "receipt.lane_block_view");
    }

    public ConsensusHash coordinatorProposalHash() {
      return new ConsensusHash(delegate.getCoordinatorProposalHash());
    }

    public List<Leg> legs() {
      final ArrayList<Leg> values = new ArrayList<>();
      for (final NativeAmxV2.Leg value : delegate.getLegs()) {
        values.add(new Leg(value));
      }
      return Collections.unmodifiableList(values);
    }

    @Override
    public boolean equals(final Object other) {
      return other instanceof Receipt && delegate.equals(((Receipt) other).delegate);
    }

    @Override
    public int hashCode() {
      return delegate.hashCode();
    }
  }

  /** One lane settlement containing an ordered Native AMX source group. */
  public static final class ReceiptGroup {
    private final NativeAmxV2.ReceiptGroup delegate;

    private ReceiptGroup(final NativeAmxV2.ReceiptGroup delegate) {
      this.delegate = delegate;
    }

    public BigInteger blockHeight() {
      return unsigned64(delegate.getBlockHeight(), "receipt_group.block_height");
    }

    public long laneId() {
      return delegate.getLaneId();
    }

    public ConsensusHash laneIncarnation() {
      return new ConsensusHash(delegate.getLaneIncarnation());
    }

    public BigInteger dataspaceId() {
      return unsigned64(delegate.getDataspaceId(), "receipt_group.dataspace_id");
    }

    public long transactionCount() {
      return delegate.getTransactionCount();
    }

    public List<Receipt> receipts() {
      final ArrayList<Receipt> values = new ArrayList<>();
      for (final NativeAmxV2.Receipt value : delegate.getReceipts()) {
        values.add(new Receipt(value));
      }
      return Collections.unmodifiableList(values);
    }

    @Override
    public boolean equals(final Object other) {
      return other instanceof ReceiptGroup
          && delegate.equals(((ReceiptGroup) other).delegate);
    }

    @Override
    public int hashCode() {
      return delegate.hashCode();
    }
  }

  /** Return true only for a canonical non-infinity BLS-Normal subgroup point. */
  public static boolean isCanonicalBlsNormalPeerId(final String value) {
    return NativeAmxV2.isCanonicalBlsNormalPeerId(value);
  }

  /** Return whether this entrypoint needs block-wide mixed-role anchor validation. */
  public static boolean requiresMixedRoleAnchorValidation(
      final ParticipantDescriptor descriptor,
      final String transactionEntrypointHash) {
    if (descriptor == null) {
      throw new IllegalArgumentException("descriptor must be provided");
    }
    return NativeAmxV2.requiresMixedRoleAnchorValidation(
        descriptor.delegate,
        new NativeAmxV2.TransactionEntrypointHash(transactionEntrypointHash));
  }

  /** Parse and strictly validate a Native AMX receipt-group JSON string. */
  public static ReceiptGroup parseReceiptGroup(final String json) {
    return new ReceiptGroup(NativeAmxV2.parseReceiptGroup(json));
  }

  /** Parse and strictly validate UTF-8 Native AMX receipt-group JSON. */
  public static ReceiptGroup parseReceiptGroup(final byte[] json) {
    return parseReceiptGroup(
        SumeragiJsonSupport.decodeUtf8(json, "Native AMX receipt group"));
  }

  /** Validate a map returned by the Java SDK JSON parser. */
  public static ReceiptGroup parseReceiptGroup(final Map<String, Object> value) {
    return new ReceiptGroup(NativeAmxV2.parseReceiptGroup(value));
  }

  /** Parse and strictly validate a standalone Native AMX V2 receipt. */
  public static Receipt parseReceipt(final String json) {
    return new Receipt(NativeAmxV2.parseReceipt(json));
  }

  /** Parse and strictly validate a standalone UTF-8 receipt. */
  public static Receipt parseReceipt(final byte[] json) {
    return parseReceipt(SumeragiJsonSupport.decodeUtf8(json, "Native AMX V2 receipt"));
  }

  /** Validate a standalone receipt map returned by the Java SDK JSON parser. */
  public static Receipt parseReceipt(final Map<String, Object> value) {
    return new Receipt(NativeAmxV2.parseReceipt(value));
  }

  /** Parse a JSON string with the Java SDK parser and validate its Native AMX group. */
  @SuppressWarnings("unchecked")
  public static ReceiptGroup parseReceiptGroupWithJavaParser(final String json) {
    final Object value = JsonParser.parse(json);
    if (!(value instanceof Map)) {
      throw new IllegalArgumentException("Native AMX receipt group must be a JSON object");
    }
    return parseReceiptGroup((Map<String, Object>) value);
  }

  private static BigInteger unsigned64(final Object value, final String field) {
    final BigInteger parsed;
    if (value instanceof BigInteger) {
      parsed = (BigInteger) value;
    } else if (value instanceof Byte
        || value instanceof Short
        || value instanceof Integer
        || value instanceof Long) {
      parsed = BigInteger.valueOf(((Number) value).longValue());
    } else {
      throw new IllegalArgumentException(field + " must be an integer");
    }
    if (parsed.signum() < 0 || parsed.compareTo(U64_MAX) > 0) {
      throw new IllegalArgumentException(field + " must fit in unsigned 64-bit range");
    }
    return parsed;
  }
}

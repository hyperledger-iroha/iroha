package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for {@code LeaveKaigi} instructions. */
public final class LeaveKaigiInstruction implements InstructionTemplate {

  private static final String ACTION = "LeaveKaigi";
  private static final java.util.Set<String> ALLOWED_ARGUMENTS =
      KaigiInstructionUtils.argumentSet(
          "action",
          "call.domain_id",
          "call.call_name",
          "participant",
          "commitment.commitment",
          "commitment.alias_tag",
          "nullifier.digest",
          "nullifier.issued_at_ms",
          "roster_root",
          "proof");

  private final KaigiInstructionUtils.CallId callId;
  private final String participant;
  private final String commitment;
  private final String commitmentAliasTag;
  private final String nullifierDigest;
  private final Long nullifierIssuedAtMs;
  private final String rosterRoot;
  private final String proofBase64;
  private final Map<String, String> arguments;

  private LeaveKaigiInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private LeaveKaigiInstruction(final Builder builder, final Map<String, String> argumentOrder) {
    this.callId = builder.callId;
    this.participant = builder.participant;
    this.commitment = builder.commitment;
    this.commitmentAliasTag = builder.commitmentAliasTag;
    this.nullifierDigest = builder.nullifierDigest;
    this.nullifierIssuedAtMs = builder.nullifierIssuedAtMs;
    this.rosterRoot = builder.rosterRoot;
    this.proofBase64 = builder.proofBase64;
    this.arguments = Collections.unmodifiableMap(new LinkedHashMap<>(argumentOrder));
  }

  public KaigiInstructionUtils.CallId callId() {
    return callId;
  }

  public String participant() {
    return participant;
  }

  public String commitment() {
    return commitment;
  }

  public String commitmentAliasTag() {
    return commitmentAliasTag;
  }

  public String nullifierDigest() {
    return nullifierDigest;
  }

  public Long nullifierIssuedAtMs() {
    return nullifierIssuedAtMs;
  }

  public String rosterRoot() {
    return rosterRoot;
  }

  public String proofBase64() {
    return proofBase64;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static LeaveKaigiInstruction fromArguments(final Map<String, String> arguments) {
    KaigiInstructionUtils.requireKnownArguments(arguments, ALLOWED_ARGUMENTS);
    KaigiInstructionUtils.requireAction(arguments, ACTION);
    final Builder builder = builder();
    builder.setCallId(KaigiInstructionUtils.parseCallId(arguments, "call"));
    builder.setParticipant(KaigiInstructionUtils.require(arguments, "participant"));

    if (arguments.containsKey("commitment.commitment")
        || arguments.containsKey("commitment.alias_tag")
        || arguments.containsKey("nullifier.digest")
        || arguments.containsKey("nullifier.issued_at_ms")
        || arguments.containsKey("roster_root")
        || arguments.containsKey("proof")) {
      throw new IllegalArgumentException(
          "LeaveKaigi privacy artifacts are reserved and must be omitted in V1");
    }

    return builder.build();
  }

  public static Builder builder() {
    return new Builder();
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof LeaveKaigiInstruction other)) {
      return false;
    }
    return Objects.equals(callId.domainId(), other.callId.domainId())
        && Objects.equals(callId.callName(), other.callId.callName())
        && Objects.equals(participant, other.participant)
        && Objects.equals(commitment, other.commitment)
        && Objects.equals(commitmentAliasTag, other.commitmentAliasTag)
        && Objects.equals(nullifierDigest, other.nullifierDigest)
        && Objects.equals(nullifierIssuedAtMs, other.nullifierIssuedAtMs)
        && Objects.equals(rosterRoot, other.rosterRoot)
        && Objects.equals(proofBase64, other.proofBase64);
  }

  @Override
  public int hashCode() {
    return Objects.hash(
        callId.domainId(),
        callId.callName(),
        participant,
        commitment,
        commitmentAliasTag,
        nullifierDigest,
        nullifierIssuedAtMs,
        rosterRoot,
        proofBase64);
  }

  public static final class Builder {
    private KaigiInstructionUtils.CallId callId;
    private String participant;
    private String commitment;
    private String commitmentAliasTag;
    private String nullifierDigest;
    private Long nullifierIssuedAtMs;
    private String rosterRoot;
    private String proofBase64;

    private Builder() {}

    public Builder setCallId(final String domainId, final String callName) {
      this.callId = new KaigiInstructionUtils.CallId(domainId, callName);
      return this;
    }

    public Builder setCallId(final KaigiInstructionUtils.CallId callId) {
      this.callId = Objects.requireNonNull(callId, "callId");
      return this;
    }

    public Builder setParticipant(final String participant) {
      if (participant == null || participant.isBlank()) {
        throw new IllegalArgumentException("participant must not be blank");
      }
      this.participant = participant;
      return this;
    }

    public Builder setCommitment(final byte[] commitment) {
      rejectReservedPrivacyArtifact(commitment, "commitment");
      this.commitment = null;
      return this;
    }

    public Builder setCommitment(final String commitmentHexOrLiteral) {
      rejectReservedPrivacyArtifact(commitmentHexOrLiteral, "commitment");
      this.commitment = null;
      return this;
    }

    Builder setCommitmentLiteral(final String literal) {
      rejectReservedPrivacyArtifact(literal, "commitment");
      this.commitment = null;
      return this;
    }

    public Builder setCommitmentAliasTag(final String aliasTag) {
      if (aliasTag != null) {
        throw new IllegalArgumentException(
            "commitment aliasTag is off-chain only and must be omitted");
      }
      this.commitmentAliasTag = null;
      return this;
    }

    public Builder setNullifierDigest(final byte[] digest) {
      rejectReservedPrivacyArtifact(digest, "nullifier");
      this.nullifierDigest = null;
      return this;
    }

    public Builder setNullifierDigest(final String digestHexOrLiteral) {
      rejectReservedPrivacyArtifact(digestHexOrLiteral, "nullifier");
      this.nullifierDigest = null;
      return this;
    }

    public Builder setNullifierIssuedAtMs(final Long issuedAtMs) {
      rejectReservedPrivacyArtifact(issuedAtMs, "nullifier issuedAtMs");
      this.nullifierIssuedAtMs = null;
      return this;
    }

    public Builder setRosterRoot(final byte[] rosterRoot) {
      rejectReservedPrivacyArtifact(rosterRoot, "roster root");
      this.rosterRoot = null;
      return this;
    }

    public Builder setRosterRoot(final String rosterRootHexOrLiteral) {
      rejectReservedPrivacyArtifact(rosterRootHexOrLiteral, "roster root");
      this.rosterRoot = null;
      return this;
    }

    Builder setRosterRootLiteral(final String literal) {
      rejectReservedPrivacyArtifact(literal, "roster root");
      this.rosterRoot = null;
      return this;
    }

    public Builder setProof(final byte[] proofBytes) {
      rejectReservedPrivacyArtifact(proofBytes, "proof");
      this.proofBase64 = null;
      return this;
    }

    public Builder setProofBase64(final String proofBase64) {
      rejectReservedPrivacyArtifact(proofBase64, "proof");
      this.proofBase64 = null;
      return this;
    }

    private static void rejectReservedPrivacyArtifact(
        final Object value, final String fieldName) {
      if (value != null) {
        throw new IllegalArgumentException(
            "LeaveKaigi " + fieldName + " is reserved and must be omitted in V1");
      }
    }

    public LeaveKaigiInstruction build() {
      if (callId == null) {
        throw new IllegalStateException("callId must be provided");
      }
      if (participant == null) {
        throw new IllegalStateException("participant must be provided");
      }
      return new LeaveKaigiInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      KaigiInstructionUtils.appendCallId(callId, args, "call");
      args.put("participant", participant);
      if (commitment != null) {
        args.put("commitment.commitment", commitment);
      }
      if (nullifierDigest != null) {
        args.put("nullifier.digest", nullifierDigest);
        if (nullifierIssuedAtMs != null) {
          args.put("nullifier.issued_at_ms", Long.toUnsignedString(nullifierIssuedAtMs));
        }
      }
      if (rosterRoot != null) {
        args.put("roster_root", rosterRoot);
      }
      if (proofBase64 != null) {
        args.put("proof", proofBase64);
      }
      return args;
    }
  }
}

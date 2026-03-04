package org.hyperledger.iroha.android.model.instructions;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for {@code JoinKaigi} instructions. */
public final class JoinKaigiInstruction implements InstructionTemplate {

  private static final String ACTION = "JoinKaigi";

  private final KaigiInstructionUtils.CallId callId;
  private final String participant;
  private final String commitment;
  private final String commitmentAliasTag;
  private final String nullifierDigest;
  private final Long nullifierIssuedAtMs;
  private final String rosterRoot;
  private final String proofBase64;
  private final Map<String, String> arguments;

  private JoinKaigiInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private JoinKaigiInstruction(final Builder builder, final Map<String, String> argumentOrder) {
    this.callId = builder.callId;
    this.participant = builder.participant;
    this.commitment = builder.commitment;
    this.commitmentAliasTag = builder.commitmentAliasTag;
    this.nullifierDigest = builder.nullifierDigest;
    this.nullifierIssuedAtMs = builder.nullifierIssuedAtMs;
    this.rosterRoot = builder.rosterRoot;
    this.proofBase64 = builder.proofBase64;
    this.arguments = Map.copyOf(argumentOrder);
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

  public static JoinKaigiInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder = builder();
    builder.setCallId(KaigiInstructionUtils.parseCallId(arguments, "call"));
    builder.setParticipant(KaigiInstructionUtils.require(arguments, "participant"));

    final String commitmentValue = arguments.get("commitment.commitment");
    if (commitmentValue != null) {
      builder.setCommitmentLiteral(commitmentValue);
      final String alias = arguments.get("commitment.alias_tag");
      if (alias != null) {
        builder.setCommitmentAliasTag(alias);
      }
    }

    final String nullifier = arguments.get("nullifier.digest");
    if (nullifier != null) {
      builder.setNullifierDigest(nullifier);
      final Long issuedAt =
          KaigiInstructionUtils.parseOptionalUnsignedLong(
              arguments.get("nullifier.issued_at_ms"), "nullifier.issued_at_ms");
      builder.setNullifierIssuedAtMs(issuedAt);
    }

    builder.setRosterRootLiteral(arguments.get("roster_root"));
    builder.setProofBase64(arguments.get("proof"));

    return new JoinKaigiInstruction(builder, new LinkedHashMap<>(arguments));
  }

  public static Builder builder() {
    return new Builder();
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof JoinKaigiInstruction other)) {
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
      this.commitment = KaigiInstructionUtils.canonicalizeOptionalHash(commitment);
      return this;
    }

    public Builder setCommitment(final String commitmentHexOrLiteral) {
      this.commitment = KaigiInstructionUtils.canonicalizeOptionalHash(commitmentHexOrLiteral);
      return this;
    }

    Builder setCommitmentLiteral(final String literal) {
      this.commitment = literal;
      return this;
    }

    public Builder setCommitmentAliasTag(final String aliasTag) {
      this.commitmentAliasTag = aliasTag;
      return this;
    }

    public Builder setNullifierDigest(final byte[] digest) {
      this.nullifierDigest = KaigiInstructionUtils.canonicalizeOptionalHash(digest);
      return this;
    }

    public Builder setNullifierDigest(final String digestHexOrLiteral) {
      this.nullifierDigest = KaigiInstructionUtils.canonicalizeOptionalHash(digestHexOrLiteral);
      return this;
    }

    public Builder setNullifierIssuedAtMs(final Long issuedAtMs) {
      if (issuedAtMs != null && issuedAtMs < 0) {
        throw new IllegalArgumentException("nullifier issuedAtMs must be non-negative");
      }
      this.nullifierIssuedAtMs = issuedAtMs;
      return this;
    }

    public Builder setRosterRoot(final byte[] rosterRoot) {
      this.rosterRoot = KaigiInstructionUtils.canonicalizeOptionalHash(rosterRoot);
      return this;
    }

    public Builder setRosterRoot(final String rosterRootHexOrLiteral) {
      this.rosterRoot = KaigiInstructionUtils.canonicalizeOptionalHash(rosterRootHexOrLiteral);
      return this;
    }

    Builder setRosterRootLiteral(final String literal) {
      this.rosterRoot = literal;
      return this;
    }

    public Builder setProof(final byte[] proofBytes) {
      this.proofBase64 = proofBytes == null ? null : KaigiInstructionUtils.toBase64(proofBytes);
      return this;
    }

    public Builder setProofBase64(final String proofBase64) {
      this.proofBase64 =
          proofBase64 == null ? null : KaigiInstructionUtils.requireBase64(proofBase64, "proof");
      return this;
    }

    public JoinKaigiInstruction build() {
      if (callId == null) {
        throw new IllegalStateException("callId must be provided");
      }
      if (participant == null) {
        throw new IllegalStateException("participant must be provided");
      }
      return new JoinKaigiInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      KaigiInstructionUtils.appendCallId(callId, args, "call");
      args.put("participant", participant);
      if (commitment != null) {
        args.put("commitment.commitment", commitment);
        if (commitmentAliasTag != null) {
          args.put("commitment.alias_tag", commitmentAliasTag);
        }
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

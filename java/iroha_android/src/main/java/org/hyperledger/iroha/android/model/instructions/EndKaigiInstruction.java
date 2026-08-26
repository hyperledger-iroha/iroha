package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for {@code EndKaigi} instructions. */
public final class EndKaigiInstruction implements InstructionTemplate {

  private static final String ACTION = "EndKaigi";

  private final KaigiInstructionUtils.CallId callId;
  private final Long endedAtMs;
  private final String commitment;
  private final String commitmentAliasTag;
  private final String nullifierDigest;
  private final Long nullifierIssuedAtMs;
  private final String rosterRoot;
  private final String proofBase64;
  private final Map<String, String> arguments;

  private EndKaigiInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private EndKaigiInstruction(final Builder builder, final Map<String, String> argumentOrder) {
    this.callId = builder.callId;
    this.endedAtMs = builder.endedAtMs;
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

  public Long endedAtMs() {
    return endedAtMs;
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

  public static EndKaigiInstruction fromArguments(final Map<String, String> arguments) {
    KaigiInstructionUtils.requireAction(arguments, ACTION);
    final Builder builder = builder();
    builder.setCallId(KaigiInstructionUtils.parseCallId(arguments, "call"));
    final Long ended =
        KaigiInstructionUtils.parseOptionalUnsignedLong(arguments.get("ended_at_ms"), "ended_at_ms");
    builder.setEndedAtMs(ended);
    if (arguments.get("commitment.alias_tag") != null) {
      throw new IllegalArgumentException(
          "commitment aliasTag is off-chain only and must be omitted");
    }
    final String commitmentValue = arguments.get("commitment.commitment");
    if (commitmentValue != null) {
      builder.setCommitmentLiteral(commitmentValue);
    }
    final String nullifier = arguments.get("nullifier.digest");
    final Long nullifierIssuedAt =
        KaigiInstructionUtils.parseOptionalUnsignedLong(
            arguments.get("nullifier.issued_at_ms"), "nullifier.issued_at_ms");
    if (nullifierIssuedAt != null && nullifierIssuedAt.longValue() != 0L) {
      throw new IllegalArgumentException(
          "nullifier issuedAtMs is off-chain only and must be zero when provided");
    }
    if (nullifierIssuedAt != null && nullifier == null) {
      throw new IllegalArgumentException("nullifier issuedAtMs requires nullifier digest");
    }
    if (nullifier != null) {
      builder.setNullifierDigestLiteral(nullifier);
      builder.setNullifierIssuedAtMs(nullifierIssuedAt);
    }
    builder.setRosterRootLiteral(arguments.get("roster_root"));
    builder.setProofBase64(arguments.get("proof"));
    return new EndKaigiInstruction(builder, new LinkedHashMap<>(arguments));
  }

  public static Builder builder() {
    return new Builder();
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof EndKaigiInstruction other)) {
      return false;
    }
    return Objects.equals(callId.domainId(), other.callId.domainId())
        && Objects.equals(callId.callName(), other.callId.callName())
        && Objects.equals(endedAtMs, other.endedAtMs)
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
        endedAtMs,
        commitment,
        commitmentAliasTag,
        nullifierDigest,
        nullifierIssuedAtMs,
        rosterRoot,
        proofBase64);
  }

  public static final class Builder {
    private KaigiInstructionUtils.CallId callId;
    private Long endedAtMs;
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

    public Builder setEndedAtMs(final Long endedAtMs) {
      this.endedAtMs = endedAtMs;
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
      if (aliasTag != null) {
        throw new IllegalArgumentException(
            "commitment aliasTag is off-chain only and must be omitted");
      }
      this.commitmentAliasTag = null;
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

    Builder setNullifierDigestLiteral(final String literal) {
      this.nullifierDigest = literal;
      return this;
    }

    public Builder setNullifierIssuedAtMs(final Long issuedAtMs) {
      if (issuedAtMs != null && issuedAtMs.longValue() != 0L) {
        throw new IllegalArgumentException(
            "nullifier issuedAtMs is off-chain only and must be zero when provided");
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

    public EndKaigiInstruction build() {
      if (callId == null) {
        throw new IllegalStateException("callId must be provided");
      }
      if (nullifierIssuedAtMs != null && nullifierDigest == null) {
        throw new IllegalStateException("nullifier issuedAtMs requires nullifier digest");
      }
      return new EndKaigiInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      KaigiInstructionUtils.appendCallId(callId, args, "call");
      if (endedAtMs != null) {
        args.put("ended_at_ms", Long.toUnsignedString(endedAtMs));
      }
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

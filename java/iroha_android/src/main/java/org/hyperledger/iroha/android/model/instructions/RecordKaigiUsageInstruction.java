package org.hyperledger.iroha.android.model.instructions;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for {@code RecordKaigiUsage} instructions. */
public final class RecordKaigiUsageInstruction implements InstructionTemplate {

  private static final String ACTION = "RecordKaigiUsage";

  private final KaigiInstructionUtils.CallId callId;
  private final long durationMs;
  private final long billedGas;
  private final String usageCommitment;
  private final String proofBase64;
  private final Map<String, String> arguments;

  private RecordKaigiUsageInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private RecordKaigiUsageInstruction(final Builder builder, final Map<String, String> argumentOrder) {
    this.callId = builder.callId;
    this.durationMs = builder.durationMs;
    this.billedGas = builder.billedGas;
    this.usageCommitment = builder.usageCommitment;
    this.proofBase64 = builder.proofBase64;
    this.arguments = Map.copyOf(argumentOrder);
  }

  public KaigiInstructionUtils.CallId callId() {
    return callId;
  }

  public long durationMs() {
    return durationMs;
  }

  public long billedGas() {
    return billedGas;
  }

  public String usageCommitment() {
    return usageCommitment;
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

  public static RecordKaigiUsageInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder = builder();
    builder.setCallId(KaigiInstructionUtils.parseCallId(arguments, "call"));
    builder.setDurationMs(
        KaigiInstructionUtils.parsePositiveInt(arguments.get("duration_ms"), "duration_ms"));
    builder.setBilledGas(
        KaigiInstructionUtils.parseUnsignedLong(arguments.getOrDefault("billed_gas", "0"), "billed_gas"));
    builder.setUsageCommitmentLiteral(arguments.get("usage_commitment"));
    builder.setProofBase64(arguments.get("proof"));
    return new RecordKaigiUsageInstruction(builder, new LinkedHashMap<>(arguments));
  }

  public static Builder builder() {
    return new Builder();
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof RecordKaigiUsageInstruction other)) {
      return false;
    }
    return Objects.equals(callId.domainId(), other.callId.domainId())
        && Objects.equals(callId.callName(), other.callId.callName())
        && durationMs == other.durationMs
        && billedGas == other.billedGas
        && Objects.equals(usageCommitment, other.usageCommitment)
        && Objects.equals(proofBase64, other.proofBase64);
  }

  @Override
  public int hashCode() {
    return Objects.hash(
        callId.domainId(), callId.callName(), durationMs, billedGas, usageCommitment, proofBase64);
  }

  public static final class Builder {
    private KaigiInstructionUtils.CallId callId;
    private long durationMs;
    private long billedGas;
    private String usageCommitment;
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

    public Builder setDurationMs(final long durationMs) {
      if (durationMs <= 0) {
        throw new IllegalArgumentException("durationMs must be greater than zero");
      }
      this.durationMs = durationMs;
      return this;
    }

    public Builder setBilledGas(final long billedGas) {
      if (billedGas < 0) {
        throw new IllegalArgumentException("billedGas must be non-negative");
      }
      this.billedGas = billedGas;
      return this;
    }

    public Builder setUsageCommitment(final byte[] commitment) {
      this.usageCommitment = KaigiInstructionUtils.canonicalizeOptionalHash(commitment);
      return this;
    }

    public Builder setUsageCommitment(final String commitmentHexOrLiteral) {
      this.usageCommitment = KaigiInstructionUtils.canonicalizeOptionalHash(commitmentHexOrLiteral);
      return this;
    }

    Builder setUsageCommitmentLiteral(final String literal) {
      this.usageCommitment = literal;
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

    public RecordKaigiUsageInstruction build() {
      if (callId == null) {
        throw new IllegalStateException("callId must be provided");
      }
      if (durationMs <= 0) {
        throw new IllegalStateException("durationMs must be set and positive");
      }
      if (billedGas < 0) {
        throw new IllegalStateException("billedGas must be non-negative");
      }
      return new RecordKaigiUsageInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      KaigiInstructionUtils.appendCallId(callId, args, "call");
      args.put("duration_ms", Long.toUnsignedString(durationMs));
      args.put("billed_gas", Long.toUnsignedString(billedGas));
      if (usageCommitment != null) {
        args.put("usage_commitment", usageCommitment);
      }
      if (proofBase64 != null) {
        args.put("proof", proofBase64);
      }
      return args;
    }
  }
}

package org.hyperledger.iroha.android.model.instructions;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for {@code EndKaigi} instructions. */
public final class EndKaigiInstruction implements InstructionTemplate {

  private static final String ACTION = "EndKaigi";

  private final KaigiInstructionUtils.CallId callId;
  private final Long endedAtMs;
  private final Map<String, String> arguments;

  private EndKaigiInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private EndKaigiInstruction(final Builder builder, final Map<String, String> argumentOrder) {
    this.callId = builder.callId;
    this.endedAtMs = builder.endedAtMs;
    this.arguments = Map.copyOf(argumentOrder);
  }

  public KaigiInstructionUtils.CallId callId() {
    return callId;
  }

  public Long endedAtMs() {
    return endedAtMs;
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
    final Builder builder = builder();
    builder.setCallId(KaigiInstructionUtils.parseCallId(arguments, "call"));
    final Long ended =
        KaigiInstructionUtils.parseOptionalUnsignedLong(arguments.get("ended_at_ms"), "ended_at_ms");
    builder.setEndedAtMs(ended);
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
        && Objects.equals(endedAtMs, other.endedAtMs);
  }

  @Override
  public int hashCode() {
    return Objects.hash(callId.domainId(), callId.callName(), endedAtMs);
  }

  public static final class Builder {
    private KaigiInstructionUtils.CallId callId;
    private Long endedAtMs;

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
      if (endedAtMs != null && endedAtMs < 0) {
        throw new IllegalArgumentException("endedAtMs must be non-negative");
      }
      this.endedAtMs = endedAtMs;
      return this;
    }

    public EndKaigiInstruction build() {
      if (callId == null) {
        throw new IllegalStateException("callId must be provided");
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
      return args;
    }
  }
}


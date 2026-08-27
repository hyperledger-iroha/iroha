package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for {@code UnregisterKaigiRelay} instructions. */
public final class UnregisterKaigiRelayInstruction implements InstructionTemplate {

  private static final String ACTION = "UnregisterKaigiRelay";
  private static final java.util.Set<String> ALLOWED_ARGUMENTS =
      KaigiInstructionUtils.argumentSet("action", "relay_id");

  private final String relayId;
  private final Map<String, String> arguments;

  private UnregisterKaigiRelayInstruction(final Builder builder) {
    this.relayId = builder.relayId;
    final Map<String, String> values = new LinkedHashMap<>();
    values.put("action", ACTION);
    values.put("relay_id", relayId);
    this.arguments = Collections.unmodifiableMap(values);
  }

  public String relayId() {
    return relayId;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static UnregisterKaigiRelayInstruction fromArguments(
      final Map<String, String> arguments) {
    KaigiInstructionUtils.requireKnownArguments(arguments, ALLOWED_ARGUMENTS);
    KaigiInstructionUtils.requireAction(arguments, ACTION);
    return builder().setRelayId(KaigiInstructionUtils.require(arguments, "relay_id")).build();
  }

  public static Builder builder() {
    return new Builder();
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof UnregisterKaigiRelayInstruction other)) {
      return false;
    }
    return Objects.equals(relayId, other.relayId);
  }

  @Override
  public int hashCode() {
    return Objects.hash(relayId);
  }

  public static final class Builder {
    private String relayId;

    private Builder() {}

    public Builder setRelayId(final String relayId) {
      if (relayId == null || relayId.isBlank()) {
        throw new IllegalArgumentException("relayId must not be blank");
      }
      this.relayId = relayId;
      return this;
    }

    public UnregisterKaigiRelayInstruction build() {
      if (relayId == null) {
        throw new IllegalStateException("relayId must be provided");
      }
      return new UnregisterKaigiRelayInstruction(this);
    }
  }
}

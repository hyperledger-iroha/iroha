package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for `BurnTriggerRepetitions` instructions. */
public final class BurnTriggerRepetitionsInstruction implements InstructionTemplate {

  private static final String ACTION = "BurnTriggerRepetitions";

  private final String triggerId;
  private final int repetitions;
  private final Map<String, String> arguments;

  private BurnTriggerRepetitionsInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private BurnTriggerRepetitionsInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.triggerId = builder.triggerId;
    this.repetitions = builder.repetitions;
    this.arguments = Collections.unmodifiableMap(new LinkedHashMap<>(argumentOrder));
  }

  public String triggerId() {
    return triggerId;
  }

  public int repetitions() {
    return repetitions;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.BURN;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static BurnTriggerRepetitionsInstruction fromArguments(final Map<String, String> arguments) {
    return new BurnTriggerRepetitionsInstruction(
        builder()
            .setTriggerId(require(arguments, "trigger"))
            .setRepetitions(parseUnsignedInt(require(arguments, "repetitions"))),
        new LinkedHashMap<>(arguments));
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  private static int parseUnsignedInt(final String value) {
    try {
      final int parsed = Integer.parseUnsignedInt(value);
      if (parsed < 0) {
        throw new NumberFormatException("negative");
      }
      return parsed;
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException("repetitions must be an unsigned integer", ex);
    }
  }

  public static Builder builder() {
    return new Builder();
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof BurnTriggerRepetitionsInstruction other)) {
      return false;
    }
    return repetitions == other.repetitions && Objects.equals(triggerId, other.triggerId);
  }

  @Override
  public int hashCode() {
    return Objects.hash(triggerId, repetitions);
  }

  public static final class Builder {
    private String triggerId;
    private int repetitions;

    private Builder() {}

    public Builder setTriggerId(final String triggerId) {
      if (triggerId == null || triggerId.isBlank()) {
        throw new IllegalArgumentException("triggerId must not be blank");
      }
      this.triggerId = triggerId;
      return this;
    }

    public Builder setRepetitions(final int repetitions) {
      if (repetitions < 0) {
        throw new IllegalArgumentException("repetitions must be non-negative");
      }
      this.repetitions = repetitions;
      return this;
    }

    public BurnTriggerRepetitionsInstruction build() {
      if (triggerId == null) {
        throw new IllegalStateException("triggerId must be provided");
      }
      return new BurnTriggerRepetitionsInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("trigger", triggerId);
      args.put("repetitions", Integer.toUnsignedString(repetitions));
      return args;
    }
  }
}

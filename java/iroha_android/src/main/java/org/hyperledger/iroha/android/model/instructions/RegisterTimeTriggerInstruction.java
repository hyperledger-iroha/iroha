package org.hyperledger.iroha.android.model.instructions;

import java.util.ArrayList;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.InstructionBox;

/** Typed builder for {@code RegisterTimeTrigger} instructions. */
public final class RegisterTimeTriggerInstruction implements InstructionTemplate {

public static final String ACTION = "RegisterTimeTrigger";
  static final String REPEATS_INDEFINITE = "indefinite";

  private final String triggerId;
  private final String authority;
  private final long startMs;
  private final Long periodMs;
  private final Integer repeats;
  private final List<InstructionBox> instructions;
  private final Map<String, String> metadata;
  private final Map<String, String> arguments;

  private RegisterTimeTriggerInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private RegisterTimeTriggerInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.triggerId = builder.triggerId;
    this.authority = builder.authority;
    this.startMs = builder.startMs;
    this.periodMs = builder.periodMs;
    this.repeats = builder.repeats;
    this.instructions = Collections.unmodifiableList(new ArrayList<>(builder.instructions));
    this.metadata =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(builder.metadata)));
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  public String triggerId() {
    return triggerId;
  }

  public String authority() {
    return authority;
  }

  public long startMs() {
    return startMs;
  }

  public Long periodMs() {
    return periodMs;
  }

  public Integer repeats() {
    return repeats;
  }

  public List<InstructionBox> instructions() {
    return instructions;
  }

  public Map<String, String> metadata() {
    return metadata;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.REGISTER;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static RegisterTimeTriggerInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setTriggerId(require(arguments, "trigger"))
            .setAuthority(require(arguments, "authority"))
            .setStartMs(parsePositiveLong(require(arguments, "start_ms"), "start_ms"))
            .setInstructions(TriggerInstructionUtils.parseInstructions(arguments))
            .setMetadata(TriggerInstructionUtils.extractMetadata(arguments));
    if (arguments.containsKey("period_ms")) {
      builder.setPeriodMs(parsePositiveLong(arguments.get("period_ms"), "period_ms"));
    }
    final Integer repeats = TriggerInstructionUtils.parseRepeats(arguments.get("repeats"));
    if (repeats != null) {
      builder.setRepeats(repeats);
    }
    return new RegisterTimeTriggerInstruction(builder, new LinkedHashMap<>(arguments));
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  private static long parsePositiveLong(final String value, final String field) {
    try {
      final long parsed = Long.parseUnsignedLong(value);
      if (parsed <= 0) {
        throw new IllegalArgumentException(field + " must be greater than zero");
      }
      return parsed;
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException(field + " must be an unsigned integer", ex);
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
    if (!(obj instanceof RegisterTimeTriggerInstruction other)) {
      return false;
    }
    return startMs == other.startMs
        && Objects.equals(triggerId, other.triggerId)
        && Objects.equals(authority, other.authority)
        && Objects.equals(periodMs, other.periodMs)
        && Objects.equals(repeats, other.repeats)
        && instructions.equals(other.instructions)
        && metadata.equals(other.metadata);
  }

  @Override
  public int hashCode() {
    return Objects.hash(triggerId, authority, startMs, periodMs, repeats, instructions, metadata);
  }

  public static final class Builder {
    private String triggerId;
    private String authority;
    private Long startMs;
    private Long periodMs;
    private Integer repeats;
    private final List<InstructionBox> instructions = new ArrayList<>();
    private final Map<String, String> metadata = new LinkedHashMap<>();

    private Builder() {}

    public Builder setTriggerId(final String triggerId) {
      if (triggerId == null || triggerId.isBlank()) {
        throw new IllegalArgumentException("triggerId must not be blank");
      }
      this.triggerId = triggerId;
      return this;
    }

    public Builder setAuthority(final String authority) {
      this.authority =
          org.hyperledger.iroha.android.address.AccountIdLiteral.requireCanonicalI105Address(
              authority, "authority");
      return this;
    }

    public Builder setStartMs(final long startMs) {
      if (startMs <= 0) {
        throw new IllegalArgumentException("startMs must be greater than zero");
      }
      this.startMs = startMs;
      return this;
    }

    public Builder setPeriodMs(final Long periodMs) {
      if (periodMs == null) {
        this.periodMs = null;
        return this;
      }
      if (periodMs <= 0) {
        throw new IllegalArgumentException("periodMs must be greater than zero when provided");
      }
      this.periodMs = periodMs;
      return this;
    }

    public Builder setRepeats(final Integer repeats) {
      if (repeats == null) {
        this.repeats = null;
        return this;
      }
      if (repeats <= 0) {
        throw new IllegalArgumentException("repeats must be greater than zero when provided");
      }
      this.repeats = repeats;
      return this;
    }

    public Builder addInstruction(final InstructionBox instruction) {
      instructions.add(Objects.requireNonNull(instruction, "instruction"));
      return this;
    }

    public Builder setInstructions(final List<InstructionBox> newInstructions) {
      instructions.clear();
      if (newInstructions != null) {
        newInstructions.forEach(this::addInstruction);
      }
      return this;
    }

    public Builder putMetadata(final String key, final String value) {
      metadata.put(Objects.requireNonNull(key, "metadata key"), Objects.requireNonNull(value, "metadata value"));
      return this;
    }

    public Builder setMetadata(final Map<String, String> entries) {
      metadata.clear();
      if (entries != null) {
        entries.forEach(this::putMetadata);
      }
      return this;
    }

    public RegisterTimeTriggerInstruction build() {
      if (triggerId == null) {
        throw new IllegalStateException("triggerId must be set");
      }
      if (authority == null) {
        throw new IllegalStateException("authority must be set");
      }
      if (startMs == null) {
        throw new IllegalStateException("startMs must be set");
      }
      if (instructions.isEmpty()) {
        throw new IllegalStateException("at least one instruction must be provided");
      }
      return new RegisterTimeTriggerInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("trigger", triggerId);
      args.put("authority", authority);
      args.put("start_ms", Long.toUnsignedString(startMs));
      if (periodMs != null) {
        args.put("period_ms", Long.toUnsignedString(periodMs));
      }
      args.put(
          "repeats",
          repeats == null ? REPEATS_INDEFINITE : Integer.toUnsignedString(repeats));
      TriggerInstructionUtils.appendInstructions(instructions, args);
      TriggerInstructionUtils.appendMetadata(metadata, args);
      return args;
    }
  }
}

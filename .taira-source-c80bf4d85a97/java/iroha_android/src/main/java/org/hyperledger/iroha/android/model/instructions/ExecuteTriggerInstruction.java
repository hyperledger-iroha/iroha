package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.InstructionBox;

/** Typed builder for {@code ExecuteTrigger} instructions. */
public final class ExecuteTriggerInstruction implements InstructionTemplate {

  private final String triggerId;
  private final String authorityOverride;
  private final Map<String, String> arguments;

  private ExecuteTriggerInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private ExecuteTriggerInstruction(final Builder builder, final Map<String, String> arguments) {
    this.triggerId = builder.triggerId;
    this.authorityOverride = builder.authorityOverride;
    this.arguments = Collections.unmodifiableMap(new LinkedHashMap<>(arguments));
  }

  /** Returns the trigger identifier to be executed. */
  public String triggerId() {
    return triggerId;
  }

  /**
   * Returns the optional authority override. When present the trigger executes as the provided
   * account.
   */
  public String authorityOverride() {
    return authorityOverride;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.EXECUTE_TRIGGER;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static ExecuteTriggerInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder().setTriggerId(require(arguments, "trigger")).setAuthorityOverride(arguments.get("authority"));
    return new ExecuteTriggerInstruction(builder, new LinkedHashMap<>(arguments));
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  public static Builder builder() {
    return new Builder();
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof ExecuteTriggerInstruction other)) {
      return false;
    }
    return Objects.equals(triggerId, other.triggerId)
        && Objects.equals(authorityOverride, other.authorityOverride);
  }

  @Override
  public int hashCode() {
    return Objects.hash(triggerId, authorityOverride);
  }

  public static final class Builder {
    private String triggerId;
    private String authorityOverride;

    private Builder() {}

    public Builder setTriggerId(final String triggerId) {
      if (triggerId == null || triggerId.isBlank()) {
        throw new IllegalArgumentException("triggerId must not be blank");
      }
      this.triggerId = triggerId;
      return this;
    }

    public Builder setAuthorityOverride(final String authorityOverride) {
      if (authorityOverride != null && authorityOverride.isBlank()) {
        throw new IllegalArgumentException("authorityOverride must not be blank when present");
      }
      this.authorityOverride = authorityOverride;
      return this;
    }

    public ExecuteTriggerInstruction build() {
      if (triggerId == null) {
        throw new IllegalStateException("triggerId must be provided");
      }
      return new ExecuteTriggerInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", "ExecuteTrigger");
      args.put("trigger", triggerId);
      if (authorityOverride != null) {
        args.put("authority", authorityOverride);
      }
      return args;
    }
  }
}

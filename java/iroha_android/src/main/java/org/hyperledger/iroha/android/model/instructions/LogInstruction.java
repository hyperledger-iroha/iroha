package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for {@code Log} instructions. */
public final class LogInstruction implements InstructionTemplate {

  private final String level;
  private final String message;
  private final Map<String, String> arguments;

  private LogInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private LogInstruction(final Builder builder, final Map<String, String> arguments) {
    this.level = builder.level;
    this.message = builder.message;
    this.arguments = Collections.unmodifiableMap(new LinkedHashMap<>(arguments));
  }

  /** Returns the log level (INFO/WARN/ERROR/DEBUG). */
  public String level() {
    return level;
  }

  /** Returns the log message payload. */
  public String message() {
    return message;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.LOG;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static LogInstruction fromArguments(final Map<String, String> arguments) {
    return new LogInstruction(
        builder()
            .setLevel(require(arguments, "level"))
            .setMessage(require(arguments, "message")),
        new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof LogInstruction other)) {
      return false;
    }
    return Objects.equals(level, other.level) && Objects.equals(message, other.message);
  }

  @Override
  public int hashCode() {
    return Objects.hash(level, message);
  }

  public static final class Builder {
    private String level = "INFO";
    private String message;

    private Builder() {}

    public Builder setLevel(final String level) {
      if (level != null && !level.isBlank()) {
        this.level = level;
      }
      return this;
    }

    public Builder setMessage(final String message) {
      this.message = Objects.requireNonNull(message, "message");
      return this;
    }

    public LogInstruction build() {
      if (message == null || message.isBlank()) {
        throw new IllegalStateException("message must be provided");
      }
      return new LogInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", "Log");
      args.put("level", level);
      args.put("message", message);
      return args;
    }
  }
}

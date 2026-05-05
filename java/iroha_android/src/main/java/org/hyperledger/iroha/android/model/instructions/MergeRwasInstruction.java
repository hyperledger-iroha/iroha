package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/**
 * Typed builder for the {@code MergeRwas} instruction.
 *
 * <p>The payload stores the canonical JSON representation of the merge request.
 */
public final class MergeRwasInstruction implements InstructionTemplate {

  private static final String ACTION = "MergeRwas";

  private final String mergeJson;
  private final Map<String, String> arguments;

  private MergeRwasInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private MergeRwasInstruction(final Builder builder, final Map<String, String> argumentOrder) {
    this.mergeJson = builder.mergeJson;
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  /** Returns the canonical JSON representation of the merge request payload. */
  public String mergeJson() {
    return mergeJson;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static MergeRwasInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder = builder().setMergeJson(require(arguments, "merge"));
    return new MergeRwasInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof MergeRwasInstruction other)) {
      return false;
    }
    return Objects.equals(mergeJson, other.mergeJson);
  }

  @Override
  public int hashCode() {
    return Objects.hash(mergeJson);
  }

  public static final class Builder {
    private String mergeJson;

    private Builder() {}

    public Builder setMergeJson(final String mergeJson) {
      this.mergeJson = Objects.requireNonNull(mergeJson, "mergeJson");
      return this;
    }

    public MergeRwasInstruction build() {
      if (mergeJson == null || mergeJson.isBlank()) {
        throw new IllegalStateException("mergeJson must be provided");
      }
      return new MergeRwasInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("merge", mergeJson);
      return args;
    }
  }
}

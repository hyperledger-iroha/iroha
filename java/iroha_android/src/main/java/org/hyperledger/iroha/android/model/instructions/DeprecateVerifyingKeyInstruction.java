package org.hyperledger.iroha.android.model.instructions;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for {@code DeprecateVerifyingKey} instructions. */
public final class DeprecateVerifyingKeyInstruction implements InstructionTemplate {

  private static final String ACTION = "DeprecateVerifyingKey";

  private final String backend;
  private final String name;
  private final Map<String, String> arguments;

  private DeprecateVerifyingKeyInstruction(final Builder builder, final Map<String, String> args) {
    this.backend = builder.backend;
    this.name = builder.name;
    this.arguments = Map.copyOf(args);
  }

  public String backend() {
    return backend;
  }

  public String name() {
    return name;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static DeprecateVerifyingKeyInstruction fromArguments(
      final Map<String, String> arguments) {
    final String backend = VerifyingKeyInstructionUtils.require(arguments, "backend");
    final String name = VerifyingKeyInstructionUtils.require(arguments, "name");
    return builder().setBackend(backend).setName(name).build();
  }

  public static Builder builder() {
    return new Builder();
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof DeprecateVerifyingKeyInstruction other)) {
      return false;
    }
    return Objects.equals(backend, other.backend) && Objects.equals(name, other.name);
  }

  @Override
  public int hashCode() {
    return Objects.hash(backend, name);
  }

  public static final class Builder {
    private String backend;
    private String name;

    private Builder() {}

    public Builder setBackend(final String backend) {
      if (backend == null || backend.trim().isEmpty()) {
        throw new IllegalArgumentException("backend must not be blank");
      }
      this.backend = backend.trim();
      return this;
    }

    public Builder setName(final String name) {
      if (name == null || name.trim().isEmpty()) {
        throw new IllegalArgumentException("name must not be blank");
      }
      this.name = name.trim();
      return this;
    }

    public DeprecateVerifyingKeyInstruction build() {
      if (backend == null) {
        throw new IllegalStateException("backend must be provided");
      }
      if (name == null) {
        throw new IllegalStateException("name must be provided");
      }
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("backend", backend);
      args.put("name", name);
      return new DeprecateVerifyingKeyInstruction(this, args);
    }
  }
}


package org.hyperledger.iroha.android.model.instructions;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.zk.VerifyingKeyRecordDescription;

/** Typed builder for {@code RegisterVerifyingKey} instructions. */
public final class RegisterVerifyingKeyInstruction implements InstructionTemplate {

  private static final String ACTION = "RegisterVerifyingKey";

  private final String backend;
  private final String name;
  private final VerifyingKeyRecordDescription record;
  private final Map<String, String> arguments;

  private RegisterVerifyingKeyInstruction(final Builder builder, final Map<String, String> args) {
    this.backend = builder.backend;
    this.name = builder.name;
    this.record = builder.record;
    this.arguments = Map.copyOf(args);
  }

  public String backend() {
    return backend;
  }

  public String name() {
    return name;
  }

  public VerifyingKeyRecordDescription record() {
    return record;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static RegisterVerifyingKeyInstruction fromArguments(final Map<String, String> arguments) {
    final String backend = VerifyingKeyInstructionUtils.require(arguments, "backend");
    final String name = VerifyingKeyInstructionUtils.require(arguments, "name");
    final VerifyingKeyRecordDescription record =
        VerifyingKeyInstructionUtils.parseRecord(arguments, backend);
    final Builder builder =
        builder().setBackend(backend).setName(name).setRecord(record);
    return builder.build();
  }

  public static Builder builder() {
    return new Builder();
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof RegisterVerifyingKeyInstruction other)) {
      return false;
    }
    return Objects.equals(backend, other.backend)
        && Objects.equals(name, other.name)
        && Objects.equals(record, other.record);
  }

  @Override
  public int hashCode() {
    return Objects.hash(backend, name, record);
  }

  public static final class Builder {
    private String backend;
    private String name;
    private VerifyingKeyRecordDescription record;

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

    public Builder setRecord(final VerifyingKeyRecordDescription record) {
      this.record = Objects.requireNonNull(record, "record");
      return this;
    }

    public RegisterVerifyingKeyInstruction build() {
      if (backend == null) {
        throw new IllegalStateException("backend must be provided");
      }
      if (name == null) {
        throw new IllegalStateException("name must be provided");
      }
      if (record == null) {
        throw new IllegalStateException("record must be provided");
      }
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("backend", backend);
      args.put("name", name);
      args.putAll(record.toArguments(backend));
      return new RegisterVerifyingKeyInstruction(this, args);
    }
  }
}


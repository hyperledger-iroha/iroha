package org.hyperledger.iroha.android.model.instructions;

import java.util.Arrays;
import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.model.InstructionBox;

/** Typed builder for {@code Upgrade} instructions carrying executor bytecode. */
public final class UpgradeInstruction implements InstructionTemplate {

  private final byte[] bytecode;
  private final Map<String, String> arguments;

  private UpgradeInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private UpgradeInstruction(final Builder builder, final Map<String, String> argumentOrder) {
    this.bytecode = builder.bytecode.clone();
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  /** Returns the executor bytecode that will replace the active executor once applied. */
  public byte[] bytecode() {
    return bytecode.clone();
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.UPGRADE;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static UpgradeInstruction fromArguments(final Map<String, String> arguments) {
    final String encoded = require(arguments, "bytecode");
    final byte[] decoded;
    try {
      decoded = Base64.getDecoder().decode(encoded);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException("bytecode argument must be base64", ex);
    }
    return new UpgradeInstruction(builder().setBytecode(decoded), new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof UpgradeInstruction other)) {
      return false;
    }
    return Arrays.equals(bytecode, other.bytecode);
  }

  @Override
  public int hashCode() {
    return Arrays.hashCode(bytecode);
  }

  public static final class Builder {
    private byte[] bytecode;

    private Builder() {}

    public Builder setBytecode(final byte[] bytecode) {
      this.bytecode = Objects.requireNonNull(bytecode, "bytecode").clone();
      if (this.bytecode.length == 0) {
        throw new IllegalArgumentException("bytecode must not be empty");
      }
      return this;
    }

    public UpgradeInstruction build() {
      if (bytecode == null || bytecode.length == 0) {
        throw new IllegalStateException("bytecode must be provided");
      }
      return new UpgradeInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", "UpgradeExecutor");
      args.put("bytecode", Base64.getEncoder().encodeToString(bytecode));
      return args;
    }
  }
}

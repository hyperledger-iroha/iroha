package org.hyperledger.iroha.android.model.instructions;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for {@code EnactReferendum} instructions. */
public final class EnactReferendumInstruction implements InstructionTemplate {

  private static final String ACTION = "EnactReferendum";

  private final String referendumIdHex;
  private final String preimageHashHex;
  private final GovernanceInstructionUtils.AtWindow window;
  private final Map<String, String> arguments;

  private EnactReferendumInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private EnactReferendumInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.referendumIdHex = builder.referendumIdHex;
    this.preimageHashHex = builder.preimageHashHex;
    this.window = builder.window;
    this.arguments = Map.copyOf(argumentOrder);
  }

  public String referendumIdHex() {
    return referendumIdHex;
  }

  public String preimageHashHex() {
    return preimageHashHex;
  }

  public GovernanceInstructionUtils.AtWindow window() {
    return window;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static Builder builder() {
    return new Builder();
  }

  public static EnactReferendumInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setReferendumIdHex(require(arguments, "referendum_id_hex"))
            .setPreimageHashHex(require(arguments, "preimage_hash_hex"));
    builder.setWindow(
        GovernanceInstructionUtils.parseAtWindow(arguments, "window", "enactment window"));
    return new EnactReferendumInstruction(builder, new LinkedHashMap<>(arguments));
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof EnactReferendumInstruction other)) {
      return false;
    }
    return Objects.equals(referendumIdHex, other.referendumIdHex)
        && Objects.equals(preimageHashHex, other.preimageHashHex)
        && Objects.equals(window, other.window);
  }

  @Override
  public int hashCode() {
    return Objects.hash(referendumIdHex, preimageHashHex, window);
  }

  public static final class Builder {
    private String referendumIdHex;
    private String preimageHashHex;
    private GovernanceInstructionUtils.AtWindow window;

    private Builder() {}

    public Builder setReferendumIdHex(final String referendumIdHex) {
      this.referendumIdHex =
          GovernanceInstructionUtils.requireHex(referendumIdHex, "referendumIdHex", 32);
      return this;
    }

    public Builder setPreimageHashHex(final String preimageHashHex) {
      this.preimageHashHex =
          GovernanceInstructionUtils.requireHex(preimageHashHex, "preimageHashHex", 32);
      return this;
    }

    public Builder setWindow(final GovernanceInstructionUtils.AtWindow window) {
      this.window = Objects.requireNonNull(window, "window");
      return this;
    }

    public EnactReferendumInstruction build() {
      if (referendumIdHex == null) {
        throw new IllegalStateException("referendumIdHex must be provided");
      }
      if (preimageHashHex == null) {
        throw new IllegalStateException("preimageHashHex must be provided");
      }
      if (window == null) {
        throw new IllegalStateException("window must be provided");
      }
      return new EnactReferendumInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("referendum_id_hex", referendumIdHex);
      args.put("preimage_hash_hex", preimageHashHex);
      GovernanceInstructionUtils.appendAtWindow(args, window, "window");
      return args;
    }
  }
}

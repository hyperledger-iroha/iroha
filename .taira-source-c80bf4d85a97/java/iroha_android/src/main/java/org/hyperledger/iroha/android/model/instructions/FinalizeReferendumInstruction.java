package org.hyperledger.iroha.android.model.instructions;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for {@code FinalizeReferendum} instructions. */
public final class FinalizeReferendumInstruction implements InstructionTemplate {

  private static final String ACTION = "FinalizeReferendum";

  private final String referendumId;
  private final String proposalIdHex;
  private final Map<String, String> arguments;

  private FinalizeReferendumInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private FinalizeReferendumInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.referendumId = builder.referendumId;
    this.proposalIdHex = builder.proposalIdHex;
    this.arguments = Map.copyOf(argumentOrder);
  }

  public String referendumId() {
    return referendumId;
  }

  public String proposalIdHex() {
    return proposalIdHex;
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

  public static FinalizeReferendumInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setReferendumId(require(arguments, "referendum_id"))
            .setProposalIdHex(require(arguments, "proposal_id_hex"));
    return new FinalizeReferendumInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof FinalizeReferendumInstruction other)) {
      return false;
    }
    return Objects.equals(referendumId, other.referendumId)
        && Objects.equals(proposalIdHex, other.proposalIdHex);
  }

  @Override
  public int hashCode() {
    return Objects.hash(referendumId, proposalIdHex);
  }

  public static final class Builder {
    private String referendumId;
    private String proposalIdHex;

    private Builder() {}

    public Builder setReferendumId(final String referendumId) {
      if (referendumId == null || referendumId.isBlank()) {
        throw new IllegalArgumentException("referendumId must not be blank");
      }
      this.referendumId = referendumId;
      return this;
    }

    public Builder setProposalIdHex(final String proposalIdHex) {
      this.proposalIdHex =
          GovernanceInstructionUtils.requireHex(proposalIdHex, "proposalIdHex", 32);
      return this;
    }

    public FinalizeReferendumInstruction build() {
      if (referendumId == null) {
        throw new IllegalStateException("referendumId must be provided");
      }
      if (proposalIdHex == null) {
        throw new IllegalStateException("proposalIdHex must be provided");
      }
      return new FinalizeReferendumInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("referendum_id", referendumId);
      args.put("proposal_id_hex", proposalIdHex);
      return args;
    }
  }
}

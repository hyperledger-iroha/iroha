package org.hyperledger.iroha.android.model.instructions;

import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.StringJoiner;

/** Typed builder for {@code PersistCouncilForEpoch} instructions. */
public final class PersistCouncilForEpochInstruction implements InstructionTemplate {

  private static final String ACTION = "PersistCouncilForEpoch";

  private final long epoch;
  private final List<String> members;
  private final List<String> alternates;
  private final Map<String, String> arguments;

  private PersistCouncilForEpochInstruction(final Builder builder) {
    this.epoch = builder.epoch;
    this.members = List.copyOf(builder.members);
    this.alternates = List.copyOf(builder.alternates);
    this.arguments = Map.copyOf(builder.canonicalArguments());
  }

  public long epoch() {
    return epoch;
  }

  public List<String> members() {
    return members;
  }

  public List<String> alternates() {
    return alternates;
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

  public static PersistCouncilForEpochInstruction fromArguments(
      final Map<String, String> arguments) {
    final Builder builder =
        builder().setEpoch(parseLong(require(arguments, "epoch"), "epoch"));

    final String membersCsv = arguments.get("members");
    if (membersCsv != null && !membersCsv.isBlank()) {
      for (final String member : membersCsv.split(",")) {
        final String trimmed = member.trim();
        if (!trimmed.isEmpty()) {
          builder.addMember(trimmed);
        }
      }
    }
    final String alternatesCsv = arguments.get("alternates");
    if (alternatesCsv != null && !alternatesCsv.isBlank()) {
      for (final String alternate : alternatesCsv.split(",")) {
        final String trimmed = alternate.trim();
        if (!trimmed.isEmpty()) {
          builder.addAlternate(trimmed);
        }
      }
    }
    return builder.build();
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  private static long parseLong(final String value, final String field) {
    try {
      final long parsed = Long.parseLong(value);
      if (parsed < 0) {
        throw new IllegalArgumentException(field + " must be non-negative");
      }
      return parsed;
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException(field + " must be numeric: " + value, ex);
    }
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof PersistCouncilForEpochInstruction other)) {
      return false;
    }
    return epoch == other.epoch
        && Objects.equals(members, other.members)
        && Objects.equals(alternates, other.alternates);
  }

  @Override
  public int hashCode() {
    return Objects.hash(epoch, members, alternates);
  }

  public static final class Builder {
    private Long epoch;
    private final List<String> members = new ArrayList<>();
    private final List<String> alternates = new ArrayList<>();

    private Builder() {}

    public Builder setEpoch(final long epoch) {
      if (epoch < 0) {
        throw new IllegalArgumentException("epoch must be non-negative");
      }
      this.epoch = epoch;
      return this;
    }

    public Builder addMember(final String accountId) {
      this.members.add(
          org.hyperledger.iroha.android.address.AccountIdLiteral.requireCanonicalI105Address(
              accountId, "member account id"));
      return this;
    }

    public Builder setMembers(final List<String> members) {
      Objects.requireNonNull(members, "members");
      this.members.clear();
      members.stream().filter(Objects::nonNull).forEach(value -> addMember(value.trim()));
      return this;
    }

    public Builder addAlternate(final String accountId) {
      this.alternates.add(
          org.hyperledger.iroha.android.address.AccountIdLiteral.requireCanonicalI105Address(
              accountId, "alternate account id"));
      return this;
    }

    public Builder setAlternates(final List<String> alternates) {
      Objects.requireNonNull(alternates, "alternates");
      this.alternates.clear();
      alternates.stream()
          .filter(Objects::nonNull)
          .forEach(value -> addAlternate(value.trim()));
      return this;
    }

    public PersistCouncilForEpochInstruction build() {
      if (epoch == null) {
        throw new IllegalStateException("epoch must be provided");
      }
      if (members.isEmpty()) {
        throw new IllegalStateException("members must contain at least one account id");
      }
      return new PersistCouncilForEpochInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("epoch", Long.toString(epoch));
      final StringJoiner joiner = new StringJoiner(",");
      for (final String member : members) {
        joiner.add(member);
      }
      args.put("members", joiner.toString());
      final StringJoiner alternatesJoiner = new StringJoiner(",");
      for (final String alternate : alternates) {
        alternatesJoiner.add(alternate);
      }
      args.put("alternates", alternatesJoiner.toString());
      return args;
    }
  }
}

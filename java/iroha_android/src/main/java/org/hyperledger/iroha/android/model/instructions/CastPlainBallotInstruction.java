package org.hyperledger.iroha.android.model.instructions;

import java.math.BigInteger;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/** Typed builder for {@code CastPlainBallot} instructions. */
public final class CastPlainBallotInstruction implements InstructionTemplate {

  private static final String ACTION = "CastPlainBallot";

  private final String referendumId;
  private final String ownerAccountId;
  private final String amount;
  private final long durationBlocks;
  private final int direction;
  private final Map<String, String> arguments;

  private CastPlainBallotInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private CastPlainBallotInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.referendumId = builder.referendumId;
    this.ownerAccountId = builder.ownerAccountId;
    this.amount = builder.amount;
    this.durationBlocks = builder.durationBlocks;
    this.direction = builder.direction;
    this.arguments = Map.copyOf(argumentOrder);
  }

  public String referendumId() {
    return referendumId;
  }

  public String ownerAccountId() {
    return ownerAccountId;
  }

  public String amount() {
    return amount;
  }

  public long durationBlocks() {
    return durationBlocks;
  }

  public int direction() {
    return direction;
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

  public static CastPlainBallotInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setReferendumId(require(arguments, "referendum_id"))
            .setOwnerAccountId(require(arguments, "owner"))
            .setAmount(require(arguments, "amount"))
            .setDurationBlocks(parseLong(require(arguments, "duration_blocks"), "duration_blocks"))
            .setDirection(parseDirection(require(arguments, "direction")));
    return new CastPlainBallotInstruction(builder, new LinkedHashMap<>(arguments));
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
      return Long.parseLong(value);
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException(field + " must be a number: " + value, ex);
    }
  }

  private static int parseDirection(final String value) {
    try {
      final int parsed = Integer.parseInt(value);
      if (parsed < 0 || parsed > 0xFF) {
        throw new IllegalArgumentException("direction must be between 0 and 255");
      }
      return parsed;
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException("direction must be numeric: " + value, ex);
    }
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof CastPlainBallotInstruction other)) {
      return false;
    }
    return Objects.equals(referendumId, other.referendumId)
        && Objects.equals(ownerAccountId, other.ownerAccountId)
        && Objects.equals(amount, other.amount)
        && durationBlocks == other.durationBlocks
        && direction == other.direction;
  }

  @Override
  public int hashCode() {
    return Objects.hash(referendumId, ownerAccountId, amount, durationBlocks, direction);
  }

  public static final class Builder {
    private String referendumId;
    private String ownerAccountId;
    private String amount;
    private Long durationBlocks;
    private Integer direction;

    private Builder() {}

    public Builder setReferendumId(final String referendumId) {
      if (referendumId == null || referendumId.isBlank()) {
        throw new IllegalArgumentException("referendumId must not be blank");
      }
      this.referendumId = referendumId;
      return this;
    }

    public Builder setOwnerAccountId(final String ownerAccountId) {
      this.ownerAccountId =
          org.hyperledger.iroha.android.address.AccountIdLiteral.requireCanonicalI105Address(
              ownerAccountId, "ownerAccountId");
      return this;
    }

    public Builder setAmount(final String amount) {
      if (amount == null || amount.isBlank()) {
        throw new IllegalArgumentException("amount must not be blank");
      }
      // Validate numeric content without constraining size
      try {
        final BigInteger parsed = new BigInteger(amount);
        if (parsed.signum() < 0) {
          throw new IllegalArgumentException("amount must be non-negative");
        }
      } catch (final NumberFormatException ex) {
        throw new IllegalArgumentException("amount must be a non-negative integer", ex);
      }
      this.amount = amount;
      return this;
    }

    public Builder setAmount(final BigInteger amount) {
      Objects.requireNonNull(amount, "amount");
      if (amount.signum() < 0) {
        throw new IllegalArgumentException("amount must be non-negative");
      }
      this.amount = amount.toString();
      return this;
    }

    public Builder setDurationBlocks(final long durationBlocks) {
      if (durationBlocks < 0) {
        throw new IllegalArgumentException("durationBlocks must be non-negative");
      }
      this.durationBlocks = durationBlocks;
      return this;
    }

    public Builder setDirection(final int direction) {
      if (direction < 0 || direction > 0xFF) {
        throw new IllegalArgumentException("direction must be between 0 and 255");
      }
      this.direction = direction;
      return this;
    }

    public CastPlainBallotInstruction build() {
      if (referendumId == null) {
        throw new IllegalStateException("referendumId must be provided");
      }
      if (ownerAccountId == null) {
        throw new IllegalStateException("ownerAccountId must be provided");
      }
      if (amount == null) {
        throw new IllegalStateException("amount must be provided");
      }
      if (durationBlocks == null) {
        throw new IllegalStateException("durationBlocks must be provided");
      }
      if (direction == null) {
        throw new IllegalStateException("direction must be provided");
      }
      return new CastPlainBallotInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("referendum_id", referendumId);
      args.put("owner", ownerAccountId);
      args.put("amount", amount);
      args.put("duration_blocks", Long.toString(durationBlocks));
      args.put("direction", Integer.toString(direction));
      return args;
    }
  }
}

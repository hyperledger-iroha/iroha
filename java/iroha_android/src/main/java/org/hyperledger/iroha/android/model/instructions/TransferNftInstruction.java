package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.address.AccountIdLiteral;

/** Typed builder for the {@code TransferNft} instruction. */
public final class TransferNftInstruction implements InstructionTemplate {

  private static final String ACTION = "TransferNft";

  private final String sourceAccountId;
  private final String nftId;
  private final String destinationAccountId;
  private final Map<String, String> arguments;

  private TransferNftInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private TransferNftInstruction(final Builder builder, final Map<String, String> argumentOrder) {
    this.sourceAccountId = builder.sourceAccountId;
    this.nftId = builder.nftId;
    this.destinationAccountId = builder.destinationAccountId;
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  public String sourceAccountId() {
    return sourceAccountId;
  }

  public String nftId() {
    return nftId;
  }

  public String destinationAccountId() {
    return destinationAccountId;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.TRANSFER;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static TransferNftInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setSourceAccountId(require(arguments, "source"))
            .setNftId(require(arguments, "nft"))
            .setDestinationAccountId(require(arguments, "destination"));
    return new TransferNftInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof TransferNftInstruction other)) {
      return false;
    }
    return Objects.equals(sourceAccountId, other.sourceAccountId)
        && Objects.equals(nftId, other.nftId)
        && Objects.equals(destinationAccountId, other.destinationAccountId);
  }

  @Override
  public int hashCode() {
    return Objects.hash(sourceAccountId, nftId, destinationAccountId);
  }

  public static final class Builder {
    private String sourceAccountId;
    private String nftId;
    private String destinationAccountId;

    private Builder() {}

    public Builder setSourceAccountId(final String sourceAccountId) {
      this.sourceAccountId =
          AccountIdLiteral.extractIh58Address(
              Objects.requireNonNull(sourceAccountId, "sourceAccountId"));
      return this;
    }

    public Builder setNftId(final String nftId) {
      this.nftId = Objects.requireNonNull(nftId, "nftId");
      return this;
    }

    public Builder setDestinationAccountId(final String destinationAccountId) {
      this.destinationAccountId =
          AccountIdLiteral.extractIh58Address(
              Objects.requireNonNull(destinationAccountId, "destinationAccountId"));
      return this;
    }

    public TransferNftInstruction build() {
      if (sourceAccountId == null || sourceAccountId.isBlank()) {
        throw new IllegalStateException("sourceAccountId must be set");
      }
      if (nftId == null || nftId.isBlank()) {
        throw new IllegalStateException("nftId must be set");
      }
      if (destinationAccountId == null || destinationAccountId.isBlank()) {
        throw new IllegalStateException("destinationAccountId must be set");
      }
      return new TransferNftInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("source", sourceAccountId);
      args.put("nft", nftId);
      args.put("destination", destinationAccountId);
      return args;
    }
  }
}

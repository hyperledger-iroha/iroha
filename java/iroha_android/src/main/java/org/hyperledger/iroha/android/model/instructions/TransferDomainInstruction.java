package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.address.AccountIdLiteral;

/** Typed builder for the `TransferDomain` instruction. */
public final class TransferDomainInstruction implements InstructionTemplate {

  private static final String ACTION = "TransferDomain";

  private final String sourceAccountId;
  private final String domainId;
  private final String destinationAccountId;
  private final Map<String, String> arguments;

  private TransferDomainInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private TransferDomainInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.sourceAccountId = builder.sourceAccountId;
    this.domainId = builder.domainId;
    this.destinationAccountId = builder.destinationAccountId;
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  public String sourceAccountId() {
    return sourceAccountId;
  }

  public String domainId() {
    return domainId;
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

  public static TransferDomainInstruction fromArguments(final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setSourceAccountId(require(arguments, "source"))
            .setDomainId(require(arguments, "domain"))
            .setDestinationAccountId(require(arguments, "destination"));
    return new TransferDomainInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof TransferDomainInstruction other)) {
      return false;
    }
    return Objects.equals(sourceAccountId, other.sourceAccountId)
        && Objects.equals(domainId, other.domainId)
        && Objects.equals(destinationAccountId, other.destinationAccountId);
  }

  @Override
  public int hashCode() {
    return Objects.hash(sourceAccountId, domainId, destinationAccountId);
  }

  public static final class Builder {
    private String sourceAccountId;
    private String domainId;
    private String destinationAccountId;

    private Builder() {}

    public Builder setSourceAccountId(final String sourceAccountId) {
      this.sourceAccountId =
          AccountIdLiteral.extractI105Address(
              Objects.requireNonNull(sourceAccountId, "sourceAccountId"));
      return this;
    }

    public Builder setDomainId(final String domainId) {
      this.domainId = Objects.requireNonNull(domainId, "domainId");
      return this;
    }

    public Builder setDestinationAccountId(final String destinationAccountId) {
      this.destinationAccountId =
          AccountIdLiteral.extractI105Address(
              Objects.requireNonNull(destinationAccountId, "destinationAccountId"));
      return this;
    }

    public TransferDomainInstruction build() {
      if (sourceAccountId == null || sourceAccountId.isBlank()) {
        throw new IllegalStateException("sourceAccountId must be set");
      }
      if (domainId == null || domainId.isBlank()) {
        throw new IllegalStateException("domainId must be set");
      }
      if (destinationAccountId == null || destinationAccountId.isBlank()) {
        throw new IllegalStateException("destinationAccountId must be set");
      }
      return new TransferDomainInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("source", sourceAccountId);
      args.put("domain", domainId);
      args.put("destination", destinationAccountId);
      return args;
    }
  }
}

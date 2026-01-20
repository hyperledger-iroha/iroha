package org.hyperledger.iroha.android.model.instructions;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import java.util.TreeMap;
import org.hyperledger.iroha.android.multisig.MultisigSpec;

/** Typed builder for {@code MultisigRegister} custom instructions. */
public final class MultisigRegisterInstruction implements InstructionTemplate {

  public static final String ACTION = "MultisigRegister";
  private static final String SIGNATORIES_PREFIX = "spec.signatories.";

  private final String accountId;
  private final MultisigSpec spec;
  private final Map<String, String> arguments;

  private MultisigRegisterInstruction(final Builder builder, final Map<String, String> argumentOrder) {
    this.accountId = builder.accountId;
    this.spec = builder.spec;
    this.arguments =
        Map.copyOf(Objects.requireNonNull(argumentOrder, "argument order must not be null"));
  }

  /** Controller account backing the multisig. */
  public String accountId() {
    return accountId;
  }

  /** Canonical multisig specification. */
  public MultisigSpec spec() {
    return spec;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.CUSTOM;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static MultisigRegisterInstruction fromArguments(final Map<String, String> arguments) {
    final MultisigSpec parsedSpec = parseSpec(arguments);
    final Builder builder =
        builder().setAccountId(require(arguments, "account")).setSpec(parsedSpec);
    return builder.buildWithArguments(new LinkedHashMap<>(arguments));
  }

  private static MultisigSpec parseSpec(final Map<String, String> arguments) {
    final MultisigSpec.Builder builder =
        MultisigSpec.builder()
            .setQuorum(requireInt(arguments, "spec.quorum"))
            .setTransactionTtlMs(requireLong(arguments, "spec.transaction_ttl_ms"));

    boolean sawSignatory = false;
    for (final Map.Entry<String, String> entry : arguments.entrySet()) {
      final String key = entry.getKey();
      if (key.startsWith(SIGNATORIES_PREFIX)) {
        final String accountId = key.substring(SIGNATORIES_PREFIX.length());
        builder.addSignatory(accountId, requireInt(arguments, key));
        sawSignatory = true;
      }
    }
    if (!sawSignatory) {
      throw new IllegalArgumentException(
          "At least one signatory is required via '" + SIGNATORIES_PREFIX + "<account_id>'");
    }

    try {
      return builder.build();
    } catch (final IllegalStateException ex) {
      throw new IllegalArgumentException("Invalid multisig spec: " + ex.getMessage(), ex);
    }
  }

  private static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  private static int requireInt(final Map<String, String> arguments, final String key) {
    final String value = require(arguments, key);
    try {
      return Integer.parseInt(value);
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException(
          "Instruction argument '" + key + "' must be an integer: " + value, ex);
    }
  }

  private static long requireLong(final Map<String, String> arguments, final String key) {
    final String value = require(arguments, key);
    try {
      return Long.parseLong(value);
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException(
          "Instruction argument '" + key + "' must be a long: " + value, ex);
    }
  }

  public static Builder builder() {
    return new Builder();
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof MultisigRegisterInstruction other)) {
      return false;
    }
    return Objects.equals(accountId, other.accountId) && Objects.equals(spec, other.spec);
  }

  @Override
  public int hashCode() {
    return Objects.hash(accountId, spec);
  }

  public static final class Builder {
    private String accountId;
    private MultisigSpec spec;

    private Builder() {}

    public Builder setAccountId(final String accountId) {
      this.accountId = normalizeAccount(accountId, "accountId");
      return this;
    }

    public Builder setSpec(final MultisigSpec spec) {
      this.spec = Objects.requireNonNull(spec, "spec");
      return this;
    }

    public MultisigRegisterInstruction build() {
      return buildWithArguments(canonicalArguments());
    }

    private MultisigRegisterInstruction buildWithArguments(final Map<String, String> argumentOrder) {
      if (accountId == null || accountId.isBlank()) {
        throw new IllegalStateException("accountId must be set");
      }
      if (spec == null) {
        throw new IllegalStateException("spec must be set");
      }
      // TODO: reject deterministic derived controller ids once the seed helper is exposed.
      return new MultisigRegisterInstruction(this, argumentOrder);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("account", accountId);
      args.put("spec.quorum", Integer.toString(spec.quorum()));
      args.put("spec.transaction_ttl_ms", Long.toString(spec.transactionTtlMs()));
      final Map<String, Integer> sortedSignatories = new TreeMap<>(spec.signatories());
      sortedSignatories.forEach(
          (account, weight) -> args.put(SIGNATORIES_PREFIX + account, Integer.toString(weight)));
      return args;
    }
  }

  private static String normalizeAccount(final String accountId, final String field) {
    if (accountId == null || accountId.isBlank()) {
      throw new IllegalArgumentException(field + " must not be blank");
    }
    return accountId.trim();
  }

}

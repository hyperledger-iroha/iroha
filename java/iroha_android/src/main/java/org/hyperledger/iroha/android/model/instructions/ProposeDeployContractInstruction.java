package org.hyperledger.iroha.android.model.instructions;

import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/**
 * Typed builder for {@code ProposeDeployContract} instructions.
 *
 * <p>Captures the governance namespace, contract identifiers, deterministic code/ABI hashes, and
 * optional enactment window + voting mode overrides. The builder emits canonical Norito arguments
 * so that Kotlin/Java clients match the Rust data model expectations.
 */
public final class ProposeDeployContractInstruction implements InstructionTemplate {

  private static final String ACTION = "ProposeDeployContract";

  private final String namespace;
  private final String contractId;
  private final String codeHashHex;
  private final String abiHashHex;
  private final String abiVersion;
  private final GovernanceInstructionUtils.AtWindow window;
  private final GovernanceInstructionUtils.VotingMode votingMode;
  private final Map<String, String> arguments;

  private ProposeDeployContractInstruction(final Builder builder) {
    this(builder, builder.canonicalArguments());
  }

  private ProposeDeployContractInstruction(
      final Builder builder, final Map<String, String> argumentOrder) {
    this.namespace = builder.namespace;
    this.contractId = builder.contractId;
    this.codeHashHex = builder.codeHashHex;
    this.abiHashHex = builder.abiHashHex;
    this.abiVersion = builder.abiVersion;
    this.window = builder.window;
    this.votingMode = builder.votingMode;
    this.arguments = Map.copyOf(argumentOrder);
  }

  public String namespace() {
    return namespace;
  }

  public String contractId() {
    return contractId;
  }

  public String codeHashHex() {
    return codeHashHex;
  }

  public String abiHashHex() {
    return abiHashHex;
  }

  public String abiVersion() {
    return abiVersion;
  }

  public GovernanceInstructionUtils.AtWindow window() {
    return window;
  }

  public GovernanceInstructionUtils.VotingMode votingMode() {
    return votingMode;
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

  public static ProposeDeployContractInstruction fromArguments(
      final Map<String, String> arguments) {
    final Builder builder =
        builder()
            .setNamespace(require(arguments, "namespace"))
            .setContractId(require(arguments, "contract_id"))
            .setCodeHashHex(require(arguments, "code_hash_hex"))
            .setAbiHashHex(require(arguments, "abi_hash_hex"))
            .setAbiVersion(require(arguments, "abi_version"));
    if (arguments.containsKey("mode")) {
      builder.setVotingMode(
          GovernanceInstructionUtils.VotingMode.parse(require(arguments, "mode")));
    }
    if (arguments.containsKey("window.lower") || arguments.containsKey("window.upper")) {
      builder.setWindow(
          GovernanceInstructionUtils.parseAtWindow(arguments, "window", "window override"));
    }
    return new ProposeDeployContractInstruction(builder, new LinkedHashMap<>(arguments));
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
    if (!(obj instanceof ProposeDeployContractInstruction other)) {
      return false;
    }
    return Objects.equals(namespace, other.namespace)
        && Objects.equals(contractId, other.contractId)
        && Objects.equals(codeHashHex, other.codeHashHex)
        && Objects.equals(abiHashHex, other.abiHashHex)
        && Objects.equals(abiVersion, other.abiVersion)
        && Objects.equals(window, other.window)
        && votingMode == other.votingMode;
  }

  @Override
  public int hashCode() {
    return Objects.hash(
        namespace, contractId, codeHashHex, abiHashHex, abiVersion, window, votingMode);
  }

  public static final class Builder {
    private String namespace;
    private String contractId;
    private String codeHashHex;
    private String abiHashHex;
    private String abiVersion;
    private GovernanceInstructionUtils.AtWindow window;
    private GovernanceInstructionUtils.VotingMode votingMode;

    private Builder() {}

    public Builder setNamespace(final String namespace) {
      if (namespace == null || namespace.isBlank()) {
        throw new IllegalArgumentException("namespace must not be blank");
      }
      this.namespace = namespace;
      return this;
    }

    public Builder setContractId(final String contractId) {
      if (contractId == null || contractId.isBlank()) {
        throw new IllegalArgumentException("contractId must not be blank");
      }
      this.contractId = contractId;
      return this;
    }

    public Builder setCodeHashHex(final String codeHashHex) {
      this.codeHashHex =
          GovernanceInstructionUtils.requireHex(codeHashHex, "codeHashHex", 32);
      return this;
    }

    public Builder setAbiHashHex(final String abiHashHex) {
      this.abiHashHex = GovernanceInstructionUtils.requireHex(abiHashHex, "abiHashHex", 32);
      return this;
    }

    public Builder setAbiVersion(final String abiVersion) {
      if (abiVersion == null || abiVersion.isBlank()) {
        throw new IllegalArgumentException("abiVersion must not be blank");
      }
      this.abiVersion = abiVersion;
      return this;
    }

    public Builder setWindow(final GovernanceInstructionUtils.AtWindow window) {
      this.window = Objects.requireNonNull(window, "window");
      return this;
    }

    public Builder setVotingMode(final GovernanceInstructionUtils.VotingMode votingMode) {
      this.votingMode = Objects.requireNonNull(votingMode, "votingMode");
      return this;
    }

    public ProposeDeployContractInstruction build() {
      if (namespace == null) {
        throw new IllegalStateException("namespace must be provided");
      }
      if (contractId == null) {
        throw new IllegalStateException("contractId must be provided");
      }
      if (codeHashHex == null) {
        throw new IllegalStateException("codeHashHex must be provided");
      }
      if (abiHashHex == null) {
        throw new IllegalStateException("abiHashHex must be provided");
      }
      if (abiVersion == null) {
        throw new IllegalStateException("abiVersion must be provided");
      }
      return new ProposeDeployContractInstruction(this);
    }

    private Map<String, String> canonicalArguments() {
      final Map<String, String> args = new LinkedHashMap<>();
      args.put("action", ACTION);
      args.put("namespace", namespace);
      args.put("contract_id", contractId);
      args.put("code_hash_hex", codeHashHex);
      args.put("abi_hash_hex", abiHashHex);
      args.put("abi_version", abiVersion);
      if (window != null) {
        GovernanceInstructionUtils.appendAtWindow(args, window, "window");
      }
      if (votingMode != null) {
        args.put("mode", votingMode.wireValue());
      }
      return args;
    }
  }
}

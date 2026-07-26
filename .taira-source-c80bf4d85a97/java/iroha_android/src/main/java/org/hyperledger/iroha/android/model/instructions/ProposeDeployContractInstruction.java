package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;

/**
 * Typed builder for {@code ProposeDeployContract} instructions.
 *
 * <p>Captures exactly one target selector ({@code contract_address} or {@code contract_alias}),
 * deterministic code/ABI hashes, and optional enactment window + voting mode overrides. The
 * builder emits canonical Norito arguments so that Android clients match the Rust data model
 * expectations.
 */
public final class ProposeDeployContractInstruction implements InstructionTemplate {

  private static final String ACTION = "ProposeDeployContract";

  private final String contractAddress;
  private final String contractAlias;
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
    this.contractAddress = builder.contractAddress;
    this.contractAlias = builder.contractAlias;
    this.codeHashHex = builder.codeHashHex;
    this.abiHashHex = builder.abiHashHex;
    this.abiVersion = builder.abiVersion;
    this.window = builder.window;
    this.votingMode = builder.votingMode;
    this.arguments =
        Collections.unmodifiableMap(new LinkedHashMap<>(Objects.requireNonNull(argumentOrder)));
  }

  public String contractAddress() {
    return contractAddress;
  }

  public String contractAlias() {
    return contractAlias;
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
    final String contractAddress = blankToNull(arguments.get("contract_address"));
    final String contractAlias = blankToNull(arguments.get("contract_alias"));
    if ((contractAddress == null) == (contractAlias == null)) {
      throw new IllegalArgumentException(
          "Instruction arguments must include exactly one of contract_address or contract_alias");
    }

    final Builder builder =
        builder()
            .setCodeHashHex(require(arguments, "code_hash_hex"))
            .setAbiHashHex(require(arguments, "abi_hash_hex"))
            .setAbiVersion(require(arguments, "abi_version"));
    if (contractAddress != null) {
      builder.setContractAddress(contractAddress);
    } else {
      builder.setContractAlias(contractAlias);
    }
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
    final String value = blankToNull(arguments.get(key));
    if (value == null) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  private static String blankToNull(final String value) {
    if (value == null) {
      return null;
    }
    final String trimmed = value.trim();
    return trimmed.isEmpty() ? null : trimmed;
  }

  @Override
  public boolean equals(final Object obj) {
    if (this == obj) {
      return true;
    }
    if (!(obj instanceof ProposeDeployContractInstruction other)) {
      return false;
    }
    return Objects.equals(contractAddress, other.contractAddress)
        && Objects.equals(contractAlias, other.contractAlias)
        && Objects.equals(codeHashHex, other.codeHashHex)
        && Objects.equals(abiHashHex, other.abiHashHex)
        && Objects.equals(abiVersion, other.abiVersion)
        && Objects.equals(window, other.window)
        && votingMode == other.votingMode;
  }

  @Override
  public int hashCode() {
    return Objects.hash(
        contractAddress, contractAlias, codeHashHex, abiHashHex, abiVersion, window, votingMode);
  }

  public static final class Builder {
    private String contractAddress;
    private String contractAlias;
    private String codeHashHex;
    private String abiHashHex;
    private String abiVersion;
    private GovernanceInstructionUtils.AtWindow window;
    private GovernanceInstructionUtils.VotingMode votingMode;

    private Builder() {}

    public Builder setContractAddress(final String contractAddress) {
      this.contractAddress = requireNonBlank(contractAddress, "contractAddress");
      return this;
    }

    public Builder setContractAlias(final String contractAlias) {
      this.contractAlias = requireNonBlank(contractAlias, "contractAlias");
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
      this.abiVersion = requireNonBlank(abiVersion, "abiVersion");
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
      final boolean hasContractAddress = contractAddress != null;
      final boolean hasContractAlias = contractAlias != null;
      if (hasContractAddress == hasContractAlias) {
        throw new IllegalStateException(
            "Exactly one of contractAddress or contractAlias must be provided");
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
      if (contractAddress != null) {
        args.put("contract_address", contractAddress);
      } else {
        args.put("contract_alias", contractAlias);
      }
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

    private static String requireNonBlank(final String value, final String field) {
      if (value == null || value.isBlank()) {
        throw new IllegalArgumentException(field + " must not be blank");
      }
      return value;
    }
  }
}

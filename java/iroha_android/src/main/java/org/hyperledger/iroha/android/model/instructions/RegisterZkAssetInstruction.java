package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

/** Typed representation of {@code zk::RegisterZkAsset}. */
public final class RegisterZkAssetInstruction implements InstructionTemplate {
  private final String asset;
  private final String unshieldVerifyingKey;
  private final String shieldVerifyingKey;
  private final Map<String, String> arguments;

  private RegisterZkAssetInstruction(final Builder builder) {
    this.asset = builder.asset;
    this.unshieldVerifyingKey = builder.unshieldVerifyingKey;
    this.shieldVerifyingKey = builder.shieldVerifyingKey;
    final LinkedHashMap<String, String> args = new LinkedHashMap<>();
    args.put("action", "RegisterZkAsset");
    args.put("asset", asset);
    args.put("vk_unshield", unshieldVerifyingKey == null ? "" : unshieldVerifyingKey);
    args.put("vk_shield", shieldVerifyingKey == null ? "" : shieldVerifyingKey);
    this.arguments = Collections.unmodifiableMap(args);
  }

  public String asset() {
    return asset;
  }

  public String unshieldVerifyingKey() {
    return unshieldVerifyingKey;
  }

  public String shieldVerifyingKey() {
    return shieldVerifyingKey;
  }

  @Override
  public InstructionKind kind() {
    return InstructionKind.REGISTER;
  }

  @Override
  public Map<String, String> toArguments() {
    return arguments;
  }

  public static Builder builder() {
    return new Builder();
  }

  public static RegisterZkAssetInstruction fromArguments(final Map<String, String> arguments) {
    for (final String key : arguments.keySet()) {
      if (!"action".equals(key)
          && !"asset".equals(key)
          && !"vk_unshield".equals(key)
          && !"vk_shield".equals(key)) {
        throw new IllegalArgumentException("Unknown instruction argument: " + key);
      }
    }
    final Builder builder = builder().setAsset(requireArgument(arguments, "asset"));
    final String unshield = optionalArgument(arguments, "vk_unshield");
    if (unshield != null) {
      builder.setUnshieldVerifyingKey(unshield);
    }
    final String shield = optionalArgument(arguments, "vk_shield");
    if (shield != null) {
      builder.setShieldVerifyingKey(shield);
    }
    return builder.build();
  }

  private static String requireArgument(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.trim().isEmpty()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  private static String optionalArgument(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    return (value == null || value.trim().isEmpty()) ? null : value;
  }

  public static final class Builder {
    private String asset;
    private String unshieldVerifyingKey;
    private String shieldVerifyingKey;

    private Builder() {}

    public Builder setAsset(final String asset) {
      this.asset = ZkInstructionUtils.requireText(asset, "asset");
      return this;
    }

    public Builder setUnshieldVerifyingKey(final String verifyingKey) {
      this.unshieldVerifyingKey =
          ZkInstructionUtils.optionalVerifyingKeyId(verifyingKey, "unshieldVerifyingKey");
      return this;
    }

    public Builder setShieldVerifyingKey(final String verifyingKey) {
      this.shieldVerifyingKey =
          ZkInstructionUtils.optionalVerifyingKeyId(verifyingKey, "shieldVerifyingKey");
      return this;
    }

    public RegisterZkAssetInstruction build() {
      if (asset == null) {
        throw new IllegalStateException("asset must be provided");
      }
      if (shieldVerifyingKey != null && unshieldVerifyingKey == null) {
        throw new IllegalStateException(
            "shieldVerifyingKey requires unshieldVerifyingKey so shielded funds remain redeemable");
      }
      return new RegisterZkAssetInstruction(this);
    }
  }
}

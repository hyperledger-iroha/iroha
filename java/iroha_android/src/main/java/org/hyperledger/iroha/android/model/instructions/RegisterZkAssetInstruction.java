package org.hyperledger.iroha.android.model.instructions;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;

/** Typed representation of {@code zk::RegisterZkAsset}. */
public final class RegisterZkAssetInstruction implements InstructionTemplate {
  private final String asset;
  private final ZkAssetMode mode;
  private final boolean allowShield;
  private final boolean allowUnshield;
  private final String transferVerifyingKey;
  private final String unshieldVerifyingKey;
  private final String shieldVerifyingKey;
  private final Map<String, String> arguments;

  private RegisterZkAssetInstruction(final Builder builder) {
    this.asset = builder.asset;
    this.mode = builder.mode;
    this.allowShield = builder.allowShield;
    this.allowUnshield = builder.allowUnshield;
    this.transferVerifyingKey = builder.transferVerifyingKey;
    this.unshieldVerifyingKey = builder.unshieldVerifyingKey;
    this.shieldVerifyingKey = builder.shieldVerifyingKey;
    final LinkedHashMap<String, String> args = new LinkedHashMap<>();
    args.put("action", "RegisterZkAsset");
    args.put("asset", asset);
    args.put("mode", mode.wireName());
    args.put("allow_shield", Boolean.toString(allowShield));
    args.put("allow_unshield", Boolean.toString(allowUnshield));
    args.put("vk_transfer", transferVerifyingKey == null ? "" : transferVerifyingKey);
    args.put("vk_unshield", unshieldVerifyingKey == null ? "" : unshieldVerifyingKey);
    args.put("vk_shield", shieldVerifyingKey == null ? "" : shieldVerifyingKey);
    this.arguments = Collections.unmodifiableMap(args);
  }

  public String asset() {
    return asset;
  }

  public ZkAssetMode mode() {
    return mode;
  }

  public boolean allowShield() {
    return allowShield;
  }

  public boolean allowUnshield() {
    return allowUnshield;
  }

  public String transferVerifyingKey() {
    return transferVerifyingKey;
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

  public static final class Builder {
    private String asset;
    private ZkAssetMode mode = ZkAssetMode.ZK_NATIVE;
    private boolean allowShield = true;
    private boolean allowUnshield = true;
    private String transferVerifyingKey;
    private String unshieldVerifyingKey;
    private String shieldVerifyingKey;

    private Builder() {}

    public Builder setAsset(final String asset) {
      this.asset = ZkInstructionUtils.requireText(asset, "asset");
      return this;
    }

    public Builder setMode(final ZkAssetMode mode) {
      if (mode == null) {
        throw new IllegalArgumentException("mode must be provided");
      }
      this.mode = mode;
      return this;
    }

    public Builder setAllowShield(final boolean allowShield) {
      this.allowShield = allowShield;
      return this;
    }

    public Builder setAllowUnshield(final boolean allowUnshield) {
      this.allowUnshield = allowUnshield;
      return this;
    }

    public Builder setTransferVerifyingKey(final String verifyingKey) {
      this.transferVerifyingKey =
          ZkInstructionUtils.optionalVerifyingKeyId(verifyingKey, "transferVerifyingKey");
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
      return new RegisterZkAssetInstruction(this);
    }
  }
}

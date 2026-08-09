package org.hyperledger.iroha.android.model.instructions;

/** Shielded asset registration mode accepted by {@code zk::RegisterZkAsset}. */
public enum ZkAssetMode {
  HYBRID(0, "Hybrid");

  private final int bridgeCode;
  private final String wireName;

  ZkAssetMode(final int bridgeCode, final String wireName) {
    this.bridgeCode = bridgeCode;
    this.wireName = wireName;
  }

  public int bridgeCode() {
    return bridgeCode;
  }

  public String wireName() {
    return wireName;
  }

  public static ZkAssetMode fromWireName(final String value) {
    final String text = ZkInstructionUtils.requireText(value, "mode");
    for (final ZkAssetMode mode : values()) {
      if (mode.wireName.equals(text)) {
        return mode;
      }
    }
    throw new IllegalArgumentException("mode must be Hybrid");
  }
}

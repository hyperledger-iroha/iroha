package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;

/** Revision-CAS instruction that enables or disables native alias auto-renew. */
public final class ConfigureAliasAutoRenew extends AliasJsonValue {
  /** Stable instruction registry identifier. */
  public static final String WIRE_ID = "iroha.alias.auto_renew.configure";

  private final AliasSetupModels.AliasTargetV1 target;
  private final long expectedRevision;
  private final AliasAutoRenewConfigV1 config;

  /** Constructs an auto-renew configuration compare-and-set. */
  public ConfigureAliasAutoRenew(
      final AliasSetupModels.AliasTargetV1 target,
      final long expectedRevision,
      final AliasAutoRenewConfigV1 config) {
    if (target == null) throw new IllegalArgumentException("target must not be null");
    this.target = target;
    this.expectedRevision =
        AliasNameSupport.requireNonNegative(expectedRevision, "expectedRevision");
    this.config = config;
  }

  /** Returns the exact resolved target. */
  public AliasSetupModels.AliasTargetV1 target() { return target; }

  /** Returns the revision that must still be current. */
  public long expectedRevision() { return expectedRevision; }

  /** Returns the new configuration, or null when disabling. */
  public AliasAutoRenewConfigV1 config() { return config; }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("target", target.toJsonMap());
    map.put("expected_revision", expectedRevision);
    map.put("config", config == null ? null : config.toJsonMap());
    return map;
  }
}

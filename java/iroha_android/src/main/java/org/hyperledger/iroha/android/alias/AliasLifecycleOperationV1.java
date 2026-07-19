package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;

/** Exact lifecycle operation committed by a lifecycle transaction plan. */
public abstract class AliasLifecycleOperationV1 extends AliasJsonValue {
  /** Returns the stable JSON variant name. */
  public abstract String kind();

  /** Returns the exact resolved target. */
  public abstract AliasSetupModels.AliasTargetV1 target();

  /** Absolute-expiry lease renewal. */
  public static final class RenewLease extends AliasLifecycleOperationV1 {
    private final RenewAliasLease renewal;

    /** Wraps one exact renewal. */
    public RenewLease(final RenewAliasLease renewal) {
      if (renewal == null) throw new IllegalArgumentException("renewal must not be null");
      this.renewal = renewal;
    }

    /** Returns the exact renewal. */
    public RenewAliasLease renewal() {
      return renewal;
    }

    @Override
    public String kind() {
      return "renew_lease";
    }

    @Override
    public AliasSetupModels.AliasTargetV1 target() {
      return renewal.target();
    }

    @Override
    public Map<String, Object> toJsonMap() {
      return variant(kind(), renewal.toJsonMap());
    }
  }

  /** Enable, replace, or disable deterministic native auto-renew. */
  public static final class ConfigureAutoRenew extends AliasLifecycleOperationV1 {
    private final org.hyperledger.iroha.android.alias.ConfigureAliasAutoRenew configuration;

    /** Wraps one exact configuration CAS. */
    public ConfigureAutoRenew(
        final org.hyperledger.iroha.android.alias.ConfigureAliasAutoRenew configuration) {
      if (configuration == null) {
        throw new IllegalArgumentException("configuration must not be null");
      }
      this.configuration = configuration;
    }

    /** Returns the exact configuration CAS. */
    public org.hyperledger.iroha.android.alias.ConfigureAliasAutoRenew configuration() {
      return configuration;
    }

    @Override
    public String kind() {
      return "configure_auto_renew";
    }

    @Override
    public AliasSetupModels.AliasTargetV1 target() {
      return configuration.target();
    }

    @Override
    public Map<String, Object> toJsonMap() {
      return variant(kind(), configuration.toJsonMap());
    }
  }

  private static Map<String, Object> variant(
      final String kind, final Map<String, Object> operation) {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("kind", kind);
    map.put("operation", operation);
    return map;
  }
}

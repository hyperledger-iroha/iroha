package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;

/** Canonical signed request body for one native auto-renew configuration plan. */
public final class AliasAutoRenewPlanRequestV1 extends AliasLifecyclePlanRequestV1 {
  /** Current request layout. */
  public static final int VERSION = 1;

  private final int schemaVersion;
  private final ConfigureAliasAutoRenew configuration;
  private final AliasLifecycleOperationV1 operation;

  /** Constructs a version-one request. */
  public AliasAutoRenewPlanRequestV1(final ConfigureAliasAutoRenew configuration) {
    this(VERSION, configuration);
  }

  /** Constructs an explicitly versioned request. */
  public AliasAutoRenewPlanRequestV1(
      final int schemaVersion, final ConfigureAliasAutoRenew configuration) {
    if (schemaVersion != VERSION) {
      throw new IllegalArgumentException("schemaVersion must be " + VERSION);
    }
    if (configuration == null) {
      throw new IllegalArgumentException("configuration must not be null");
    }
    this.schemaVersion = schemaVersion;
    this.configuration = configuration;
    this.operation = new AliasLifecycleOperationV1.ConfigureAutoRenew(configuration);
  }

  /** Returns the request layout version. */
  public int schemaVersion() {
    return schemaVersion;
  }

  /** Returns the exact configuration CAS. */
  public ConfigureAliasAutoRenew configuration() {
    return configuration;
  }

  @Override
  public AliasLifecycleOperationV1 operation() {
    return operation;
  }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("schema_version", schemaVersion);
    map.put("configuration", configuration.toJsonMap());
    return map;
  }
}

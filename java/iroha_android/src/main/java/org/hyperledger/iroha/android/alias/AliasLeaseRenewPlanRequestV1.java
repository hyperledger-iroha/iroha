package org.hyperledger.iroha.android.alias;

import java.util.LinkedHashMap;
import java.util.Map;

/** Canonical signed request body for one lease-renewal plan. */
public final class AliasLeaseRenewPlanRequestV1 extends AliasLifecyclePlanRequestV1 {
  /** Current request layout. */
  public static final int VERSION = 1;

  private final int schemaVersion;
  private final RenewAliasLease renewal;
  private final AliasLifecycleOperationV1 operation;

  /** Constructs a version-one request. */
  public AliasLeaseRenewPlanRequestV1(final RenewAliasLease renewal) {
    this(VERSION, renewal);
  }

  /** Constructs an explicitly versioned request. */
  public AliasLeaseRenewPlanRequestV1(
      final int schemaVersion, final RenewAliasLease renewal) {
    if (schemaVersion != VERSION) {
      throw new IllegalArgumentException("schemaVersion must be " + VERSION);
    }
    if (renewal == null) throw new IllegalArgumentException("renewal must not be null");
    this.schemaVersion = schemaVersion;
    this.renewal = renewal;
    this.operation = new AliasLifecycleOperationV1.RenewLease(renewal);
  }

  /** Returns the request layout version. */
  public int schemaVersion() {
    return schemaVersion;
  }

  /** Returns the exact renewal. */
  public RenewAliasLease renewal() {
    return renewal;
  }

  @Override
  public AliasLifecycleOperationV1 operation() {
    return operation;
  }

  @Override
  public Map<String, Object> toJsonMap() {
    final Map<String, Object> map = new LinkedHashMap<>();
    map.put("schema_version", schemaVersion);
    map.put("renewal", renewal.toJsonMap());
    return map;
  }
}

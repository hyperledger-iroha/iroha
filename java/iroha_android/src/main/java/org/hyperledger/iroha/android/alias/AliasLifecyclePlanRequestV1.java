package org.hyperledger.iroha.android.alias;

/** Versioned canonical request accepted by an alias lifecycle planner. */
public abstract class AliasLifecyclePlanRequestV1 extends AliasJsonValue {
  /** Returns the exact lifecycle operation that the planner must preserve. */
  public abstract AliasLifecycleOperationV1 operation();
}

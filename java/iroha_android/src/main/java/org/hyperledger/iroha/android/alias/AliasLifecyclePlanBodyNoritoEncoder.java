package org.hyperledger.iroha.android.alias;

/** Encodes a lifecycle plan body to the exact canonical Norito bytes committed by its hash. */
@FunctionalInterface
public interface AliasLifecyclePlanBodyNoritoEncoder {
  /** Returns canonical Norito bytes for the supplied body. */
  byte[] encode(AliasLifecycleTransactionPlanBodyV1 body);
}

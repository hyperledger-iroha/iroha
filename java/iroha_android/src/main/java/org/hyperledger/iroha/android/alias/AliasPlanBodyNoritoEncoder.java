package org.hyperledger.iroha.android.alias;

/** Encodes a typed plan body to the exact canonical Norito bytes committed by its hash. */
@FunctionalInterface
public interface AliasPlanBodyNoritoEncoder {
  /** Returns canonical Norito bytes for the supplied body. */
  byte[] encode(AliasSetupModels.AliasTransactionPlanBodyV1 body);
}

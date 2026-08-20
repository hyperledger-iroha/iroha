package org.hyperledger.iroha.android.model;

/** Signature-bound admission protocol required before a transaction may execute. */
public enum TransactionAdmissionIntent {
  /** Ordinary queue admission without a globally certified QueuePlan owner. */
  ORDINARY,

  /** Require an exact quorum-certified QueuePlan registry owner before execution. */
  QUEUE_PLAN_SYNCED
}

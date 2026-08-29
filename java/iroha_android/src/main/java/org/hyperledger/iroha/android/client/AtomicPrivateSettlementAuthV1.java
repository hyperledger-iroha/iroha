package org.hyperledger.iroha.android.client;

/** Authentication class required by an atomic-private-settlement Torii operation. */
public enum AtomicPrivateSettlementAuthV1 {
  /** Exact sponsor account request signature. */
  SPONSOR,
  /** Exact validator or auditor identity request signature. */
  ROLE_IDENTITY,
  /** Public redacted query with no identity material. */
  PUBLIC
}

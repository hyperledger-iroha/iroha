package org.hyperledger.iroha.android.alias;

/** Canonical terminal or nonterminal state for one exact prepared hash. */
public enum PreparedTransactionOutcomeV1 {
  APPLIED("Applied"),
  PENDING("Pending"),
  REJECTED("Rejected");

  private final String wireValue;
  PreparedTransactionOutcomeV1(final String wireValue) { this.wireValue = wireValue; }
  public String wireValue() { return wireValue; }
}

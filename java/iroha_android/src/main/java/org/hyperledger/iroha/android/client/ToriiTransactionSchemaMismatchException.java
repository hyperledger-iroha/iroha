package org.hyperledger.iroha.android.client;

/** Raised when Torii advertises a different signed-transaction schema. */
public final class ToriiTransactionSchemaMismatchException
    extends ToriiTransactionCompatibilityException {
  private final String expected;
  private final String actual;

  /** Creates a mismatch for the expected and advertised schema hashes. */
  public ToriiTransactionSchemaMismatchException(
      final String expected, final String actual) {
    super(
        "Torii node signed_transaction_schema_hash_hex "
            + (actual == null ? "<missing-or-invalid>" : actual)
            + " does not match client schema "
            + expected);
    this.expected = expected;
    this.actual = actual;
  }

  /** Returns the schema hash encoded by this SDK. */
  public String expected() {
    return expected;
  }

  /** Returns the schema hash advertised by Torii, or {@code null} when absent or invalid. */
  public String actual() {
    return actual;
  }
}

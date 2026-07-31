package org.hyperledger.iroha.android.client;

/** Raised when Torii advertises a data-model version this SDK cannot encode. */
public final class ToriiDataModelMismatchException
    extends ToriiTransactionCompatibilityException {
  private final int expected;
  private final int actual;

  /** Creates a mismatch for the expected and advertised data-model versions. */
  public ToriiDataModelMismatchException(final int expected, final int actual) {
    super(
        "Torii node data_model_version "
            + actual
            + " does not match client version "
            + expected);
    this.expected = expected;
    this.actual = actual;
  }

  /** Returns the version encoded by this SDK. */
  public int expected() {
    return expected;
  }

  /** Returns the version advertised by Torii. */
  public int actual() {
    return actual;
  }
}

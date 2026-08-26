package org.hyperledger.iroha.android.client;

/** Closed first-release phase of private uploaded-model transaction submission. */
public enum SoracloudPrivateUploadedModelSubmissionPhase {
  /** Encrypted output exists, but its durability transaction has not been submitted. */
  AWAITING_OUTPUT_DURABILITY("awaiting_output_durability"),

  /** The preparation transaction has been submitted. */
  PREPARE_SUBMITTED("prepare_submitted"),

  /** The durable-output receipt transaction has been submitted. */
  RECEIPT_SUBMITTED("receipt_submitted"),

  /** The execution receipt has committed with ledger-assigned coordinates. */
  COMMITTED("committed");

  private final String wireValue;

  SoracloudPrivateUploadedModelSubmissionPhase(final String wireValue) {
    this.wireValue = wireValue;
  }

  /** Return the exact Norito JSON spelling. */
  public String wireValue() {
    return wireValue;
  }

  /** Parse the exact first-release Norito JSON spelling. */
  public static SoracloudPrivateUploadedModelSubmissionPhase fromWireValue(
      final String value) {
    final String canonical =
        SoracloudPrivateModelValidation.requireCanonicalString(value, "submissionPhase");
    switch (canonical) {
      case "awaiting_output_durability":
        return AWAITING_OUTPUT_DURABILITY;
      case "prepare_submitted":
        return PREPARE_SUBMITTED;
      case "receipt_submitted":
        return RECEIPT_SUBMITTED;
      case "committed":
        return COMMITTED;
      default:
        throw new IllegalArgumentException(
            "submissionPhase must equal awaiting_output_durability, prepare_submitted, "
                + "receipt_submitted, or committed");
    }
  }

  boolean requiresTransactionHash() {
    return this == PREPARE_SUBMITTED || this == RECEIPT_SUBMITTED;
  }

  boolean requiresAssignedReceipt() {
    return this == COMMITTED;
  }

  @Override
  public String toString() {
    return wireValue;
  }
}

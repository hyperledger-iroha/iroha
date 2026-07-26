package org.hyperledger.iroha.android.alias;

/** Sponsored onboarding apply status. */
public enum AccountOnboardingStatusV1 {
  QUEUED("Queued"),
  REPAIRED("Repaired"),
  UNCHANGED("Unchanged");

  private final String wireValue;

  AccountOnboardingStatusV1(final String wireValue) {
    this.wireValue = wireValue;
  }

  /** Returns the stable response value. */
  public String wireValue() {
    return wireValue;
  }
}

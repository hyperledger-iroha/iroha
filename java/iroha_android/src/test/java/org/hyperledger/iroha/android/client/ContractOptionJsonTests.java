package org.hyperledger.iroha.android.client;

/** Focused contract-call boundary coverage for tagged Option values containing Unit. */
public final class ContractOptionJsonTests {
  private ContractOptionJsonTests() {}

  /** Runs the same production HTTP boundary regression as the complete transport harness. */
  public static void main(final String[] args) throws Exception {
    HttpClientTransportTests.contractCallPreservesUnitAndNestedOptionTags();
    System.out.println("[IrohaAndroid] ContractOptionJsonTests passed.");
  }
}

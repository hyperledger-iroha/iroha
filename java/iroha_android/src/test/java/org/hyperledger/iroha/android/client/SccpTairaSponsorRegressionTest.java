package org.hyperledger.iroha.android.client;

import org.junit.Test;

/** Focused regression gate for exact Taira fee-sponsor identity across controller-only wire data. */
public final class SccpTairaSponsorRegressionTest {
  @Test
  public void exactTairaSponsorSurvivesControllerOnlyWireIdentity() throws Exception {
    SccpClientExactTests.signedSubmitPreservesExactTairaSponsorAcrossControllerOnlyWireIdentity();
  }
}

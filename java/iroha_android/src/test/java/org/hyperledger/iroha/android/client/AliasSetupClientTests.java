package org.hyperledger.iroha.android.client;

import org.junit.Test;

/** Focused Java mirror tests for safe alias planning over Torii. */
public final class AliasSetupClientTests {
  /** Verifies the planner route, canonical headers, typed response, and secret-free request. */
  @Test
  public void plansWithoutCallingAnAliasMutationRoute() throws Exception {
    HttpClientTransportTests.aliasSetupPlanningIsCanonicalSignedReadOnlyAndParsesTypedPlan();
  }
}

package org.hyperledger.iroha.android.multisig;

public final class MultisigSpecTests {
  private static final String ALICE_ID =
      "6cmzPVPX5jDQFNfiz6KgmVfm1fhoAqjPhoPFn4nx9mBWaFMyUCwq4cw";
  private static final String BOB_ID =
      "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn";

  private MultisigSpecTests() {}

  public static void main(final String[] args) {
    testBuilderProducesJson();
    testPreviewClampsToPolicyCap();
    testEnforceRejectsAboveCap();
  }

  private static void testBuilderProducesJson() {
    final MultisigSpec spec =
        MultisigSpec.builder()
            .setQuorum(3)
            .setTransactionTtlMs(60_000)
            .addSignatory(ALICE_ID, 2)
            .addSignatory(BOB_ID, 1)
            .build();

    assert spec.quorum() == 3 : "quorum mismatch";
    assert spec.transactionTtlMs() == 60_000 : "ttl mismatch";
    final String json = spec.toJson(true);
    assert json.contains("\"transaction_ttl_ms\": 60000") : "json missing ttl";
    assert json.indexOf(ALICE_ID) < json.indexOf(BOB_ID) : "signatories not sorted";
  }

  private static void testPreviewClampsToPolicyCap() {
    final MultisigSpec spec =
        MultisigSpec.builder()
            .setQuorum(1)
            .setTransactionTtlMs(10_000)
            .addSignatory(ALICE_ID, 1)
            .build();

    final MultisigProposalTtlPreview preview = spec.previewProposalExpiry(20_000L, 0L);
    assert preview.wasCapped() : "expected cap";
    assert preview.policyCapMs() == 10_000 : "policy cap mismatch";
    assert preview.effectiveTtlMs() == 10_000 : "effective ttl mismatch";
    assert preview.expiresAtMs() == 10_000 : "expiry mismatch";
  }

  private static void testEnforceRejectsAboveCap() {
    final MultisigSpec spec =
        MultisigSpec.builder()
            .setQuorum(1)
            .setTransactionTtlMs(5_000)
            .addSignatory(ALICE_ID, 1)
            .build();

    boolean threw = false;
    try {
      spec.enforceProposalTtl(6_000L, 0L);
    } catch (IllegalArgumentException expected) {
      threw = expected.getMessage().contains("exceeds the policy cap");
    }
    assert threw : "expected enforcement to reject ttl above cap";
  }
}

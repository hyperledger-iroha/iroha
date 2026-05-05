package org.hyperledger.iroha.android.multisig;

import java.util.Arrays;
import org.hyperledger.iroha.android.address.AccountAddress;

public final class MultisigSpecTests {

  private MultisigSpecTests() {}

  public static void main(final String[] args) {
    testBuilderProducesJson();
    testPreviewClampsToPolicyCap();
    testEnforceRejectsAboveCap();
  }

  private static void testBuilderProducesJson() {
    final String signerA = sampleI105(0x11);
    final String signerB = sampleI105(0x12);
    final MultisigSpec spec =
        MultisigSpec.builder()
            .setQuorum(3)
            .setTransactionTtlMs(60_000)
            .addSignatory(signerA, 2)
            .addSignatory(signerB, 1)
            .build();

    assert spec.quorum() == 3 : "quorum mismatch";
    assert spec.transactionTtlMs() == 60_000 : "ttl mismatch";
    final String json = spec.toJson(true);
    assert json.contains("\"transaction_ttl_ms\": 60000") : "json missing ttl";
    assert json.indexOf(signerA) < json.indexOf(signerB) : "signatories not sorted";
  }

  private static void testPreviewClampsToPolicyCap() {
    final String signer = sampleI105(0x21);
    final MultisigSpec spec =
        MultisigSpec.builder()
            .setQuorum(1)
            .setTransactionTtlMs(10_000)
            .addSignatory(signer, 1)
            .build();

    final MultisigProposalTtlPreview preview = spec.previewProposalExpiry(20_000L, 0L);
    assert preview.wasCapped() : "expected cap";
    assert preview.policyCapMs() == 10_000 : "policy cap mismatch";
    assert preview.effectiveTtlMs() == 10_000 : "effective ttl mismatch";
    assert preview.expiresAtMs() == 10_000 : "expiry mismatch";
  }

  private static void testEnforceRejectsAboveCap() {
    final String signer = sampleI105(0x31);
    final MultisigSpec spec =
        MultisigSpec.builder()
            .setQuorum(1)
            .setTransactionTtlMs(5_000)
            .addSignatory(signer, 1)
            .build();

    boolean threw = false;
    try {
      spec.enforceProposalTtl(6_000L, 0L);
    } catch (IllegalArgumentException expected) {
      threw = expected.getMessage().contains("exceeds the policy cap");
    }
    assert threw : "expected enforcement to reject ttl above cap";
  }

  private static String sampleI105(final int fill) {
    try {
      final byte[] publicKey = new byte[32];
      Arrays.fill(publicKey, (byte) fill);
      return AccountAddress.fromAccount(publicKey, "ed25519")
          .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final Exception ex) {
      throw new IllegalStateException("failed to build canonical account fixture", ex);
    }
  }
}

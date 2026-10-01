package org.hyperledger.iroha.android.multisig;

import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Map;
import org.hyperledger.iroha.android.testing.TestEd25519Keys;
import org.hyperledger.iroha.sdk.address.AccountAddress;
import org.hyperledger.iroha.sdk.multisig.MultisigProposalTtlPreview;
import org.hyperledger.iroha.sdk.multisig.MultisigSpec;

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
    final Map<String, Integer> signatories = new LinkedHashMap<>();
    signatories.put(signerA, 2);
    signatories.put(signerB, 1);
    final MultisigSpec spec = new MultisigSpec(signatories, 3, 60_000);

    assert spec.quorum == 3 : "quorum mismatch";
    assert spec.transactionTtlMs == 60_000 : "ttl mismatch";
    final String json = spec.toJson(true);
    assert json.contains("\"transaction_ttl_ms\": 60000") : "json missing ttl";
    assert json.contains("\"" + signerA + "\": 2") : "first signatory weight missing";
    assert json.contains("\"" + signerB + "\": 1") : "second signatory weight missing";
    // JSON presentation sorts encoded keys, independently of fixture seed or insertion order.
    final String first = signerA.compareTo(signerB) < 0 ? signerA : signerB;
    final String second = signerA.compareTo(signerB) < 0 ? signerB : signerA;
    assert json.indexOf(first) < json.indexOf(second) : "signatories not sorted";
    final Map<String, Integer> reversed = new LinkedHashMap<>();
    reversed.put(signerB, 1);
    reversed.put(signerA, 2);
    assert json.equals(new MultisigSpec(reversed, 3, 60_000).toJson(true))
        : "JSON signatories must be independent of insertion order";
  }

  private static void testPreviewClampsToPolicyCap() {
    final String signer = sampleI105(0x21);
    final MultisigSpec spec =
        new MultisigSpec(Collections.singletonMap(signer, 1), 1, 10_000);

    final MultisigProposalTtlPreview preview = spec.previewProposalExpiry(20_000L, 0L);
    assert preview.wasCapped : "expected cap";
    assert preview.policyCapMs == 10_000 : "policy cap mismatch";
    assert preview.effectiveTtlMs == 10_000 : "effective ttl mismatch";
    assert preview.expiresAtMs == 10_000 : "expiry mismatch";
  }

  private static void testEnforceRejectsAboveCap() {
    final String signer = sampleI105(0x31);
    final MultisigSpec spec =
        new MultisigSpec(Collections.singletonMap(signer, 1), 1, 5_000);

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
      return AccountAddress.fromAccount(TestEd25519Keys.publicKey(fill), "ed25519")
          .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final Exception ex) {
      throw new IllegalStateException("failed to build canonical account fixture", ex);
    }
  }
}

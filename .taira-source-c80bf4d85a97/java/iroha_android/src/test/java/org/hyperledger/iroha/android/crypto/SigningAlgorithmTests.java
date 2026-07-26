package org.hyperledger.iroha.android.crypto;

import java.util.LinkedHashMap;
import java.util.Map;
import org.junit.Test;

public final class SigningAlgorithmTests {
  @Test
  public void bridgeCodesAndCanonicalWireNamesMatchRustAlgorithms() {
    final Map<Integer, String> expected = new LinkedHashMap<>();
    expected.put(0, "ed25519");
    expected.put(1, "secp256k1");
    expected.put(2, "bls_normal");
    expected.put(3, "bls_small");
    expected.put(4, "ml-dsa");
    expected.put(5, "gost3410-2012-256-paramset-a");
    expected.put(6, "gost3410-2012-256-paramset-b");
    expected.put(7, "gost3410-2012-256-paramset-c");
    expected.put(8, "gost3410-2012-512-paramset-a");
    expected.put(9, "gost3410-2012-512-paramset-b");
    expected.put(10, "sm2");

    final Map<Integer, String> actual = new LinkedHashMap<>();
    for (final SigningAlgorithm algorithm : SigningAlgorithm.values()) {
      actual.put(algorithm.bridgeCode(), algorithm.wireName());
      assert expected.get(algorithm.bridgeCode()).equals(
          SigningAlgorithm.fromBridgeCode(algorithm.bridgeCode()).wireName());
    }
    assert expected.equals(actual);
  }

  @Test
  public void aliasesNormalizeToCanonicalAlgorithms() {
    assert SigningAlgorithm.fromAlgorithmName("secp-256k1") == SigningAlgorithm.SECP256K1;
    assert SigningAlgorithm.fromAlgorithmName("bls-normal") == SigningAlgorithm.BLS_NORMAL;
    assert SigningAlgorithm.fromAlgorithmName("bls12-381-g2") == SigningAlgorithm.BLS_SMALL;
    assert SigningAlgorithm.fromAlgorithmName("ML_DSA-65") == SigningAlgorithm.ML_DSA;
    assert SigningAlgorithm.fromAlgorithmName("GOST3410-2012-512-PARAMSET-B")
        == SigningAlgorithm.GOST_2012_512_B;
    assert SigningAlgorithm.fromAlgorithmName("sm-2") == SigningAlgorithm.SM2;
  }

  @Test
  public void unsupportedAndUnicodeConfusableAliasesFailClosed() {
    final String[] blankAlgorithms = {null, "", "   "};
    for (final String algorithm : blankAlgorithms) {
      final IllegalArgumentException error =
          assertThrows(() -> SigningAlgorithm.fromAlgorithmName(algorithm));
      assert error.getMessage().contains("non-empty string") : error.getMessage();
    }

    final String[] paddedAlgorithms = {" ed25519", "ed25519 ", "\ted25519"};
    for (final String algorithm : paddedAlgorithms) {
      final IllegalArgumentException error =
          assertThrows(() -> SigningAlgorithm.fromAlgorithmName(algorithm));
      assert error.getMessage().contains("surrounding whitespace") : error.getMessage();
    }

    final String[] algorithms = {
      "unknown",
      "ed\t25519",
      "ed\u200B25519",
      "\u0435d25519",
      "ml\uFF0Ddsa",
      "gost3410-2012-512-paramset-\u0432"
    };
    for (final String algorithm : algorithms) {
      assertThrows(() -> SigningAlgorithm.fromAlgorithmName(algorithm));
    }
  }

  private static IllegalArgumentException assertThrows(final Runnable action) {
    try {
      action.run();
    } catch (final IllegalArgumentException expected) {
      return expected;
    }
    throw new AssertionError("expected IllegalArgumentException");
  }
}

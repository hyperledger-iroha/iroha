package org.hyperledger.iroha.android.address;

import java.util.Arrays;

/** Tests for canonical asset-id literal helpers. */
public final class AssetIdLiteralTests {
  private AssetIdLiteralTests() {}

  public static void main(final String[] args) {
    encodeFromPartsRejectsAliasDefinition();
    encodeFromPartsRejectsAliasAccount();
    encodeFromPartsProducesNoritoLiteralWhenNativeAvailable();
    System.out.println("[IrohaAndroid] AssetIdLiteral tests passed.");
  }

  private static void encodeFromPartsRejectsAliasDefinition() {
    final String accountId = sampleI105(0x51);
    boolean threw = false;
    try {
      AssetIdLiteral.encodeFromParts("usd#issuer@main", accountId);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage() != null && ex.getMessage().contains("aliases require online");
    }
    assert threw : "Asset definition aliases must be rejected for offline-only encoding";
  }

  private static void encodeFromPartsRejectsAliasAccount() {
    boolean threw = false;
    try {
      AssetIdLiteral.encodeFromParts("usd#wonderland", "alice@wonderland");
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage() != null && ex.getMessage().contains("must not include @domain");
    }
    assert threw : "Account aliases must be rejected for offline-only encoding";
  }

  private static void encodeFromPartsProducesNoritoLiteralWhenNativeAvailable() {
    if (!AssetIdLiteral.isNativeAvailable()) {
      System.out.println(
          "[IrohaAndroid] AssetIdLiteral success path skipped (native unavailable).");
      return;
    }
    final String accountId = sampleI105(0x52);
    final String literal =
        AssetIdLiteral.encodeFromParts("usd#wonderland", accountId);
    assert literal.startsWith("norito:") : "Encoded asset id must use norito prefix";
  }

  private static String sampleI105(final int fill) {
    try {
      final byte[] publicKey = new byte[32];
      Arrays.fill(publicKey, (byte) fill);
      return AccountAddress.fromAccount(publicKey, "ed25519")
          .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final Exception ex) {
      throw new AssertionError("failed to generate sample I105 account id", ex);
    }
  }
}

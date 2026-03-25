// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.address;

import org.hyperledger.iroha.android.testing.TestAccountIds;

public final class AssetIdEncoderTests {

  private static final String ACCOUNT_ID = TestAccountIds.ed25519Authority(0x11);

  private AssetIdEncoderTests() {}

  public static void main(final String[] args) {
    encodeAssetIdUsesCanonicalDefinitionAddress();
    encodeAssetIdFromDefinitionBuildsCanonicalLiteral();
    encodeScopedAssetIdFromDefinitionAppendsDataspaceSuffix();
    encodeAssetIdRejectsNonCanonicalAccountId();
    System.out.println("[IrohaAndroid] AssetIdEncoder tests passed.");
  }

  private static void encodeAssetIdUsesCanonicalDefinitionAddress() {
    final String encoded = AssetIdEncoder.encodeAssetId("rose", "wonderland", ACCOUNT_ID);
    final String expected = AssetDefinitionIdEncoder.encode("rose", "wonderland") + "#" + ACCOUNT_ID;

    assert expected.equals(encoded)
        : "encodeAssetId must emit '<asset-definition-id>#<account-id>'";
  }

  private static void encodeAssetIdFromDefinitionBuildsCanonicalLiteral() {
    final String definitionAddress = AssetDefinitionIdEncoder.encode("rose", "wonderland");
    final String encoded = AssetIdEncoder.encodeAssetIdFromDefinition(definitionAddress, ACCOUNT_ID);

    assert (definitionAddress + "#" + ACCOUNT_ID).equals(encoded)
        : "encodeAssetIdFromDefinition must preserve canonical public formatting";
  }

  private static void encodeScopedAssetIdFromDefinitionAppendsDataspaceSuffix() {
    final String definitionAddress = AssetDefinitionIdEncoder.encode("rose", "wonderland");
    final String encoded =
        AssetIdEncoder.encodeScopedAssetIdFromDefinition(definitionAddress, ACCOUNT_ID, 42L);

    assert (definitionAddress + "#" + ACCOUNT_ID + "#dataspace:42").equals(encoded)
        : "encodeScopedAssetIdFromDefinition must append the dataspace suffix";
  }

  private static void encodeAssetIdRejectsNonCanonicalAccountId() {
    boolean threw = false;
    try {
      AssetIdEncoder.encodeAssetId("rose", "wonderland", "ed0120ABCD@wonderland");
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage() != null
              && ex.getMessage().contains("canonical I105");
    }

    assert threw : "encodeAssetId must reject non-canonical account literals";
  }
}

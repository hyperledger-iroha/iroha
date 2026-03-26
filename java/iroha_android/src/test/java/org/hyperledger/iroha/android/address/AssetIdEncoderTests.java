// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.address;

public final class AssetIdEncoderTests {

  private AssetIdEncoderTests() {}

  public static void main(final String[] args) {
    encodeAssetIdUsesCanonicalDefinitionAddress();
    encodeAssetIdFromDefinitionPreservesCanonicalDefinition();
    encodeScopedAssetIdFromDefinitionPreservesCanonicalDefinition();
    System.out.println("[IrohaAndroid] AssetIdEncoder tests passed.");
  }

  private static void encodeAssetIdUsesCanonicalDefinitionAddress() {
    final String encoded = AssetIdEncoder.encodeAssetId("rose", "wonderland", "ignored-account");
    final String expected = AssetDefinitionIdEncoder.encode("rose", "wonderland");

    assert expected.equals(encoded)
        : "encodeAssetId must emit the canonical Base58 asset-definition id";
  }

  private static void encodeAssetIdFromDefinitionPreservesCanonicalDefinition() {
    final String definitionAddress = AssetDefinitionIdEncoder.encode("rose", "wonderland");
    final String encoded = AssetIdEncoder.encodeAssetIdFromDefinition(definitionAddress, "ignored-account");

    assert definitionAddress.equals(encoded)
        : "encodeAssetIdFromDefinition must preserve the canonical Base58 definition";
  }

  private static void encodeScopedAssetIdFromDefinitionPreservesCanonicalDefinition() {
    final String definitionAddress = AssetDefinitionIdEncoder.encode("rose", "wonderland");
    final String encoded =
        AssetIdEncoder.encodeScopedAssetIdFromDefinition(definitionAddress, "ignored-account", 42L);

    assert definitionAddress.equals(encoded)
        : "encodeScopedAssetIdFromDefinition must preserve the canonical Base58 definition";
  }
}

// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.address;

import org.hyperledger.iroha.android.testing.TestAssetDefinitionIds;

public final class AssetIdEncoderTests {

  private AssetIdEncoderTests() {}

  public static void main(final String[] args) {
    encodeAssetIdFromDefinitionPreservesCanonicalDefinition();
    System.out.println("[IrohaAndroid] AssetIdEncoder tests passed.");
  }

  private static void encodeAssetIdFromDefinitionPreservesCanonicalDefinition() {
    final String definitionAddress = TestAssetDefinitionIds.PRIMARY;
    final String encoded = AssetIdEncoder.encodeAssetIdFromDefinition(definitionAddress);

    assert definitionAddress.equals(encoded)
        : "encodeAssetIdFromDefinition must preserve the canonical Base58 definition";
  }
}

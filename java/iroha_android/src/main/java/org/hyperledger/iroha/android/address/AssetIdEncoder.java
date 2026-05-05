// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.address;

/** Public asset identifiers are canonical unprefixed Base58 asset-definition ids only. */
public final class AssetIdEncoder {

  private AssetIdEncoder() {}

  /** Returns the canonical public asset identifier from a definition address. */
  public static String encodeAssetIdFromDefinition(String definitionAddress) {
    return canonicalDefinitionAddress(definitionAddress);
  }

  private static String canonicalDefinitionAddress(String definitionAddress) {
    final String trimmed = definitionAddress.trim();
    if (!trimmed.equals(definitionAddress) || trimmed.isEmpty()) {
      throw new IllegalArgumentException(
          "assetDefinitionId must use canonical unprefixed Base58 form");
    }
    AssetDefinitionIdEncoder.parseAddressBytes(trimmed);
    return trimmed;
  }
}

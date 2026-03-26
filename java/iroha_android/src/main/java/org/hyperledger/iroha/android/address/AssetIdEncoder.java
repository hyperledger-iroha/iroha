// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.address;

import java.util.Objects;

/** Public asset identifiers are canonical unprefixed Base58 asset-definition ids only. */
public final class AssetIdEncoder {

  private AssetIdEncoder() {}

  /** Computes a canonical asset-holding identifier from asset name and domain. */
  public static String encodeAssetId(String assetName, String domainName, String accountId) {
    Objects.requireNonNull(assetName, "assetName");
    Objects.requireNonNull(domainName, "domainName");
    Objects.requireNonNull(accountId, "accountId");
    return encodeAssetIdFromDefinition(AssetDefinitionIdEncoder.encode(assetName, domainName), accountId);
  }

  /** Returns the canonical asset-holding identifier from a definition address. */
  public static String encodeAssetIdFromDefinition(String definitionAddress, String accountId) {
    Objects.requireNonNull(definitionAddress, "definitionAddress");
    Objects.requireNonNull(accountId, "accountId");
    return canonicalDefinitionAddress(definitionAddress);
  }

  /** Dataspace does not change the canonical asset-holding identifier in first-release semantics. */
  public static String encodeScopedAssetIdFromDefinition(
      String definitionAddress, String accountId, long dataspaceId) {
    Objects.requireNonNull(accountId, "accountId");
    if (dataspaceId < 0L) {
      throw new IllegalArgumentException("dataspaceId must be non-negative");
    }
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

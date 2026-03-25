// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.address;

import java.util.Objects;

/**
 * Composes canonical public asset identifiers.
 *
 * <p>Public asset literals use:
 *
 * <ul>
 *   <li>{@code <asset-definition-id>#<account-id>}
 *   <li>{@code <asset-definition-id>#<account-id>#dataspace:<id>}
 * </ul>
 */
public final class AssetIdEncoder {

  private AssetIdEncoder() {}

  /**
   * Computes a canonical public asset identifier from asset name, domain, and account id.
   */
  public static String encodeAssetId(String assetName, String domainName, String accountId) {
    Objects.requireNonNull(assetName, "assetName");
    Objects.requireNonNull(domainName, "domainName");
    return encodeAssetIdFromDefinition(AssetDefinitionIdEncoder.encode(assetName, domainName), accountId);
  }

  /**
   * Composes a canonical public asset identifier from a definition address and account id.
   */
  public static String encodeAssetIdFromDefinition(String definitionAddress, String accountId) {
    Objects.requireNonNull(definitionAddress, "definitionAddress");
    Objects.requireNonNull(accountId, "accountId");
    final String canonicalDefinition = canonicalDefinitionAddress(definitionAddress);
    final String canonicalAccount = AccountIdLiteral.requireCanonicalI105Address(accountId, "accountId");
    return canonicalDefinition + "#" + canonicalAccount;
  }

  /**
   * Composes a dataspace-scoped canonical public asset identifier.
   */
  public static String encodeScopedAssetIdFromDefinition(
      String definitionAddress, String accountId, long dataspaceId) {
    if (dataspaceId < 0L) {
      throw new IllegalArgumentException("dataspaceId must be non-negative");
    }
    return encodeAssetIdFromDefinition(definitionAddress, accountId) + "#dataspace:" + dataspaceId;
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

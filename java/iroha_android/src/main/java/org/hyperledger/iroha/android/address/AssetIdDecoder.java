// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.address;

import java.util.Objects;

/**
 * Parses canonical public asset identifiers.
 */
public final class AssetIdDecoder {

  private AssetIdDecoder() {}

  /** Parsed canonical asset identifier. */
  public static final class AssetId {
    private final AssetDefinition definition;
    private final String accountId;
    private final Long dataspaceId;

    AssetId(AssetDefinition definition, String accountId, Long dataspaceId) {
      this.definition = Objects.requireNonNull(definition, "definition");
      this.accountId = Objects.requireNonNull(accountId, "accountId");
      this.dataspaceId = dataspaceId;
    }

    public AssetDefinition definition() {
      return definition;
    }

    public String accountId() {
      return accountId;
    }

    public Long dataspaceId() {
      return dataspaceId;
    }
  }

  /** Decoded asset definition carrying the canonical Base58 address. */
  public static final class AssetDefinition {
    private final String address;

    AssetDefinition(String address) {
      this.address = Objects.requireNonNull(address, "address");
    }

    public String address() {
      return address;
    }
  }

  /**
   * Checks whether the given value is a canonical public asset literal.
   */
  public static boolean isCanonical(String value) {
    if (value == null) {
      return false;
    }
    try {
      decode(value);
      return true;
    } catch (IllegalArgumentException ex) {
      return false;
    }
  }

  /**
   * Parses {@code <asset-definition-id>#<account-id>} with an optional
   * {@code #dataspace:<id>} suffix.
   */
  public static AssetId decode(String assetId) {
    Objects.requireNonNull(assetId, "assetId");
    final String trimmed = assetId.trim();
    if (!trimmed.equals(assetId) || trimmed.isEmpty()) {
      throw new IllegalArgumentException("AssetId must use canonical public form");
    }

    final String[] parts = trimmed.split("#", -1);
    if (parts.length != 2 && parts.length != 3) {
      throw new IllegalArgumentException(
          "AssetId must use '<asset-definition-id>#<account-id>' with optional '#dataspace:<id>' suffix");
    }

    AssetDefinitionIdEncoder.parseAddressBytes(parts[0]);
    final String canonicalAccount =
        AccountIdLiteral.requireCanonicalI105Address(parts[1], "AssetId.account");
    final Long dataspaceId = parts.length == 3 ? parseDataspace(parts[2]) : null;
    return new AssetId(new AssetDefinition(parts[0]), canonicalAccount, dataspaceId);
  }

  /**
   * Validates a canonical asset-definition address and returns it.
   */
  public static AssetDefinition decodeDefinition(String definitionId) {
    Objects.requireNonNull(definitionId, "definitionId");
    final String trimmed = definitionId.trim();
    if (!trimmed.equals(definitionId) || trimmed.isEmpty()) {
      throw new IllegalArgumentException(
          "Asset definition id must use canonical unprefixed Base58 form");
    }
    AssetDefinitionIdEncoder.parseAddressBytes(trimmed);
    return new AssetDefinition(trimmed);
  }

  private static long parseDataspace(String scopeLiteral) {
    final String prefix = "dataspace:";
    if (!scopeLiteral.startsWith(prefix)) {
      throw new IllegalArgumentException(
          "AssetId.scope must use 'dataspace:<id>' when present");
    }
    final String raw = scopeLiteral.substring(prefix.length());
    if (raw.isEmpty()) {
      throw new IllegalArgumentException(
          "AssetId.scope must use 'dataspace:<id>' when present");
    }
    try {
      final long parsed = Long.parseLong(raw);
      if (parsed < 0L) {
        throw new IllegalArgumentException("AssetId.scope dataspace must be non-negative");
      }
      return parsed;
    } catch (NumberFormatException ex) {
      throw new IllegalArgumentException(
          "AssetId.scope must use 'dataspace:<id>' when present",
          ex);
    }
  }
}

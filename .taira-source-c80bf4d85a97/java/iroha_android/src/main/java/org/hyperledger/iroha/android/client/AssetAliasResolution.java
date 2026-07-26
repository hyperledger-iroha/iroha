package org.hyperledger.iroha.android.client;

import java.util.Objects;

/** Parsed payload for asset alias resolution (`/v1/assets/aliases/resolve`). */
public final class AssetAliasResolution {
  private final String alias;
  private final String assetDefinitionId;
  private final String assetName;
  private final String description;
  private final String logo;
  private final String source;

  public AssetAliasResolution(
      final String alias,
      final String assetDefinitionId,
      final String assetName,
      final String description,
      final String logo,
      final String source) {
    this.alias = Objects.requireNonNull(alias, "alias");
    this.assetDefinitionId = Objects.requireNonNull(assetDefinitionId, "assetDefinitionId");
    this.assetName = Objects.requireNonNull(assetName, "assetName");
    this.description = description;
    this.logo = logo;
    this.source = source;
  }

  public String alias() {
    return alias;
  }

  public String assetDefinitionId() {
    return assetDefinitionId;
  }

  public String assetName() {
    return assetName;
  }

  public String description() {
    return description;
  }

  public String logo() {
    return logo;
  }

  public String source() {
    return source;
  }
}

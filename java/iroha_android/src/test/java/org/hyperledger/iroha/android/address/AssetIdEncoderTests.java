// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.address;

import java.util.Locale;

public final class AssetIdEncoderTests {

  private static final String ED25519_KEY =
      "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";

  private AssetIdEncoderTests() {}

  public static void main(final String[] args) {
    encodeAssetIdCanonicalizesPublicKeyLiteral();
    encodeAssetIdFromDefinitionCanonicalizesPublicKeyLiteral();
    encodeAssetIdRejectsInvalidPublicKeyLiteral();
    System.out.println("[IrohaAndroid] AssetIdEncoder tests passed.");
  }

  private static void encodeAssetIdCanonicalizesPublicKeyLiteral() {
    final String canonical =
        AssetIdEncoder.encodeAssetId("rose", "wonderland", ED25519_KEY);
    final String lowerCase =
        AssetIdEncoder.encodeAssetId(
            "rose", "wonderland", ED25519_KEY.toLowerCase(Locale.ROOT));

    assert canonical.equals(lowerCase)
        : "encodeAssetId must normalize public key literals to the canonical multihash form";
  }

  private static void encodeAssetIdFromDefinitionCanonicalizesPublicKeyLiteral() {
    final String definitionAddress = AssetDefinitionIdEncoder.encode("rose", "wonderland");
    final String canonical =
        AssetIdEncoder.encodeAssetIdFromDefinition(definitionAddress, ED25519_KEY);
    final String prefixed =
        AssetIdEncoder.encodeAssetIdFromDefinition(
            definitionAddress, "pk:" + ED25519_KEY.toLowerCase(Locale.ROOT));

    assert canonical.equals(prefixed)
        : "encodeAssetIdFromDefinition must canonicalize hex-looking public key literals";
  }

  private static void encodeAssetIdRejectsInvalidPublicKeyLiteral() {
    boolean threw = false;
    try {
      AssetIdEncoder.encodeAssetId("rose", "wonderland", "ed0120ABCD");
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage() != null && ex.getMessage().contains("Invalid public key literal");
    }

    assert threw : "encodeAssetId must reject malformed multihash public key literals";
  }
}

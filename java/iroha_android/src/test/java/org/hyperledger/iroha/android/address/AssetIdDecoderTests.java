// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.address;

import org.hyperledger.iroha.android.testing.TestAssetDefinitionIds;

public final class AssetIdDecoderTests {

  private AssetIdDecoderTests() {}

  public static void main(final String[] args) {
    isCanonicalReturnsTrueForCanonicalDefinitionId();
    isCanonicalReturnsFalseForOwnerQualifiedLiteral();
    decodeReturnsDefinitionOnly();
    decodeRejectsMalformedAssetLiteral();
    decodeDefinitionAcceptsCanonicalAddress();
    System.out.println("[IrohaAndroid] AssetIdDecoder tests passed.");
  }

  private static void isCanonicalReturnsTrueForCanonicalDefinitionId() {
    final String definitionAddress = TestAssetDefinitionIds.PRIMARY;
    assert AssetIdDecoder.isCanonical(definitionAddress)
        : "isCanonical must accept canonical Base58 asset-definition ids";
  }

  private static void isCanonicalReturnsFalseForOwnerQualifiedLiteral() {
    final String definitionAddress = TestAssetDefinitionIds.PRIMARY;
    assert !AssetIdDecoder.isCanonical(definitionAddress + "#not-public")
        : "isCanonical must reject owner-qualified asset literals";
  }

  private static void decodeReturnsDefinitionOnly() {
    final String definitionAddress = TestAssetDefinitionIds.PRIMARY;
    final AssetIdDecoder.AssetId decoded = AssetIdDecoder.decode(definitionAddress);

    assert definitionAddress.equals(decoded.definition().address())
        : "definition address mismatch";
    assert decoded.accountId().isEmpty() : "account id must be empty for public asset ids";
    assert decoded.dataspaceId() == null : "dataspace must be absent for public asset ids";
  }

  private static void decodeRejectsMalformedAssetLiteral() {
    boolean threw = false;
    try {
      AssetIdDecoder.decode("not:an-asset#owner");
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage() != null
              && (ex.getMessage().contains("Base58") || ex.getMessage().contains("canonical"));
    }

    assert threw : "decode must reject malformed asset literals";
  }

  private static void decodeDefinitionAcceptsCanonicalAddress() {
    final String definitionAddress = TestAssetDefinitionIds.TERTIARY;
    final AssetIdDecoder.AssetDefinition decoded = AssetIdDecoder.decodeDefinition(definitionAddress);

    assert definitionAddress.equals(decoded.address())
        : "decodeDefinition must preserve the canonical address";
  }
}

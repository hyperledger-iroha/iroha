// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.address;

import org.hyperledger.iroha.android.testing.TestAccountIds;

public final class AssetIdDecoderTests {

  private static final String ACCOUNT_ID = TestAccountIds.ed25519Authority(0x11);

  private AssetIdDecoderTests() {}

  public static void main(final String[] args) {
    isCanonicalReturnsTrueForCanonicalAssetLiteral();
    isCanonicalReturnsFalseForMalformedAssetLiteral();
    decodeExtractsDefinitionAddressAccountAndScope();
    decodeRejectsMalformedScope();
    decodeRejectsMalformedAssetLiteral();
    decodeDefinitionAcceptsCanonicalAddress();
    System.out.println("[IrohaAndroid] AssetIdDecoder tests passed.");
  }

  private static void isCanonicalReturnsTrueForCanonicalAssetLiteral() {
    final String definitionAddress = AssetDefinitionIdEncoder.encode("rose", "wonderland");
    assert AssetIdDecoder.isCanonical(definitionAddress + "#" + ACCOUNT_ID)
        : "isCanonical must accept canonical public asset literals";
  }

  private static void isCanonicalReturnsFalseForMalformedAssetLiteral() {
    assert !AssetIdDecoder.isCanonical("not:an-asset")
        : "isCanonical must reject malformed asset literals";
  }

  private static void decodeExtractsDefinitionAddressAccountAndScope() {
    final String definitionAddress = AssetDefinitionIdEncoder.encode("rose", "wonderland");
    final AssetIdDecoder.AssetId decoded =
        AssetIdDecoder.decode(definitionAddress + "#" + ACCOUNT_ID + "#dataspace:7");

    assert definitionAddress.equals(decoded.definition().address())
        : "definition address mismatch";
    assert ACCOUNT_ID.equals(decoded.accountId()) : "account id mismatch";
    assert Long.valueOf(7L).equals(decoded.dataspaceId()) : "dataspace mismatch";
  }

  private static void decodeRejectsMalformedScope() {
    final String definitionAddress = AssetDefinitionIdEncoder.encode("rose", "wonderland");
    boolean threw = false;
    try {
      AssetIdDecoder.decode(definitionAddress + "#" + ACCOUNT_ID + "#scope:7");
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage() != null
              && ex.getMessage().contains("dataspace:<id>");
    }

    assert threw : "decode must reject non-dataspace scope suffixes";
  }

  private static void decodeRejectsMalformedAssetLiteral() {
    boolean threw = false;
    try {
      AssetIdDecoder.decode("not:an-asset");
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage() != null
              && ex.getMessage().contains("<asset-definition-id>#<account-id>");
    }

    assert threw : "decode must reject malformed asset literals";
  }

  private static void decodeDefinitionAcceptsCanonicalAddress() {
    final String definitionAddress = AssetDefinitionIdEncoder.encode("pkr", "sbp");
    final AssetIdDecoder.AssetDefinition decoded = AssetIdDecoder.decodeDefinition(definitionAddress);

    assert definitionAddress.equals(decoded.address())
        : "decodeDefinition must preserve the canonical address";
  }
}

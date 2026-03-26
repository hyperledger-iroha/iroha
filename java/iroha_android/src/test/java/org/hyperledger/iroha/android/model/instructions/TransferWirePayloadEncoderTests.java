// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.model.instructions;

import java.util.Arrays;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.testing.TestAccountIds;
import org.hyperledger.iroha.android.testing.TestAssetDefinitionIds;

public final class TransferWirePayloadEncoderTests {

  private static final String ACCOUNT_ID = TestAccountIds.ed25519Authority(0x11);

  private TransferWirePayloadEncoderTests() {}

  public static void main(final String[] args) throws Exception {
    encodeAssetTransferAcceptsCanonicalAssetId();
    encodeAssetTransferAcceptsDataspaceScopedAssetId();
    encodeAssetTransferAcceptsMultisigI105AssetOwner();
    encodeAssetTransferRejectsMalformedAssetId();
    encodeAssetTransferRejectsLegacyNameDomainAssetId();
    encodeAssetTransferRejectsMalformedScopeSuffix();
    encodeAssetTransferAcceptsMlDsaI105WhenCurveSupportEnabled();
    encodeAssetTransferAcceptsGostI105WhenCurveSupportEnabled();
    encodeAssetTransferAcceptsSm2I105WhenCurveSupportEnabled();
    System.out.println("[IrohaAndroid] TransferWirePayloadEncoder tests passed.");
  }

  private static void encodeAssetTransferAcceptsCanonicalAssetId() {
    final String definitionAddress = TestAssetDefinitionIds.PRIMARY;
    final InstructionBox box =
        TransferWirePayloadEncoder.encodeAssetTransfer(
            definitionAddress + "#" + ACCOUNT_ID, "10", ACCOUNT_ID);

    assert wirePayloadBytes(box).length > 0 : "canonical asset ids must encode transfer payloads";
  }

  private static void encodeAssetTransferAcceptsDataspaceScopedAssetId() {
    final String definitionAddress = TestAssetDefinitionIds.PRIMARY;
    final InstructionBox box =
        TransferWirePayloadEncoder.encodeAssetTransfer(
            definitionAddress + "#" + ACCOUNT_ID + "#dataspace:42", "10", ACCOUNT_ID);

    assert wirePayloadBytes(box).length > 0
        : "dataspace-scoped asset ids must encode transfer payloads";
  }

  private static void encodeAssetTransferAcceptsMultisigI105AssetOwner() throws Exception {
    final AccountAddress.MultisigPolicyPayload policy =
        AccountAddress.MultisigPolicyPayload.of(
            1,
            2,
            Arrays.asList(
                AccountAddress.MultisigMemberPayload.of(1, 1, filledKey((byte) 0x11)),
                AccountAddress.MultisigMemberPayload.of(1, 1, filledKey((byte) 0x22))));
    final String multisigAccountId =
        AccountAddress.fromMultisigPolicy(policy).toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    final String definitionAddress = TestAssetDefinitionIds.PRIMARY;

    final InstructionBox box =
        TransferWirePayloadEncoder.encodeAssetTransfer(
            definitionAddress + "#" + multisigAccountId, "10", ACCOUNT_ID);

    assert wirePayloadBytes(box).length > 0
        : "multisig I105 asset owners must encode transfer payloads";
  }

  private static void encodeAssetTransferRejectsMalformedAssetId() {
    boolean threw = false;
    try {
      TransferWirePayloadEncoder.encodeAssetTransfer("not:an-asset", "10", ACCOUNT_ID);
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage() != null
              && ex.getMessage().contains("<base58-asset-definition-id>#<katakana-i105-account-id>");
    }

    assert threw : "malformed asset ids must be rejected";
  }

  private static void encodeAssetTransferRejectsLegacyNameDomainAssetId() {
    boolean threw = false;
    try {
      TransferWirePayloadEncoder.encodeAssetTransfer(
          "rose#wonderland##alice@hbl.dataspace", "10", ACCOUNT_ID);
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage() != null
              && ex.getMessage().contains("<base58-asset-definition-id>#<katakana-i105-account-id>");
    }

    assert threw : "legacy name/domain asset ids must be rejected";
  }

  private static void encodeAssetTransferRejectsMalformedScopeSuffix() {
    final String definitionAddress = TestAssetDefinitionIds.PRIMARY;
    boolean threw = false;
    try {
      TransferWirePayloadEncoder.encodeAssetTransfer(
          definitionAddress + "#" + ACCOUNT_ID + "#scope:42", "10", ACCOUNT_ID);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage() != null && ex.getMessage().contains("dataspace:<id>");
    }

    assert threw : "malformed scope suffixes must be rejected";
  }

  private static void encodeAssetTransferAcceptsMlDsaI105WhenCurveSupportEnabled()
      throws Exception {
    assertI105CurveAccepted(
        "ml-dsa",
        AccountAddress.CurveSupportConfig.builder().allowMlDsa(true).build(),
        (byte) 0x11);
  }

  private static void encodeAssetTransferAcceptsGostI105WhenCurveSupportEnabled()
      throws Exception {
    assertI105CurveAccepted(
        "gost256a",
        AccountAddress.CurveSupportConfig.builder().allowGost(true).build(),
        (byte) 0x22);
  }

  private static void encodeAssetTransferAcceptsSm2I105WhenCurveSupportEnabled()
      throws Exception {
    assertI105CurveAccepted(
        "sm2",
        AccountAddress.CurveSupportConfig.builder().allowSm2(true).build(),
        (byte) 0x33);
  }

  private static void assertI105CurveAccepted(
      final String algorithm,
      final AccountAddress.CurveSupportConfig curveSupport,
      final byte keyFill)
      throws Exception {
    final byte[] key = filledKey(keyFill);
    final String definitionAddress = TestAssetDefinitionIds.PRIMARY;
    final String i105AccountId;

    AccountAddress.configureCurveSupport(curveSupport);
    try {
      final AccountAddress address = AccountAddress.fromAccount(key, algorithm);
      i105AccountId = address.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
      final InstructionBox box =
          TransferWirePayloadEncoder.encodeAssetTransfer(
              definitionAddress + "#" + i105AccountId, "10", i105AccountId);

      assert wirePayloadBytes(box).length > 0
          : "I105 " + algorithm + " account ids must encode transfer payloads";
    } finally {
      AccountAddress.configureCurveSupport(AccountAddress.CurveSupportConfig.ed25519Only());
    }
  }

  private static byte[] wirePayloadBytes(final InstructionBox box) {
    if (!(box.payload() instanceof InstructionBox.WirePayload wirePayload)) {
      throw new AssertionError("expected wire payload");
    }
    return wirePayload.payloadBytes();
  }

  private static byte[] filledKey(final byte fill) {
    final byte[] key = new byte[32];
    Arrays.fill(key, fill);
    return key;
  }
}

// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.model.instructions;

import java.util.Arrays;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AssetIdEncoder;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.model.InstructionBox;

public final class TransferWirePayloadEncoderTests {

  private static final String ED25519_KEY =
      "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";

  private TransferWirePayloadEncoderTests() {}

  public static void main(final String[] args) throws Exception {
    encodeAssetTransferAcceptsCanonicalNoritoAssetId();
    encodeAssetTransferAcceptsMlDsaI105WhenCurveSupportDisabled();
    encodeAssetTransferAcceptsGostI105WhenCurveSupportDisabled();
    encodeAssetTransferAcceptsSm2I105WhenCurveSupportDisabled();
    System.out.println("[IrohaAndroid] TransferWirePayloadEncoder tests passed.");
  }

  private static void encodeAssetTransferAcceptsCanonicalNoritoAssetId() {
    final String noritoAssetId = AssetIdEncoder.encodeAssetId("rose", "wonderland", ED25519_KEY);

    final InstructionBox fromNorito =
        TransferWirePayloadEncoder.encodeAssetTransfer(
            noritoAssetId, "10", ED25519_KEY + "@wonderland");
    final InstructionBox fromLegacy =
        TransferWirePayloadEncoder.encodeAssetTransfer(
            "rose#wonderland#" + ED25519_KEY + "@wonderland",
            "10",
            ED25519_KEY + "@wonderland");

    assert Arrays.equals(wirePayloadBytes(fromNorito), wirePayloadBytes(fromLegacy))
        : "canonical norito asset ids must encode to the same transfer payload as legacy asset text";
  }

  private static void encodeAssetTransferAcceptsMlDsaI105WhenCurveSupportDisabled()
      throws Exception {
    assertI105CurveRoundTrip(
        "ml-dsa",
        AccountAddress.CurveSupportConfig.builder().allowMlDsa(true).build(),
        (byte) 0x11);
  }

  private static void encodeAssetTransferAcceptsGostI105WhenCurveSupportDisabled()
      throws Exception {
    assertI105CurveRoundTrip(
        "gost256a",
        AccountAddress.CurveSupportConfig.builder().allowGost(true).build(),
        (byte) 0x22);
  }

  private static void encodeAssetTransferAcceptsSm2I105WhenCurveSupportDisabled()
      throws Exception {
    assertI105CurveRoundTrip(
        "sm2",
        AccountAddress.CurveSupportConfig.builder().allowSm2(true).build(),
        (byte) 0x33);
  }

  private static void assertI105CurveRoundTrip(
      final String algorithm,
      final AccountAddress.CurveSupportConfig curveSupport,
      final byte keyFill)
      throws Exception {
    final byte[] key = filledKey(keyFill);
    final String i105AccountId;
    final String multihashAccountId;

    AccountAddress.configureCurveSupport(curveSupport);
    try {
      final AccountAddress address = AccountAddress.fromAccount(key, algorithm);
      i105AccountId = address.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT) + "@wonderland";
      final AccountAddress.SingleKeyPayload payload =
          address.singleKeyPayload().orElseThrow(() -> new AssertionError("expected single-key address"));
      multihashAccountId =
          PublicKeyCodec.encodePublicKeyMultihash(payload.curveId(), payload.publicKey())
              + "@wonderland";
    } finally {
      AccountAddress.configureCurveSupport(AccountAddress.CurveSupportConfig.ed25519Only());
    }

    final InstructionBox fromI105 =
        TransferWirePayloadEncoder.encodeAssetTransfer(
            "rose#wonderland#" + i105AccountId, "10", i105AccountId);
    final InstructionBox fromMultihash =
        TransferWirePayloadEncoder.encodeAssetTransfer(
            "rose#wonderland#" + multihashAccountId, "10", multihashAccountId);

    assert Arrays.equals(wirePayloadBytes(fromI105), wirePayloadBytes(fromMultihash))
        : "I105 " + algorithm + " account ids must round-trip to the same transfer payload";
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

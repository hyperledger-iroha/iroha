// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.model.instructions;

import java.util.Arrays;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.numeric.NumericV1;
import org.hyperledger.iroha.android.sccp.SccpV1;
import org.hyperledger.iroha.android.testing.TestAssetDefinitionIds;
import org.hyperledger.iroha.android.testing.TestEd25519Keys;
import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoHeader;

public final class TransferWirePayloadEncoderTests {

  private static final String ACCOUNT_ID = tairaAccountId(0x11);

  private TransferWirePayloadEncoderTests() {}

  public static void main(final String[] args) throws Exception {
    encodeAssetTransferAcceptsCanonicalAssetId();
    encodeAssetTransferAcceptsDataspaceScopedAssetId();
    decodeAssetTransferRoundTripsCanonicalStrings();
    encodeAssetTransferRejectsNoncanonicalQuantities();
    decodeAssetTransferRejectsNoncanonicalQuantity();
    encodeAssetTransferAcceptsMultisigI105AssetOwner();
    decodeAccountIdRoundTripsMultisigI105();
    encodeAssetTransferRejectsMalformedAssetId();
    encodeAssetTransferRejectsNameDomainAssetId();
    encodeAssetTransferRejectsMalformedScopeSuffix();
    decodeAssetTransferRejectsUnsupportedDiscriminant();
    decodeAccountIdRejectsTrailingBytes();
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
    assert hex(wirePayloadBytes(box)).contains("0100000009082a00000000000000")
        : "dataspace scope must use canonical DataSpaceId payload length encoding";
  }

  private static void decodeAssetTransferRoundTripsCanonicalStrings() {
    final String definitionAddress = TestAssetDefinitionIds.PRIMARY;
    final String assetId = definitionAddress + "#" + ACCOUNT_ID + "#dataspace:42";
    final InstructionBox box =
        TransferWirePayloadEncoder.encodeAssetTransfer(assetId, "10.5", ACCOUNT_ID);

    final TransferWirePayloadEncoder.DecodedAssetTransfer decoded =
        TransferWirePayloadEncoder.decodeAssetTransferPayload(
            wirePayloadBytes(box), SccpV1.TAIRA_I105_DISCRIMINANT_V1);

    assert assetId.equals(decoded.assetId()) : "decoded asset id mismatch";
    assert "10.5".equals(decoded.amount()) : "decoded amount mismatch";
    assert ACCOUNT_ID.equals(decoded.destinationAccountId()) : "decoded destination mismatch";
    assert AccountAddress.detectI105Discriminant(decoded.destinationAccountId())
            == SccpV1.TAIRA_I105_DISCRIMINANT_V1
        : "decoded destination must retain the exact Taira discriminant";
  }

  private static void encodeAssetTransferRejectsNoncanonicalQuantities() {
    final String assetId = TestAssetDefinitionIds.PRIMARY + "#" + ACCOUNT_ID;
    for (final String amount : new String[] {" ", "+1", "01", "1e0", "-1", "1.0", "1.2300"}) {
      boolean threw = false;
      try {
        TransferWirePayloadEncoder.encodeAssetTransfer(assetId, amount, ACCOUNT_ID);
      } catch (final IllegalArgumentException expected) {
        threw = true;
      }
      assert threw : "noncanonical quantity was accepted: " + amount;
    }

    final InstructionBox typed =
        TransferWirePayloadEncoder.encodeAssetTransfer(
            assetId, NumericV1.QuantityValue.parseCanonical("10"), ACCOUNT_ID);
    assert wirePayloadBytes(typed).length > 0 : "typed quantity must encode";
  }

  private static void decodeAssetTransferRejectsNoncanonicalQuantity() {
    final InstructionBox box =
        TransferWirePayloadEncoder.encodeAssetTransfer(
            TestAssetDefinitionIds.PRIMARY + "#" + ACCOUNT_ID, "10", ACCOUNT_ID);
    final NoritoHeader.DecodeResult decoded = NoritoHeader.decode(wirePayloadBytes(box), null);
    decoded.header().validateChecksum(decoded.payload());
    final byte[] payload = decoded.payload().clone();
    final byte[] canonicalNumeric = {5, 1, 0, 0, 0, 10, 4, 0, 0, 0, 0};
    final int numericOffset = indexOf(payload, canonicalNumeric);
    assert numericOffset >= 0 : "canonical transfer numeric payload was not found";
    payload[numericOffset + canonicalNumeric.length - 1] = 1;

    boolean threw = false;
    try {
      TransferWirePayloadEncoder.decodeAssetTransferPayload(
          reframe(decoded.header(), payload), SccpV1.TAIRA_I105_DISCRIMINANT_V1);
    } catch (final IllegalArgumentException expected) {
      threw = true;
    }
    assert threw : "noncanonical transfer quantity wire payload must be rejected";
  }

  private static void encodeAssetTransferAcceptsMultisigI105AssetOwner() throws Exception {
    final AccountAddress.MultisigPolicyPayload policy =
        AccountAddress.MultisigPolicyPayload.of(
            1,
            2,
            Arrays.asList(
                AccountAddress.MultisigMemberPayload.of(1, 1, TestEd25519Keys.publicKey(0x00)),
                AccountAddress.MultisigMemberPayload.of(1, 1, TestEd25519Keys.publicKey(0x1F))));
    final String multisigAccountId =
        AccountAddress.fromMultisigPolicy(policy)
            .toI105(SccpV1.TAIRA_I105_DISCRIMINANT_V1);
    final String definitionAddress = TestAssetDefinitionIds.PRIMARY;

    final InstructionBox box =
        TransferWirePayloadEncoder.encodeAssetTransfer(
            definitionAddress + "#" + multisigAccountId, "10", ACCOUNT_ID);

    assert wirePayloadBytes(box).length > 0
        : "multisig I105 asset owners must encode transfer payloads";
  }

  private static void decodeAccountIdRoundTripsMultisigI105() throws Exception {
    final AccountAddress.MultisigPolicyPayload policy =
        AccountAddress.MultisigPolicyPayload.of(
            1,
            2,
            Arrays.asList(
                AccountAddress.MultisigMemberPayload.of(1, 1, TestEd25519Keys.publicKey(0x00)),
                AccountAddress.MultisigMemberPayload.of(1, 1, TestEd25519Keys.publicKey(0x1F))));
    final String multisigAccountId =
        AccountAddress.fromMultisigPolicy(policy)
            .toI105(SccpV1.TAIRA_I105_DISCRIMINANT_V1);
    final byte[] payload = TransferWirePayloadEncoder.encodeAccountIdPayload(multisigAccountId);

    assert multisigAccountId.equals(
            TransferWirePayloadEncoder.decodeAccountIdPayload(
                payload, SccpV1.TAIRA_I105_DISCRIMINANT_V1))
        : "multisig AccountId payload must decode to canonical I105";
  }

  private static void encodeAssetTransferRejectsMalformedAssetId() {
    boolean threw = false;
    try {
      TransferWirePayloadEncoder.encodeAssetTransfer("not:an-asset", "10", ACCOUNT_ID);
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage() != null
              && ex.getMessage().contains("<base58-asset-definition-id>#<i105-account-id>");
    }

    assert threw : "malformed asset ids must be rejected";
  }

  private static void encodeAssetTransferRejectsNameDomainAssetId() {
    boolean threw = false;
    try {
      TransferWirePayloadEncoder.encodeAssetTransfer(
          "rose#wonderland##alice@banka.dataspace", "10", ACCOUNT_ID);
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage() != null
              && ex.getMessage().contains("<base58-asset-definition-id>#<i105-account-id>");
    }

    assert threw : "name/domain asset ids must be rejected";
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

  private static void decodeAssetTransferRejectsUnsupportedDiscriminant() {
    final InstructionBox box =
        TransferWirePayloadEncoder.encodeAssetTransfer(
            TestAssetDefinitionIds.PRIMARY + "#" + ACCOUNT_ID, "10", ACCOUNT_ID);
    final NoritoHeader.DecodeResult decoded = NoritoHeader.decode(wirePayloadBytes(box), null);
    decoded.header().validateChecksum(decoded.payload());
    final byte[] mutated = decoded.payload().clone();
    mutated[0] = 1;

    boolean threw = false;
    try {
      TransferWirePayloadEncoder.decodeAssetTransferPayload(
          reframe(decoded.header(), mutated), SccpV1.TAIRA_I105_DISCRIMINANT_V1);
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage() != null
              && ex.getMessage().contains("Unsupported TransferBox discriminant");
    }
    assert threw : "unsupported TransferBox discriminants must be rejected";
  }

  private static void decodeAccountIdRejectsTrailingBytes() {
    final byte[] payload = TransferWirePayloadEncoder.encodeAccountIdPayload(ACCOUNT_ID);
    final byte[] withTrailing = Arrays.copyOf(payload, payload.length + 1);
    boolean threw = false;
    try {
      TransferWirePayloadEncoder.decodeAccountIdPayload(
          withTrailing, SccpV1.TAIRA_I105_DISCRIMINANT_V1);
    } catch (final IllegalArgumentException ex) {
      threw = ex.getMessage() != null && ex.getMessage().contains("Trailing bytes");
    }
    assert threw : "AccountId payloads with trailing bytes must be rejected";
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
      i105AccountId = address.toI105(SccpV1.TAIRA_I105_DISCRIMINANT_V1);
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

  private static String tairaAccountId(final int seed) {
    try {
      return AccountAddress.fromAccount(TestEd25519Keys.publicKey(seed), "ed25519")
          .toI105(SccpV1.TAIRA_I105_DISCRIMINANT_V1);
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalStateException("failed to create Taira test account", ex);
    }
  }

  private static String hex(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte value : bytes) {
      builder.append(String.format("%02x", value & 0xFF));
    }
    return builder.toString();
  }

  private static int indexOf(final byte[] haystack, final byte[] needle) {
    for (int start = 0; start <= haystack.length - needle.length; start++) {
      boolean matches = true;
      for (int index = 0; index < needle.length; index++) {
        if (haystack[start + index] != needle[index]) {
          matches = false;
          break;
        }
      }
      if (matches) return start;
    }
    return -1;
  }

  private static byte[] reframe(final NoritoHeader header, final byte[] payload) {
    final NoritoHeader reframed =
        new NoritoHeader(
            header.schemaHash(),
            payload.length,
            CRC64.compute(payload),
            header.flags(),
            NoritoHeader.COMPRESSION_NONE,
            header.minor());
    final byte[] headerBytes = reframed.encode();
    final byte[] out = new byte[headerBytes.length + payload.length];
    System.arraycopy(headerBytes, 0, out, 0, headerBytes.length);
    System.arraycopy(payload, 0, out, headerBytes.length, payload.length);
    return out;
  }
}

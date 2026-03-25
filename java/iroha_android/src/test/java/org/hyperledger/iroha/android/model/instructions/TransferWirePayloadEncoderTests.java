// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.model.instructions;

import java.util.Arrays;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.AssetDefinitionIdEncoder;
import org.hyperledger.iroha.android.address.AssetIdEncoder;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.norito.CRC64;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.SchemaHash;
import org.hyperledger.iroha.norito.TypeAdapter;

public final class TransferWirePayloadEncoderTests {

  private static final String ED25519_KEY =
      "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
  private static final byte[] ASSET_ID_SCHEMA_HASH =
      SchemaHash.hash16("iroha_data_model::asset::id::model::AssetId");
  private static final TypeAdapter<String> STRING_ADAPTER = NoritoAdapters.stringAdapter();
  private static final TypeAdapter<Long> UINT8_ADAPTER = NoritoAdapters.uint(8);
  private static final TypeAdapter<Long> UINT16_ADAPTER = NoritoAdapters.uint(16);
  private static final TypeAdapter<Long> UINT32_ADAPTER = NoritoAdapters.uint(32);

  private TransferWirePayloadEncoderTests() {}

  public static void main(final String[] args) throws Exception {
    encodeAssetTransferAcceptsCanonicalNoritoAssetId();
    encodeAssetTransferAcceptsCompactLenNoritoAssetId();
    encodeAssetTransferAcceptsCompactLenNoritoDataspaceScope();
    encodeAssetTransferAcceptsCompactLenNoritoMultisigAssetId();
    encodeAssetTransferRejectsMalformedNoritoAccountPayload();
    encodeAssetTransferRejectsMalformedNoritoScopePayload();
    encodeAssetTransferRejectsInvalidNoritoMultisigSemantics();
    encodeAssetTransferRejectsUnsupportedNoritoMultisigVersion();
    encodeAssetTransferRejectsUnsupportedNoritoAssetIdFlags();
    encodeAssetTransferRejectsLegacyTextAssetId();
    encodeAssetTransferAcceptsMlDsaI105WhenCurveSupportDisabled();
    encodeAssetTransferAcceptsGostI105WhenCurveSupportDisabled();
    encodeAssetTransferAcceptsSm2I105WhenCurveSupportDisabled();
    System.out.println("[IrohaAndroid] TransferWirePayloadEncoder tests passed.");
  }

  private static void encodeAssetTransferAcceptsCanonicalNoritoAssetId() {
    final String noritoAssetId = AssetIdEncoder.encodeAssetId("rose", "wonderland", ED25519_KEY);
    final String definitionAddress = AssetDefinitionIdEncoder.encode("rose", "wonderland");
    final String destinationAccountId = canonicalAccountIdForPublicKeyMultihash(ED25519_KEY);

    final InstructionBox fromNorito =
        TransferWirePayloadEncoder.encodeAssetTransfer(noritoAssetId, "10", destinationAccountId);
    final InstructionBox fromCanonicalLiteral =
        TransferWirePayloadEncoder.encodeAssetTransfer(
            definitionAddress + "#" + destinationAccountId,
            "10",
            destinationAccountId);

    assert Arrays.equals(wirePayloadBytes(fromNorito), wirePayloadBytes(fromCanonicalLiteral))
        : "canonical norito asset ids must encode to the same transfer payload as canonical text";
  }

  private static void encodeAssetTransferAcceptsCompactLenNoritoAssetId() {
    final byte[] aidBytes = AssetDefinitionIdEncoder.computeDefinitionBytes("rose", "wonderland");
    final String definitionAddress = AssetDefinitionIdEncoder.encode("rose", "wonderland");
    final byte[] accountPayload = encodeSingleAccountPayload(ED25519_KEY, NoritoHeader.COMPACT_LEN);
    final byte[] scopePayload = encodeGlobalScopePayload(NoritoHeader.COMPACT_LEN);
    final String destinationAccountId = canonicalAccountIdForPublicKeyMultihash(ED25519_KEY);
    final String compactNoritoAssetId =
        encodeNoritoAssetId(accountPayload, aidBytes, scopePayload, NoritoHeader.COMPACT_LEN);

    final InstructionBox fromCompactNorito =
        TransferWirePayloadEncoder.encodeAssetTransfer(
            compactNoritoAssetId, "10", destinationAccountId);
    final InstructionBox fromCanonicalLiteral =
        TransferWirePayloadEncoder.encodeAssetTransfer(
            definitionAddress + "#" + destinationAccountId,
            "10",
            destinationAccountId);

    assert Arrays.equals(wirePayloadBytes(fromCompactNorito), wirePayloadBytes(fromCanonicalLiteral))
        : "compact-len norito asset ids must normalize to the canonical transfer payload";
  }

  private static void encodeAssetTransferAcceptsCompactLenNoritoDataspaceScope() {
    final byte[] aidBytes = AssetDefinitionIdEncoder.computeDefinitionBytes("rose", "wonderland");
    final byte[] compactAccountPayload = encodeSingleAccountPayload(ED25519_KEY, NoritoHeader.COMPACT_LEN);
    final byte[] compactScopePayload = encodeDataspaceScopePayload(42L, NoritoHeader.COMPACT_LEN);
    final String destinationAccountId = canonicalAccountIdForPublicKeyMultihash(ED25519_KEY);
    final String compactNoritoAssetId =
        encodeNoritoAssetId(
            compactAccountPayload, aidBytes, compactScopePayload, NoritoHeader.COMPACT_LEN);

    final byte[] canonicalAccountPayload = encodeSingleAccountPayload(ED25519_KEY, 0);
    final byte[] canonicalScopePayload = encodeDataspaceScopePayload(42L, 0);
    final String canonicalNoritoAssetId =
        encodeNoritoAssetId(canonicalAccountPayload, aidBytes, canonicalScopePayload, 0);

    final InstructionBox fromCompactNorito =
        TransferWirePayloadEncoder.encodeAssetTransfer(
            compactNoritoAssetId, "10", destinationAccountId);
    final InstructionBox fromCanonicalNorito =
        TransferWirePayloadEncoder.encodeAssetTransfer(
            canonicalNoritoAssetId, "10", destinationAccountId);

    assert Arrays.equals(wirePayloadBytes(fromCompactNorito), wirePayloadBytes(fromCanonicalNorito))
        : "compact-len dataspace scopes must normalize to canonical non-compact transfer payload bytes";
  }

  private static void encodeAssetTransferAcceptsCompactLenNoritoMultisigAssetId()
      throws Exception {
    final PublicKeyCodec.PublicKeyPayload baseKey =
        PublicKeyCodec.decodePublicKeyLiteral(ED25519_KEY);
    if (baseKey == null) {
      throw new AssertionError("expected valid ED25519 key fixture");
    }

    final String memberA =
        PublicKeyCodec.encodePublicKeyMultihash(baseKey.curveId(), filledKey((byte) 0x44));
    final String memberB =
        PublicKeyCodec.encodePublicKeyMultihash(baseKey.curveId(), filledKey((byte) 0x11));
    final byte[] aidBytes = AssetDefinitionIdEncoder.computeDefinitionBytes("rose", "wonderland");
    final String definitionAddress = AssetDefinitionIdEncoder.encode("rose", "wonderland");
    final String destinationAccountId = canonicalAccountIdForPublicKeyMultihash(ED25519_KEY);

    final byte[] compactAccountPayload =
        encodeMultisigAccountPayload(
            1,
            2,
            new String[] {memberA, memberB},
            new int[] {1, 1},
            NoritoHeader.COMPACT_LEN);
    final byte[] compactScopePayload = encodeGlobalScopePayload(NoritoHeader.COMPACT_LEN);
    final String compactNoritoAssetId =
        encodeNoritoAssetId(
            compactAccountPayload, aidBytes, compactScopePayload, NoritoHeader.COMPACT_LEN);

    final AccountAddress.MultisigPolicyPayload policy =
        AccountAddress.MultisigPolicyPayload.of(
            1,
            2,
            Arrays.asList(
                AccountAddress.MultisigMemberPayload.of(baseKey.curveId(), 1, filledKey((byte) 0x44)),
                AccountAddress.MultisigMemberPayload.of(baseKey.curveId(), 1, filledKey((byte) 0x11))));
    final String multisigAccountId =
        AccountAddress.fromMultisigPolicy(policy).toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);

    final InstructionBox fromCompactNorito =
        TransferWirePayloadEncoder.encodeAssetTransfer(
            compactNoritoAssetId, "10", destinationAccountId);
    final InstructionBox fromCanonicalLiteral =
        TransferWirePayloadEncoder.encodeAssetTransfer(
            definitionAddress + "#" + multisigAccountId,
            "10",
            destinationAccountId);

    assert Arrays.equals(wirePayloadBytes(fromCompactNorito), wirePayloadBytes(fromCanonicalLiteral))
        : "compact-len multisig account payloads must normalize to canonical transfer payload bytes";
  }

  private static void encodeAssetTransferRejectsMalformedNoritoAccountPayload() {
    final byte[] aidBytes = AssetDefinitionIdEncoder.computeDefinitionBytes("rose", "wonderland");
    final byte[] malformedAccountPayload = new byte[] {0x01};
    final byte[] scopePayload = encodeGlobalScopePayload(0);
    final String malformedNoritoAssetId =
        encodeNoritoAssetId(malformedAccountPayload, aidBytes, scopePayload, 0);

    boolean threw = false;
    try {
      TransferWirePayloadEncoder.encodeAssetTransfer(
          malformedNoritoAssetId, "10", canonicalAccountIdForPublicKeyMultihash(ED25519_KEY));
    } catch (final IllegalArgumentException ex) {
      final String message = ex.getMessage();
      threw = message != null && message.contains("Invalid AssetId.account payload");
    }

    assert threw : "malformed AssetId.account payload must be rejected";
  }

  private static void encodeAssetTransferRejectsMalformedNoritoScopePayload() {
    final byte[] aidBytes = AssetDefinitionIdEncoder.computeDefinitionBytes("rose", "wonderland");
    final byte[] accountPayload = encodeSingleAccountPayload(ED25519_KEY, 0);
    // Discriminant 1 (Dataspace) without the required variant payload.
    final byte[] malformedScopePayload = new byte[] {0x01, 0x00, 0x00, 0x00};
    final String malformedNoritoAssetId =
        encodeNoritoAssetId(accountPayload, aidBytes, malformedScopePayload, 0);

    boolean threw = false;
    try {
      TransferWirePayloadEncoder.encodeAssetTransfer(
          malformedNoritoAssetId, "10", canonicalAccountIdForPublicKeyMultihash(ED25519_KEY));
    } catch (final IllegalArgumentException ex) {
      final String message = ex.getMessage();
      threw = message != null && message.contains("Invalid AssetId.scope payload");
    }

    assert threw : "malformed AssetId.scope payload must be rejected";
  }

  private static void encodeAssetTransferRejectsInvalidNoritoMultisigSemantics() {
    final byte[] aidBytes = AssetDefinitionIdEncoder.computeDefinitionBytes("rose", "wonderland");
    final byte[] invalidMultisigAccountPayload =
        encodeMultisigAccountPayload(1, 5, ED25519_KEY, 1, 0);
    final byte[] scopePayload = encodeGlobalScopePayload(0);
    final String malformedNoritoAssetId =
        encodeNoritoAssetId(invalidMultisigAccountPayload, aidBytes, scopePayload, 0);

    boolean threw = false;
    try {
      TransferWirePayloadEncoder.encodeAssetTransfer(
          malformedNoritoAssetId, "10", canonicalAccountIdForPublicKeyMultihash(ED25519_KEY));
    } catch (final IllegalArgumentException ex) {
      final String message = ex.getMessage();
      threw = message != null && message.contains("Invalid AssetId.account payload");
    }

    assert threw : "invalid multisig policy semantics must be rejected";
  }

  private static void encodeAssetTransferRejectsUnsupportedNoritoMultisigVersion() {
    final byte[] aidBytes = AssetDefinitionIdEncoder.computeDefinitionBytes("rose", "wonderland");
    final byte[] invalidMultisigAccountPayload =
        encodeMultisigAccountPayload(2, 1, ED25519_KEY, 1, 0);
    final byte[] scopePayload = encodeGlobalScopePayload(0);
    final String malformedNoritoAssetId =
        encodeNoritoAssetId(invalidMultisigAccountPayload, aidBytes, scopePayload, 0);

    boolean threw = false;
    try {
      TransferWirePayloadEncoder.encodeAssetTransfer(
          malformedNoritoAssetId, "10", canonicalAccountIdForPublicKeyMultihash(ED25519_KEY));
    } catch (final IllegalArgumentException ex) {
      final String message = ex.getMessage();
      threw = message != null && message.contains("Invalid AssetId.account payload");
    }

    assert threw : "unsupported multisig policy version must be rejected";
  }

  private static void encodeAssetTransferRejectsUnsupportedNoritoAssetIdFlags() {
    final byte[] aidBytes = AssetDefinitionIdEncoder.computeDefinitionBytes("rose", "wonderland");
    final byte[] accountPayload = encodeSingleAccountPayload(ED25519_KEY, NoritoHeader.PACKED_STRUCT);
    final byte[] scopePayload = encodeGlobalScopePayload(NoritoHeader.PACKED_STRUCT);
    final String flaggedNoritoAssetId =
        encodeNoritoAssetId(
            accountPayload, aidBytes, scopePayload, NoritoHeader.PACKED_STRUCT);

    boolean threw = false;
    try {
      TransferWirePayloadEncoder.encodeAssetTransfer(
          flaggedNoritoAssetId, "10", canonicalAccountIdForPublicKeyMultihash(ED25519_KEY));
    } catch (final IllegalArgumentException ex) {
      final String message = ex.getMessage();
      threw =
          message != null
              && message.contains("Unsupported norito AssetId layout flags for transfer encoding");
    }

    assert threw : "unsupported Norito AssetId layout flags must be rejected with a clear error";
  }

  private static void encodeAssetTransferRejectsLegacyTextAssetId() {
    final String destinationAccountId = canonicalAccountIdForPublicKeyMultihash(ED25519_KEY);
    boolean threw = false;
    try {
      TransferWirePayloadEncoder.encodeAssetTransfer(
          "62Fk4FPcMuLvW5QjDGNF2a4jAmjM#" + destinationAccountId,
          "10",
          destinationAccountId);
    } catch (final IllegalArgumentException ex) {
      threw =
          ex.getMessage() != null
              && ex.getMessage().contains("<asset-definition-address>#<account-id>");
    }

    assert threw : "legacy text asset ids must be rejected";
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
    final byte[] definitionBytes =
        AssetDefinitionIdEncoder.computeDefinitionBytes("rose", "wonderland");
    final String definitionAddress = AssetDefinitionIdEncoder.encode("rose", "wonderland");
    final String i105AccountId;
    final String multihash;

    AccountAddress.configureCurveSupport(curveSupport);
    try {
      final AccountAddress address = AccountAddress.fromAccount(key, algorithm);
      i105AccountId = address.toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
      final AccountAddress.SingleKeyPayload payload =
          address.singleKeyPayload().orElseThrow(() -> new AssertionError("expected single-key address"));
      multihash = PublicKeyCodec.encodePublicKeyMultihash(payload.curveId(), payload.publicKey());
    } finally {
      AccountAddress.configureCurveSupport(AccountAddress.CurveSupportConfig.ed25519Only());
    }

    final String noritoAssetId =
        encodeNoritoAssetId(
            encodeSingleAccountPayload(multihash, 0), definitionBytes, encodeGlobalScopePayload(0), 0);
    final InstructionBox fromI105 =
        TransferWirePayloadEncoder.encodeAssetTransfer(
            definitionAddress + "#" + i105AccountId, "10", i105AccountId);
    final InstructionBox fromNorito =
        TransferWirePayloadEncoder.encodeAssetTransfer(noritoAssetId, "10", i105AccountId);

    assert Arrays.equals(wirePayloadBytes(fromI105), wirePayloadBytes(fromNorito))
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

  private static String canonicalAccountIdForPublicKeyMultihash(final String publicKeyMultihash) {
    final PublicKeyCodec.PublicKeyPayload payload =
        PublicKeyCodec.decodePublicKeyLiteral(publicKeyMultihash);
    if (payload == null) {
      throw new AssertionError("expected valid multihash fixture");
    }
    try {
      return AccountAddress.fromAccount(
              payload.keyBytes(), PublicKeyCodec.algorithmForCurveId(payload.curveId()))
          .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final Exception ex) {
      throw new IllegalStateException("failed to build canonical account fixture", ex);
    }
  }

  private static byte[] encodeSingleAccountPayload(final String publicKeyMultihash, final int flags) {
    final boolean compactLen = (flags & NoritoHeader.COMPACT_LEN) != 0;
    final NoritoEncoder accountEncoder = new NoritoEncoder(flags);
    UINT32_ADAPTER.encode(accountEncoder, 0L);
    final NoritoEncoder publicKeyEncoder = new NoritoEncoder(flags);
    STRING_ADAPTER.encode(publicKeyEncoder, publicKeyMultihash);
    final byte[] publicKeyPayload = publicKeyEncoder.toByteArray();
    accountEncoder.writeLength(publicKeyPayload.length, compactLen);
    accountEncoder.writeBytes(publicKeyPayload);
    return accountEncoder.toByteArray();
  }

  private static byte[] encodeGlobalScopePayload(final int flags) {
    final NoritoEncoder encoder = new NoritoEncoder(flags);
    UINT32_ADAPTER.encode(encoder, 0L);
    return encoder.toByteArray();
  }

  private static byte[] encodeDataspaceScopePayload(final long dataspaceId, final int flags) {
    final boolean compactLen = (flags & NoritoHeader.COMPACT_LEN) != 0;
    final NoritoEncoder encoder = new NoritoEncoder(flags);
    UINT32_ADAPTER.encode(encoder, 1L);
    encoder.writeLength(8L, compactLen);
    encoder.writeUInt(dataspaceId, 64);
    return encoder.toByteArray();
  }

  private static byte[] encodeMultisigAccountPayload(
      final int version,
      final int threshold,
      final String memberMultihash,
      final int memberWeight,
      final int flags) {
    return encodeMultisigAccountPayload(
        version, threshold, new String[] {memberMultihash}, new int[] {memberWeight}, flags);
  }

  private static byte[] encodeMultisigAccountPayload(
      final int version,
      final int threshold,
      final String[] memberMultihashes,
      final int[] memberWeights,
      final int flags) {
    if (memberMultihashes.length != memberWeights.length) {
      throw new IllegalArgumentException("memberMultihashes/memberWeights length mismatch");
    }
    final boolean compactLen = (flags & NoritoHeader.COMPACT_LEN) != 0;
    final NoritoEncoder accountEncoder = new NoritoEncoder(flags);
    UINT32_ADAPTER.encode(accountEncoder, 1L);

    final NoritoEncoder policyEncoder = new NoritoEncoder(flags);
    encodeSizedField(policyEncoder, UINT8_ADAPTER, (long) version);
    encodeSizedField(policyEncoder, UINT16_ADAPTER, (long) threshold);
    encodeMultisigMembers(policyEncoder, memberMultihashes, memberWeights, flags);

    final byte[] policyPayload = policyEncoder.toByteArray();
    accountEncoder.writeLength(policyPayload.length, compactLen);
    accountEncoder.writeBytes(policyPayload);
    return accountEncoder.toByteArray();
  }

  private static void encodeMultisigMembers(
      final NoritoEncoder encoder,
      final String[] memberMultihashes,
      final int[] memberWeights,
      final int flags) {
    if (memberMultihashes.length != memberWeights.length) {
      throw new IllegalArgumentException("memberMultihashes/memberWeights length mismatch");
    }
    final boolean compactLen = (flags & NoritoHeader.COMPACT_LEN) != 0;
    final NoritoEncoder vecEncoder = new NoritoEncoder(flags);
    vecEncoder.writeUInt(memberMultihashes.length, 64);

    for (int i = 0; i < memberMultihashes.length; i++) {
      final NoritoEncoder memberEncoder = new NoritoEncoder(flags);
      encodeSizedField(memberEncoder, STRING_ADAPTER, memberMultihashes[i]);
      encodeSizedField(memberEncoder, UINT16_ADAPTER, (long) memberWeights[i]);

      final byte[] memberPayload = memberEncoder.toByteArray();
      vecEncoder.writeLength(memberPayload.length, compactLen);
      vecEncoder.writeBytes(memberPayload);
    }

    final byte[] vecPayload = vecEncoder.toByteArray();
    encoder.writeLength(vecPayload.length, compactLen);
    encoder.writeBytes(vecPayload);
  }

  private static <T> void encodeSizedField(
      final NoritoEncoder encoder, final TypeAdapter<T> adapter, final T value) {
    final boolean compactLen = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
    final NoritoEncoder child = new NoritoEncoder(encoder.flags());
    adapter.encode(child, value);
    final byte[] payload = child.toByteArray();
    encoder.writeLength(payload.length, compactLen);
    encoder.writeBytes(payload);
  }

  private static String encodeNoritoAssetId(
      final byte[] accountPayload,
      final byte[] aidBytes,
      final byte[] scopePayload,
      final int flags) {
    final boolean compactLen = (flags & NoritoHeader.COMPACT_LEN) != 0;
    final NoritoEncoder encoder = new NoritoEncoder(flags);
    encoder.writeLength(accountPayload.length, compactLen);
    encoder.writeBytes(accountPayload);

    final NoritoEncoder aidEncoder = new NoritoEncoder(flags);
    TransferWirePayloadEncoder.encodeFixedByteArray(aidEncoder, aidBytes);
    final byte[] aidPayload = aidEncoder.toByteArray();
    encoder.writeLength(aidPayload.length, compactLen);
    encoder.writeBytes(aidPayload);

    encoder.writeLength(scopePayload.length, compactLen);
    encoder.writeBytes(scopePayload);

    final byte[] payload = encoder.toByteArray();
    final NoritoHeader header =
        new NoritoHeader(
            ASSET_ID_SCHEMA_HASH,
            payload.length,
            CRC64.compute(payload),
            flags,
            NoritoHeader.COMPRESSION_NONE);
    final byte[] headerBytes = header.encode();
    final byte[] full = new byte[headerBytes.length + payload.length];
    System.arraycopy(headerBytes, 0, full, 0, headerBytes.length);
    System.arraycopy(payload, 0, full, headerBytes.length, payload.length);
    return "norito:" + bytesToHex(full);
  }

  private static String bytesToHex(final byte[] bytes) {
    final StringBuilder sb = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      sb.append(String.format("%02x", b & 0xFF));
    }
    return sb.toString();
  }
}

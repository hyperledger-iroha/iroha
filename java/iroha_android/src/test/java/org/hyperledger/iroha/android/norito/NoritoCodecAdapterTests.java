package org.hyperledger.iroha.android.norito;

import java.util.Arrays;
import java.util.Base64;
import java.nio.charset.StandardCharsets;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.client.MultisigProposeRequest;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.KeyManagementException;
import org.hyperledger.iroha.android.model.Executable;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.instructions.TransferWirePayloadEncoder;
import org.hyperledger.iroha.android.model.JsonValue;
import org.hyperledger.iroha.android.testing.TestAssetDefinitionIds;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.crypto.Blake2b;
import org.hyperledger.iroha.android.tx.MultisigSignature;
import org.hyperledger.iroha.android.tx.MultisigSignatures;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.tx.SignedTransactionHasher;
import org.hyperledger.iroha.android.norito.SignedTransactionEncoder;
import org.hyperledger.iroha.android.tx.TransactionBuilder;
import org.hyperledger.iroha.android.SigningException;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoEncoder;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;
import org.junit.Test;

public final class NoritoCodecAdapterTests {

  private static final TypeAdapter<byte[]> BYTE_VECTOR_ADAPTER = NoritoAdapters.byteVecAdapter();
  private static final TypeAdapter<byte[]> RAW_BYTE_VECTOR_ADAPTER = NoritoAdapters.rawByteVecAdapter();
  private static final TypeAdapter<List<RawMetadataEntry>> RAW_METADATA_ADAPTER =
      NoritoAdapters.sequence(new RawMetadataEntryAdapter());

  @Test
  public void runCodecScenarios() throws NoritoException {
    runAll();
  }

  public static void main(final String[] args) throws NoritoException {
    runAll();
  }

  private static void runAll() throws NoritoException {
    javaCodecRoundTripsPayload();
    javaCodecEncodesAccountIdAuthority();
    javaCodecEncodesMultisigAuthority();
    javaCodecEncodesNativeMultisigProposeRequest();
    javaCodecEncodesMultisigSignatures();
    javaCodecRejectsMalformedSignedTransactions();
    javaCodecEncodesChainIdLayout();
    javaCodecSupportsInstructionsVariant();
    javaCodecSupportsWireInstructionPayloads();
    javaCodecEncodesIvmBytecodeLayout();
    javaCodecEncodesInstructionLayout();
    javaCodecEncodesTypedMetadata();
    System.out.println("[IrohaAndroid] Norito codec scaffolding tests passed.");
  }

  private static void javaCodecRoundTripsPayload() throws NoritoException {
    final byte[] instructions = "android-instructions".getBytes();
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000001")
            .setAuthority(sampleAuthority((byte) 0x01))
            .setCreationTimeMs(1_735_000_000_123L)
            .setExecutable(Executable.ivm(instructions))
            .setTimeToLiveMs(5_000L)
            .setNonce(42)
            .putMetadata("purpose", "unit-test")
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final byte[] encoded = adapter.encodeTransaction(payload);

    final TransactionPayload decoded = adapter.decodeTransaction(encoded);
    assert decoded.chainId().equals("00000001") : "Chain ID must round-trip";
    assert decoded.authority().equals(payload.authority()) : "Authority must round-trip";
    assert decoded.creationTimeMs() == 1_735_000_000_123L : "creation_time_ms must round-trip";
    assert Arrays.equals(instructions, decoded.executable().ivmBytes())
        : "Decoded payload should match original instructions";
    assert decoded.timeToLiveMs().orElseThrow() == 5_000L : "TTL must round-trip";
    assert decoded.nonce().orElseThrow() == 42 : "Nonce must round-trip";
    assert JsonValue.string("unit-test").equals(decoded.metadata().get("purpose"))
        : "Metadata must round-trip";
    assertBarePayload(encoded);
  }

  private static void javaCodecEncodesTypedMetadata() throws NoritoException {
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000014")
            .setAuthority(sampleAuthority((byte) 0x08))
            .setCreationTimeMs(1_735_333_333_123L)
            .setExecutable(Executable.ivm(new byte[] {0x05}))
            .putMetadata("gas_asset_id", "xor#universal")
            .putMetadata("gas_limit", JsonValue.number(1000L))
            .putMetadata("checked", JsonValue.bool(true))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final byte[] encoded = adapter.encodeTransaction(payload);
    final TransactionPayload decoded = adapter.decodeTransaction(encoded);

    assert JsonValue.string("xor#universal").equals(decoded.metadata().get("gas_asset_id"))
        : "gas_asset_id must remain a JSON string";
    assert JsonValue.number(1000L).equals(decoded.metadata().get("gas_limit"))
        : "gas_limit must round-trip as a JSON number";
    assert JsonValue.bool(true).equals(decoded.metadata().get("checked"))
        : "boolean metadata must round-trip";

    final Map<String, String> rawMetadata = rawMetadata(encoded);
    assert "1000".equals(rawMetadata.get("gas_limit")) : "gas_limit must be encoded without quotes";
    assert !"\"1000\"".equals(rawMetadata.get("gas_limit"))
        : "gas_limit must not be encoded as a JSON string";
  }

  private static void javaCodecEncodesAccountIdAuthority() throws NoritoException {
    final byte[] publicKey = new byte[32];
    Arrays.fill(publicKey, (byte) 0x3A);
    final String i105;
    try {
      i105 =
          AccountAddress.fromAccount(publicKey, "ed25519")
              .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalStateException("Failed to build authority address", ex);
    }
    final String authority = i105;
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000002")
            .setAuthority(authority)
            .setCreationTimeMs(1_735_000_000_456L)
            .setExecutable(Executable.ivm(new byte[] {0x01, 0x02, 0x03}))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final byte[] encoded = adapter.encodeTransaction(payload);
    final TransactionPayload decoded = adapter.decodeTransaction(encoded);

    assert authority.equals(decoded.authority()) : "AccountId authority must round-trip";
    final NoritoDecoder decoder = canonicalDecoder(encoded);
    readField(decoder, "payload.chain_id");
    final byte[] authorityField = readField(decoder, "payload.authority");
    final NoritoDecoder authorityDecoder = canonicalDecoder(authorityField);
    final long controllerTag = NoritoAdapters.uint(32).decode(authorityDecoder);
    assert controllerTag == 0L : "AccountController tag must be Single";
    final byte[] publicKeyField = readField(authorityDecoder, "authority.controller.public_key");
    final byte[] publicKeyPayload =
        decodeFieldPayload(publicKeyField, BYTE_VECTOR_ADAPTER, "authority.controller.public_key");
    assert Arrays.equals(PublicKeyCodec.compactPublicKeyPayload(0x01, publicKey), publicKeyPayload)
        : "Public key field must wrap the compact public-key payload";
    assert authorityDecoder.remaining() == 0 : "Authority payload must contain only the controller payload";
  }

  private static void javaCodecEncodesMultisigAuthority() throws NoritoException {
    final byte[] memberKeyA = new byte[32];
    final byte[] memberKeyB = new byte[32];
    Arrays.fill(memberKeyA, (byte) 0x11);
    Arrays.fill(memberKeyB, (byte) 0x22);
    final AccountAddress.MultisigMemberPayload memberA =
        AccountAddress.MultisigMemberPayload.of(0x01, 1, memberKeyA);
    final AccountAddress.MultisigMemberPayload memberB =
        AccountAddress.MultisigMemberPayload.of(0x01, 2, memberKeyB);
    final AccountAddress.MultisigPolicyPayload policy =
        AccountAddress.MultisigPolicyPayload.of(1, 2, listOf(memberA, memberB));
    final String i105;
    try {
      i105 =
          AccountAddress.fromMultisigPolicy(policy)
              .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalStateException("Failed to build multisig authority address", ex);
    }
    final String authority = i105;
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000002")
            .setAuthority(authority)
            .setCreationTimeMs(1_735_000_000_456L)
            .setExecutable(Executable.ivm(new byte[] {0x04, 0x05, 0x06}))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final byte[] encoded = adapter.encodeTransaction(payload);
    final TransactionPayload decoded = adapter.decodeTransaction(encoded);

    assert authority.equals(decoded.authority()) : "Multisig authority must round-trip";

    final NoritoDecoder decoder = canonicalDecoder(encoded);
    readField(decoder, "payload.chain_id");
    final byte[] authorityField = readField(decoder, "payload.authority");
    final NoritoDecoder authorityDecoder = canonicalDecoder(authorityField);
    final long controllerTag = NoritoAdapters.uint(32).decode(authorityDecoder);
    assert controllerTag == 1L : "AccountController tag must be Multisig";
    final byte[] policyField = readField(authorityDecoder, "authority.controller.policy");
    assert authorityDecoder.remaining() == 0 : "Authority payload must contain only the controller payload";

    final NoritoDecoder policyDecoder = canonicalDecoder(policyField);
    final int version =
        Math.toIntExact(
            decodeFieldPayload(
                readField(policyDecoder, "authority.controller.policy.version"),
                NoritoAdapters.uint(8),
                "authority.controller.policy.version"));
    final int threshold =
        Math.toIntExact(
            decodeFieldPayload(
                readField(policyDecoder, "authority.controller.policy.threshold"),
                NoritoAdapters.uint(16),
                "authority.controller.policy.threshold"));
    assert version == 1 : "Multisig policy version must round-trip";
    assert threshold == 2 : "Multisig policy threshold must round-trip";
    final byte[] membersField = readField(policyDecoder, "authority.controller.policy.members");
    assert policyDecoder.remaining() == 0 : "Multisig policy payload must contain only the policy fields";
    final NoritoDecoder memberListDecoder = canonicalDecoder(membersField);
    final long memberCount =
        memberListDecoder.readLength(false);
    assert memberCount == 2L : "Multisig policy member count must round-trip";

    final byte[] expectedMemberA =
        PublicKeyCodec.compactPublicKeyPayload(0x01, memberKeyA);
    final byte[] expectedMemberB =
        PublicKeyCodec.compactPublicKeyPayload(0x01, memberKeyB);

    assertMultisigMember(memberListDecoder, expectedMemberA, 1, "member[0]");
    assertMultisigMember(memberListDecoder, expectedMemberB, 2, "member[1]");
    assert memberListDecoder.remaining() == 0 : "Multisig member list must contain only members";
  }

  private static void javaCodecEncodesNativeMultisigProposeRequest() throws NoritoException {
    final byte[] memberKey = fill(0x11, 32);
    final AccountAddress.MultisigMemberPayload member =
        AccountAddress.MultisigMemberPayload.of(0x01, 1, memberKey);
    final AccountAddress.MultisigPolicyPayload policy =
        AccountAddress.MultisigPolicyPayload.of(1, 1, listOf(member));
    final String multisigAccountId;
    try {
      multisigAccountId =
          AccountAddress.fromMultisigPolicy(policy)
              .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalStateException("Failed to build native multisig account address", ex);
    }
    final String signerAccountId = sampleAuthority((byte) 0x22);
    final String destinationAccountId = sampleAuthority((byte) 0x33);
    final InstructionBox transfer =
        TransferWirePayloadEncoder.encodeAssetTransfer(
            TestAssetDefinitionIds.PRIMARY + "#" + multisigAccountId + "#dataspace:1",
            "5.00",
            destinationAccountId);
    final byte[] encodedTransfer = NoritoJavaCodecAdapter.encodeInstructionBox(transfer);
    final MultisigProposeRequest request =
        MultisigProposeRequest.builder()
            .setMultisigAccountId(multisigAccountId)
            .setSignerAccountId(signerAccountId)
            .addInstructionBytes(encodedTransfer)
            .setCreationTimeMs(1_735_444_555_123L)
            .setPublicKeyHex("deadbeef")
            .setSignatureB64("c2ln")
            .setFeeSponsor("sponsor@pob.cbsi")
            .setMemo("QR invoice 42")
            .build();

    final byte[] encoded = NoritoJavaCodecAdapter.encodeMultisigProposeRequest(request);
    if (Boolean.getBoolean("iroha.android.emitMultisigProposeFixture")
        || "1".equals(System.getenv("IROHA_ANDROID_EMIT_MULTISIG_PROPOSE_FIXTURE"))) {
      System.out.println("[Fixture] native_multisig_propose_hex=" + bytesToHex(encoded));
    }
    final NoritoCodec.ArchiveView view =
        NoritoCodec.fromBytesView(encoded, "iroha_torii::routing::MultisigProposeDto");
    final NoritoDecoder decoder = new NoritoDecoder(view.asBytes(), view.flags(), view.flagsHint());

    final byte[] multisigAccountField = readField(decoder, "request.multisig_account_id");
    final byte[] multisigAccountPayload =
        decodeOptionPayload(multisigAccountField, "request.multisig_account_id")
            .orElseThrow(() -> new IllegalStateException("multisig account id missing"));
    assertNativeMultisigAccountPayload(multisigAccountPayload, memberKey, 1, 1, "request.multisig_account_id");
    assertOptionPayloadEmpty(readField(decoder, "request.multisig_account_alias"), "request.multisig_account_alias");

    final byte[] signerPayload = readField(decoder, "request.signer_account_id");
    final NoritoDecoder signerDecoder = canonicalDecoder(signerPayload);
    final long signerTag = NoritoAdapters.uint(32).decode(signerDecoder);
    assert signerTag == 0L : "Signer account id must be single-key";
    readField(signerDecoder, "request.signer_account_id.public_key");
    assert signerDecoder.remaining() == 0 : "Signer account payload has trailing bytes";

    assertOptionPayloadEmpty(readField(decoder, "request.private_key"), "request.private_key");
    assert decodeOptionPayload(readField(decoder, "request.public_key_hex"), "request.public_key_hex").isPresent()
        : "public key hex must be present";
    assert decodeOptionPayload(readField(decoder, "request.signature_b64"), "request.signature_b64").isPresent()
        : "signature must be present";
    assert decodeOptionPayload(readField(decoder, "request.creation_time_ms"), "request.creation_time_ms").isPresent()
        : "creation time must be present";
    assert decodeOptionPayload(readField(decoder, "request.fee_sponsor"), "request.fee_sponsor").isPresent()
        : "fee sponsor must be present";
    assert decodeOptionPayload(readField(decoder, "request.memo"), "request.memo").isPresent()
        : "memo must be present";

    final byte[] instructionsField = readField(decoder, "request.instructions");
    final NoritoDecoder instructionsDecoder = canonicalDecoder(instructionsField);
    final long instructionCount = instructionsDecoder.readLength(false);
    assert instructionCount == 1L : "request must include one transfer instruction";
    final byte[] instructionPayload =
        readSequenceElement(instructionsDecoder, instructionsDecoder.compactLenActive(), "request.instructions[0]");
    assert instructionPayload.length > 0 : "encoded instruction must not be empty";
    assert instructionsDecoder.remaining() == 0 : "instruction list has trailing bytes";
    assert decoder.remaining() == 0 : "multisig propose request has trailing bytes";
  }

  private static void javaCodecEncodesMultisigSignatures() throws NoritoException {
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000003")
            .setAuthority(sampleAuthority((byte) 0x02))
            .setCreationTimeMs(1_735_000_000_789L)
            .setExecutable(Executable.ivm(new byte[] {0x0A, 0x0B}))
            .build();
    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final byte[] encodedPayload = adapter.encodeTransaction(payload);
    final byte[] signature = new byte[64];
    final byte[] publicKey = new byte[32];
    Arrays.fill(signature, (byte) 0x44);
    Arrays.fill(publicKey, (byte) 0x55);

    final MultisigSignature sigA =
        MultisigSignature.fromCurveId(0x01, fill(0x11, 32), fill(0x22, 64));
    final String sigBKeyLiteral = PublicKeyCodec.encodePublicKeyMultihash(0x01, fill(0x33, 32));
    final MultisigSignature sigB =
        MultisigSignature.fromPublicKeyLiteral(sigBKeyLiteral, fill(0x44, 64));
    assert sigBKeyLiteral.equals(sigB.publicKeyMultihash())
        : "Multisig public key literal must round-trip";
    final MultisigSignatures multisig = MultisigSignatures.of(List.of(sigA, sigB));

    final SignedTransaction signed =
        new SignedTransaction(encodedPayload, signature, publicKey, adapter.schemaName());
    final SignedTransaction withMultisig =
        signed.toBuilder().setMultisigSignatures(multisig).build();
    final byte[] encodedSigned = SignedTransactionEncoder.encode(withMultisig);
    final SignedTransaction decodedSigned = SignedTransactionEncoder.decode(encodedSigned);
    assert Arrays.equals(encodedPayload, decodedSigned.encodedPayload())
        : "Decoded signed payload must match";
    assert Arrays.equals(signature, decodedSigned.signature()) : "Decoded signature must match";
    final MultisigSignatures decodedMultisig =
        decodedSigned
            .multisigSignatures()
            .orElseThrow(() -> new IllegalStateException("Decoded multisig bundle missing"));
    assert decodedMultisig.signatures().size() == 2 : "Decoded multisig count must match";
    assert Arrays.equals(sigA.publicKey(), decodedMultisig.signatures().get(0).publicKey())
        : "Decoded first multisig public key must match";
    assert Arrays.equals(sigB.signature(), decodedMultisig.signatures().get(1).signature())
        : "Decoded second multisig signature must match";
    assert Arrays.equals(encodedSigned, SignedTransactionEncoder.encode(decodedSigned))
        : "Decoded signed transaction must re-encode canonically";

    final byte[] versioned = SignedTransactionEncoder.encodeVersioned(withMultisig);
    final SignedTransaction decodedVersioned = SignedTransactionEncoder.decodeVersioned(versioned);
    assert Arrays.equals(encodedPayload, decodedVersioned.encodedPayload())
        : "Decoded versioned signed payload must match";
    assert Arrays.equals(signature, decodedVersioned.signature())
        : "Decoded versioned signature must match";

    final NoritoDecoder decoder = canonicalDecoder(encodedSigned);
    readField(decoder, "signed.signature");
    readField(decoder, "signed.payload");
    final byte[] attachmentsField = readField(decoder, "signed.attachments");
    final byte[] multisigField = readField(decoder, "signed.multisig_signatures");
    assertOptionPayloadEmpty(attachmentsField, "signed.attachments");
    final byte[] multisigPayload =
        decodeOptionPayload(multisigField, "signed.multisig_signatures")
            .orElseThrow(() -> new IllegalStateException("multisig payload missing"));
    assert decoder.remaining() == 0 : "Signed transaction payload should not have trailing bytes";

    final NoritoDecoder multisigDecoder =
        canonicalDecoder(multisigPayload);
    final long count = multisigDecoder.readLength(false);
    assert count == 2 : "Expected two multisig signatures";
    final boolean compact = multisigDecoder.compactLenActive();
    final byte[] firstPayload = readSequenceElement(multisigDecoder, compact, "multisig[0]");
    final NoritoDecoder firstDecoder = canonicalDecoder(firstPayload);
    assertMultisigSignaturePayload(firstDecoder, sigA, "multisig[0]");
    assert firstDecoder.remaining() == 0 : "multisig[0] payload should not have trailing bytes";

    final byte[] secondPayload = readSequenceElement(multisigDecoder, compact, "multisig[1]");
    final NoritoDecoder secondDecoder = canonicalDecoder(secondPayload);
    assertMultisigSignaturePayload(secondDecoder, sigB, "multisig[1]");
    assert secondDecoder.remaining() == 0 : "multisig[1] payload should not have trailing bytes";
    assert multisigDecoder.remaining() == 0 : "Multisig payload should not have trailing bytes";
  }

  private static void javaCodecRejectsMalformedSignedTransactions() throws NoritoException {
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000003")
            .setAuthority(sampleAuthority((byte) 0x12))
            .setCreationTimeMs(1_735_000_001_000L)
            .setExecutable(Executable.ivm(new byte[] {0x01}))
            .build();
    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final SignedTransaction signed =
        new SignedTransaction(
            adapter.encodeTransaction(payload),
            fill(0x66, 64),
            new byte[0],
            adapter.schemaName());
    final byte[] encoded = SignedTransactionEncoder.encode(signed);

    expectNoritoFailure(() -> SignedTransactionEncoder.decode(new byte[0]));
    expectNoritoFailure(() -> SignedTransactionEncoder.decode(Arrays.copyOf(encoded, 12)));
    final byte[] mutated = Arrays.copyOf(encoded, encoded.length);
    mutated[mutated.length - 1] ^= 0x01;
    expectNoritoFailure(() -> SignedTransactionEncoder.decode(mutated));
    expectNoritoFailure(() -> SignedTransactionEncoder.decodeVersioned(new byte[0]));
    expectNoritoFailure(() -> SignedTransactionEncoder.decodeVersioned(new byte[] {0x02}));
  }

  private static void javaCodecEncodesChainIdLayout() throws NoritoException {
    final String chainId = "00000003";
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId(chainId)
            .setAuthority(sampleAuthority((byte) 0x03))
            .setCreationTimeMs(1_735_000_000_789L)
            .setExecutable(Executable.ivm(new byte[] {0x01}))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final byte[] encoded = adapter.encodeTransaction(payload);
    final NoritoDecoder decoder = canonicalDecoder(encoded);
    final byte[] chainField = readField(decoder, "payload.chain_id");
    final NoritoDecoder chainDecoder = canonicalDecoder(chainField);
    final byte[] stringField = readField(chainDecoder, "payload.chain_id.string");
    assert chainDecoder.remaining() == 0 : "ChainId must wrap a single string";
    final String decodedChain =
        decodeFieldPayload(stringField, NoritoAdapters.stringAdapter(), "payload.chain_id.string");
    assert chainId.equals(decodedChain) : "ChainId must round-trip via layout inspection";
  }

  private static void javaCodecSupportsInstructionsVariant() throws NoritoException {
    final byte[] wirePayloadA =
        NoritoCodec.encode("wire-A", "iroha.test.WirePayload", NoritoAdapters.stringAdapter());
    final byte[] wirePayloadB =
        NoritoCodec.encode("wire-B", "iroha.test.WirePayload", NoritoAdapters.stringAdapter());
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000009")
            .setAuthority(sampleAuthority((byte) 0x04))
            .setCreationTimeMs(1_735_111_111_000L)
            .setExecutable(
                Executable.instructions(
                    listOf(
                        InstructionBox.fromWirePayload("iroha.custom.a", wirePayloadA),
                        InstructionBox.fromWirePayload("iroha.custom.b", wirePayloadB))))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final byte[] encoded = adapter.encodeTransaction(payload);
    final TransactionPayload decoded = adapter.decodeTransaction(encoded);

    assert decoded.executable().isInstructions() : "Executable should decode as instructions";
    assert decoded.executable().instructions().size() == payload.executable().instructions().size()
        : "Instruction count must match";
    final List<InstructionBox> decodedInstructions = decoded.executable().instructions();
    assert decodedInstructions.size() == 2 : "Instruction count must match";

    final InstructionBox first = decodedInstructions.get(0);
    assert first.payload() instanceof InstructionBox.WirePayload
        : "First instruction must be wire payload";
    final InstructionBox.WirePayload decodedFirst = (InstructionBox.WirePayload) first.payload();
    assert "iroha.custom.a".equals(decodedFirst.wireName()) : "Wire name must round-trip";
    assert Arrays.equals(wirePayloadA, decodedFirst.payloadBytes())
        : "Wire payload bytes must round-trip";

    final InstructionBox second = decodedInstructions.get(1);
    assert second.payload() instanceof InstructionBox.WirePayload
        : "Second instruction must be wire payload";
    final InstructionBox.WirePayload decodedSecond = (InstructionBox.WirePayload) second.payload();
    assert "iroha.custom.b".equals(decodedSecond.wireName()) : "Wire name must round-trip";
    assert Arrays.equals(wirePayloadB, decodedSecond.payloadBytes())
        : "Wire payload bytes must round-trip";
  }

  private static void javaCodecSupportsWireInstructionPayloads() throws NoritoException {
    final byte[] wirePayload =
        NoritoCodec.encode("wire-payload", "iroha.test.WirePayload", NoritoAdapters.stringAdapter());
    final InstructionBox wireInstruction =
        InstructionBox.fromWirePayload("iroha.custom", wirePayload);

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000011")
            .setAuthority(sampleAuthority((byte) 0x05))
            .setCreationTimeMs(1_735_111_111_123L)
            .setExecutable(Executable.instructions(listOf(wireInstruction)))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final byte[] encoded = adapter.encodeTransaction(payload);
    final TransactionPayload decoded = adapter.decodeTransaction(encoded);

    final InstructionBox decodedBox = decoded.executable().instructions().get(0);
    assert decodedBox.payload() instanceof InstructionBox.WirePayload
        : "Wire payload instructions must decode to wire payloads";
    final InstructionBox.WirePayload decodedWire = (InstructionBox.WirePayload) decodedBox.payload();
    assert "iroha.custom".equals(decodedWire.wireName()) : "Wire name must round-trip";
    assert Arrays.equals(wirePayload, decodedWire.payloadBytes())
        : "Wire payload bytes must round-trip";
  }

  private static void javaCodecEncodesIvmBytecodeLayout() throws NoritoException {
    final byte[] ivmBytes = new byte[] {0x01, 0x02, 0x03, 0x04};
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000012")
            .setAuthority(sampleAuthority((byte) 0x06))
            .setCreationTimeMs(1_735_222_222_123L)
            .setExecutable(Executable.ivm(ivmBytes))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final byte[] encoded = adapter.encodeTransaction(payload);

    final NoritoDecoder decoder = canonicalDecoder(encoded);
    readField(decoder, "payload.chain_id");
    readField(decoder, "payload.authority");
    readField(decoder, "payload.creation_time_ms");
    final byte[] executableField = readField(decoder, "payload.executable");
    readField(decoder, "payload.time_to_live_ms");
    readField(decoder, "payload.nonce");
    readField(decoder, "payload.metadata");
    assert decoder.remaining() == 0 : "Payload has trailing bytes";

    final NoritoDecoder execDecoder = canonicalDecoder(executableField);
    final TypeAdapter<Long> uint32 = NoritoAdapters.uint(32);
    final long tag = uint32.decode(execDecoder);
    assert tag == 2L : "Executable should be Ivm";
    final byte[] ivmField = readField(execDecoder, "payload.executable.ivm");
    assert execDecoder.remaining() == 0 : "Executable has trailing bytes";

    final NoritoDecoder ivmDecoder = canonicalDecoder(ivmField);
    final byte[] ivmPayload = readField(ivmDecoder, "payload.executable.ivm.bytes");
    assert ivmDecoder.remaining() == 0 : "IVM bytecode must be sized";
    final byte[] decodedIvm =
        decodeFieldPayload(ivmPayload, RAW_BYTE_VECTOR_ADAPTER, "payload.executable.ivm.bytes");
    assert Arrays.equals(ivmBytes, decodedIvm) : "IVM bytecode bytes should match";
  }

  private static void javaCodecEncodesInstructionLayout() throws NoritoException {
    final byte[] wirePayload =
        NoritoCodec.encode("layout", "iroha.test.Layout", NoritoAdapters.stringAdapter());
    final InstructionBox wireInstruction =
        InstructionBox.fromWirePayload("iroha.custom.layout", wirePayload);
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000013")
            .setAuthority(sampleAuthority((byte) 0x07))
            .setCreationTimeMs(1_735_222_333_123L)
            .setExecutable(Executable.instructions(listOf(wireInstruction)))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final byte[] encoded = adapter.encodeTransaction(payload);

    final NoritoDecoder decoder = canonicalDecoder(encoded);
    readField(decoder, "payload.chain_id");
    readField(decoder, "payload.authority");
    readField(decoder, "payload.creation_time_ms");
    final byte[] executableField = readField(decoder, "payload.executable");
    readField(decoder, "payload.time_to_live_ms");
    readField(decoder, "payload.nonce");
    readField(decoder, "payload.metadata");
    assert decoder.remaining() == 0 : "Payload has trailing bytes";

    final NoritoDecoder execDecoder = canonicalDecoder(executableField);
    final TypeAdapter<Long> uint32 = NoritoAdapters.uint(32);
    final long tag = uint32.decode(execDecoder);
    assert tag == 0L : "Executable should be Instructions";
    final byte[] instructionsField = readField(execDecoder, "payload.executable.instructions");
    assert execDecoder.remaining() == 0 : "Executable has trailing bytes";

    final NoritoDecoder listDecoder = canonicalDecoder(instructionsField);
    final long count = listDecoder.readLength(false);
    assert count == 1L : "Instruction list should contain one element";
    final long elementLength = listDecoder.readLength(listDecoder.compactLenActive());
    assert elementLength > 0 : "Instruction element must not be empty";
    if (elementLength > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("Instruction element too large");
    }
    final byte[] elementPayload = listDecoder.readBytes((int) elementLength);
    assert listDecoder.remaining() == 0 : "Instruction list has trailing bytes";

    final NoritoDecoder elementDecoder = canonicalDecoder(elementPayload);
    final byte[] nameField = readField(elementDecoder, "instruction.name");
    final byte[] payloadField = readField(elementDecoder, "instruction.payload");
    assert elementDecoder.remaining() == 0 : "Instruction element has trailing bytes";
    final String decodedName =
        decodeFieldPayload(nameField, NoritoAdapters.stringAdapter(), "instruction.name");
    final byte[] decodedPayload =
        decodeFieldPayload(payloadField, RAW_BYTE_VECTOR_ADAPTER, "instruction.payload");
    assert "iroha.custom.layout".equals(decodedName) : "Instruction name must match wire payload";
    assert Arrays.equals(wirePayload, decodedPayload) : "Instruction payload must match wire bytes";
  }


  private static byte[] readField(final NoritoDecoder decoder, final String field) {
    final long length = decoder.readLength(decoder.compactLenActive());
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(field + " length too large: " + length);
    }
    return decoder.readBytes((int) length);
  }

  private static <T> void encodeFieldPayload(
      final NoritoEncoder encoder, final TypeAdapter<T> adapter, final T value) {
    final NoritoEncoder fieldEncoder = encoder.childEncoder();
    adapter.encode(fieldEncoder, value);
    final byte[] payload = fieldEncoder.toByteArray();
    final boolean compact = (encoder.flags() & NoritoHeader.COMPACT_LEN) != 0;
    encoder.writeLength(payload.length, compact);
    encoder.writeBytes(payload);
  }

  private static Map<String, String> rawMetadata(final byte[] encoded) {
    final NoritoDecoder payloadDecoder = canonicalDecoder(encoded);
    readField(payloadDecoder, "payload.chain_id");
    readField(payloadDecoder, "payload.authority");
    readField(payloadDecoder, "payload.creation_time_ms");
    readField(payloadDecoder, "payload.executable");
    readField(payloadDecoder, "payload.time_to_live_ms");
    readField(payloadDecoder, "payload.nonce");
    final byte[] metadataField = readField(payloadDecoder, "payload.metadata");
    assert payloadDecoder.remaining() == 0 : "Payload has trailing bytes";

    final NoritoDecoder metadataDecoder = canonicalDecoder(metadataField);
    final List<RawMetadataEntry> entries = RAW_METADATA_ADAPTER.decode(metadataDecoder);
    assert metadataDecoder.remaining() == 0 : "Metadata field has trailing bytes";
    final Map<String, String> values = new LinkedHashMap<>();
    for (final RawMetadataEntry entry : entries) {
      values.put(entry.key(), entry.value());
    }
    return values;
  }

  private static final class RawMetadataEntry {
    private final String key;
    private final String value;

    private RawMetadataEntry(final String key, final String value) {
      this.key = key;
      this.value = value;
    }

    private String key() {
      return key;
    }

    private String value() {
      return value;
    }
  }

  private static final class RawMetadataEntryAdapter implements TypeAdapter<RawMetadataEntry> {
    private static final TypeAdapter<String> RAW_JSON_ADAPTER = new RawJsonAdapter();

    @Override
    public void encode(final NoritoEncoder encoder, final RawMetadataEntry value) {
      encodeFieldPayload(encoder, NoritoAdapters.stringAdapter(), value.key());
      encodeFieldPayload(encoder, RAW_JSON_ADAPTER, value.value());
    }

    @Override
    public RawMetadataEntry decode(final NoritoDecoder decoder) {
      final String key =
          decodeFieldPayload(readField(decoder, "metadata.key"), NoritoAdapters.stringAdapter(), "metadata.key");
      final String value =
          decodeFieldPayload(readField(decoder, "metadata.value"), RAW_JSON_ADAPTER, "metadata.value");
      return new RawMetadataEntry(key, value);
    }

    @Override
    public boolean isSelfDelimiting() {
      return true;
    }
  }

  private static final class RawJsonAdapter implements TypeAdapter<String> {
    @Override
    public void encode(final NoritoEncoder encoder, final String value) {
      encodeFieldPayload(encoder, NoritoAdapters.stringAdapter(), value);
    }

    @Override
    public String decode(final NoritoDecoder decoder) {
      return decodeFieldPayload(
          readField(decoder, "metadata.value.json"),
          NoritoAdapters.stringAdapter(),
          "metadata.value.json");
    }

    @Override
    public boolean isSelfDelimiting() {
      return true;
    }
  }

  private static byte[] readSequenceElement(
      final NoritoDecoder decoder, final boolean compact, final String field) {
    final long length = decoder.readLength(compact);
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(field + " length too large: " + length);
    }
    return decoder.readBytes((int) length);
  }

  private static byte[] unwrapSizedField(final byte[] payload, final String field) {
    final long innerLen = readU64(payload, 0, field);
    if (innerLen > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(field + " length too large: " + innerLen);
    }
    final int end = Math.toIntExact(8 + innerLen);
    if (payload.length < end) {
      throw new IllegalArgumentException(field + " payload truncated");
    }
    return Arrays.copyOfRange(payload, 8, end);
  }

  private static byte[] assemblePayload(final int flags, final byte[]... fields) {
    final NoritoEncoder encoder = new NoritoEncoder(flags);
    final boolean compact = (flags & NoritoHeader.COMPACT_LEN) != 0;
    for (final byte[] field : fields) {
      encoder.writeLength(field.length, compact);
      encoder.writeBytes(field);
    }
    return encoder.toByteArray();
  }

  private static NoritoDecoder canonicalDecoder(final byte[] payload) {
    return new NoritoDecoder(payload, NoritoCodec.DEFAULT_FLAGS, NoritoHeader.MINOR_VERSION);
  }

  private static long readU64(final byte[] payload, final int offset, final String field) {
    if (offset < 0 || payload.length - offset < 8) {
      throw new IllegalArgumentException(field + " missing u64 payload");
    }
    long value = 0L;
    for (int i = 0; i < 8; i++) {
      value |= ((long) payload[offset + i] & 0xFFL) << (8 * i);
    }
    return value;
  }

  private static long readU32(final byte[] payload, final int offset, final String field) {
    if (offset < 0 || payload.length - offset < 4) {
      throw new IllegalArgumentException(field + " missing u32 payload");
    }
    long value = 0L;
    for (int i = 0; i < 4; i++) {
      value |= ((long) payload[offset + i] & 0xFFL) << (8 * i);
    }
    return value;
  }

  private static int readU16(final byte[] payload, final int offset, final String field) {
    if (offset < 0 || payload.length - offset < 2) {
      throw new IllegalArgumentException(field + " missing u16 payload");
    }
    return (payload[offset] & 0xFF) | ((payload[offset + 1] & 0xFF) << 8);
  }

  private static void assertMultisigMember(
      final NoritoDecoder decoder,
      final byte[] expectedPublicKey,
      final int expectedWeight,
      final String label) {
    final byte[] memberPayload = readSequenceElement(decoder, decoder.compactLenActive(), label);
    final NoritoDecoder memberDecoder = canonicalDecoder(memberPayload);
    final byte[] publicKey =
        decodeFieldPayload(
            readField(memberDecoder, label + ".public_key"),
            BYTE_VECTOR_ADAPTER,
            label + ".public_key");
    final int weight =
        Math.toIntExact(
            decodeFieldPayload(
                readField(memberDecoder, label + ".weight"),
                NoritoAdapters.uint(16),
                label + ".weight"));
    assert memberDecoder.remaining() == 0 : label + " payload should not have trailing bytes";
    assert Arrays.equals(expectedPublicKey, publicKey) : label + " public key must round-trip";
    assert weight == expectedWeight : label + " weight must round-trip";
  }

  private static void assertNativeMultisigAccountPayload(
      final byte[] accountPayload,
      final byte[] expectedMemberKey,
      final int expectedThreshold,
      final int expectedWeight,
      final String label) {
    final NoritoDecoder accountDecoder = canonicalDecoder(accountPayload);
    final long controllerTag = NoritoAdapters.uint(32).decode(accountDecoder);
    assert controllerTag == 1L : label + " must use the multisig AccountController tag";
    final byte[] policyField = readField(accountDecoder, label + ".policy");
    assert accountDecoder.remaining() == 0 : label + " account payload has trailing bytes";

    final NoritoDecoder policyDecoder = canonicalDecoder(policyField);
    final int version =
        Math.toIntExact(
            decodeFieldPayload(
                readField(policyDecoder, label + ".policy.version"),
                NoritoAdapters.uint(8),
                label + ".policy.version"));
    final int threshold =
        Math.toIntExact(
            decodeFieldPayload(
                readField(policyDecoder, label + ".policy.threshold"),
                NoritoAdapters.uint(16),
                label + ".policy.threshold"));
    assert version == 1 : label + " policy version must be current";
    assert threshold == expectedThreshold : label + " policy threshold mismatch";
    final byte[] membersField = readField(policyDecoder, label + ".policy.members");
    assert policyDecoder.remaining() == 0 : label + " policy payload has trailing bytes";

    final NoritoDecoder membersDecoder = canonicalDecoder(membersField);
    final long memberCount = membersDecoder.readLength(false);
    assert memberCount == 1L : label + " policy must contain one member";
    assertMultisigMember(
        membersDecoder,
        PublicKeyCodec.compactPublicKeyPayload(0x01, expectedMemberKey),
        expectedWeight,
        label + ".policy.members[0]");
    assert membersDecoder.remaining() == 0 : label + " member list has trailing bytes";
  }

  private static <T> T decodeFieldPayload(
      final byte[] payload, final TypeAdapter<T> adapter, final String field) {
    final NoritoDecoder decoder = canonicalDecoder(payload);
    final T value = adapter.decode(decoder);
    if (decoder.remaining() != 0) {
      throw new IllegalArgumentException(field + ": trailing bytes after field payload");
    }
    return value;
  }

  private static byte[] emitFixtureMetadata(
      final String fixtureName,
      final TransactionPayload payload,
      final NoritoJavaCodecAdapter adapter,
      final String keyAlias)
      throws NoritoException {
    final byte[] encodedBytes = adapter.encodeTransaction(payload);
    final String payloadBase64 = Base64.getEncoder().encodeToString(encodedBytes);
    final String payloadHashHex = bytesToHex(Blake2b.digest(encodedBytes));
    final SignedTransaction signedTransaction;
    try {
      final IrohaKeyManager keyManager = IrohaKeyManager.withSoftwareProvider();
      final TransactionBuilder builderHelper = new TransactionBuilder(adapter, keyManager);
      signedTransaction =
          builderHelper.encodeAndSign(
              payload, keyAlias, IrohaKeyManager.KeySecurityPreference.SOFTWARE_ONLY);
    } catch (final KeyManagementException | SigningException ex) {
      throw new RuntimeException("Failed to sign " + fixtureName + " fixture", ex);
    }
    final byte[] canonicalSigned = SignedTransactionEncoder.encode(signedTransaction);
    final String signedBase64 = Base64.getEncoder().encodeToString(canonicalSigned);
    final String signedHashHex = SignedTransactionHasher.hashHex(signedTransaction);

    System.out.println("[Fixture] " + fixtureName + ".payload_base64=" + payloadBase64);
    System.out.println("[Fixture] " + fixtureName + ".payload_hash=" + payloadHashHex);
    System.out.println("[Fixture] " + fixtureName + ".signed_base64=" + signedBase64);
    System.out.println("[Fixture] " + fixtureName + ".signed_hash=" + signedHashHex);
    System.out.println("[Fixture] " + fixtureName + ".signed_len=" + canonicalSigned.length);
    return encodedBytes;
  }

  private static String bytesToHex(final byte[] data) {
    final StringBuilder builder = new StringBuilder(data.length * 2);
    for (final byte b : data) {
      builder.append(String.format("%02x", b));
    }
    return builder.toString();
  }

  private static void assertBarePayload(final byte[] encoded) {
    if (encoded.length < 4) {
      return;
    }
    final boolean hasMagic =
        encoded[0] == 'N' && encoded[1] == 'R' && encoded[2] == 'T' && encoded[3] == '0';
    assert !hasMagic : "Encoded payload should be bare (no Norito header)";
  }

  private static <T> List<T> listOf(final T... items) {
    return Arrays.asList(items);
  }

  private static Map<String, String> mapOf(final String... entries) {
    if (entries.length % 2 != 0) {
      throw new IllegalArgumentException("mapOf requires an even number of arguments");
    }
    final Map<String, String> map = new LinkedHashMap<>();
    for (int i = 0; i < entries.length; i += 2) {
      map.put(entries[i], entries[i + 1]);
    }
    return map;
  }

  private static String sampleAuthority(final byte fill) {
    final byte[] publicKey = new byte[32];
    Arrays.fill(publicKey, fill);
    try {
      return AccountAddress.fromAccount(publicKey, "ed25519")
          .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalStateException("Failed to build sample authority", ex);
    }
  }

  private static byte[] fill(final int value, final int length) {
    final byte[] out = new byte[length];
    Arrays.fill(out, (byte) value);
    return out;
  }

  private static void expectNoritoFailure(final CheckedNoritoRunnable action) {
    try {
      action.run();
      throw new AssertionError("Expected NoritoException");
    } catch (final NoritoException expected) {
      assert expected.getMessage() != null : "Norito failure should carry a message";
    }
  }

  private interface CheckedNoritoRunnable {
    void run() throws NoritoException;
  }

  private static java.util.Optional<byte[]> decodeOptionPayload(
      final byte[] payload, final String field) {
    final NoritoDecoder decoder = canonicalDecoder(payload);
    final int tag = decoder.readByte();
    if (tag == 0) {
      if (decoder.remaining() != 0) {
        throw new IllegalArgumentException(field + " Option::None has trailing bytes");
      }
      return java.util.Optional.empty();
    }
    if (tag != 1) {
      throw new IllegalArgumentException(field + " invalid Option tag: " + tag);
    }
    final long length = decoder.readLength(decoder.compactLenActive());
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(field + " Option payload too large");
    }
    final byte[] inner = decoder.readBytes((int) length);
    if (decoder.remaining() != 0) {
      throw new IllegalArgumentException(field + " Option payload has trailing bytes");
    }
    return java.util.Optional.of(inner);
  }

  private static void assertOptionPayloadEmpty(final byte[] payload, final String field) {
    final java.util.Optional<byte[]> inner = decodeOptionPayload(payload, field);
    assert inner.isEmpty() : field + " must be empty";
  }

  private static void assertMultisigSignaturePayload(
      final NoritoDecoder decoder, final MultisigSignature signature, final String field) {
    final byte[] publicKeyPayload = BYTE_VECTOR_ADAPTER.decode(decoder);
    final byte[] signaturePayload = BYTE_VECTOR_ADAPTER.decode(decoder);
    assert publicKeyPayload.length == signature.publicKey().length + 1
        : field + " public key payload length mismatch";
    assert (publicKeyPayload[0] & 0xFF) == signature.algorithmTag()
        : field + " algorithm tag mismatch";
    assert Arrays.equals(
        Arrays.copyOfRange(publicKeyPayload, 1, publicKeyPayload.length),
        signature.publicKey())
        : field + " public key payload mismatch";
    assert Arrays.equals(signaturePayload, signature.signature())
        : field + " signature payload mismatch";
  }
}

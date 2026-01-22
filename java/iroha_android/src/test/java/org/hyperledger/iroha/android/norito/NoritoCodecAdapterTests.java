package org.hyperledger.iroha.android.norito;

import java.util.Arrays;
import java.util.Base64;
import java.nio.charset.StandardCharsets;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.KeyManagementException;
import org.hyperledger.iroha.android.model.Executable;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.android.model.TransactionPayload;
import org.hyperledger.iroha.android.model.instructions.BurnAssetInstruction;
import org.hyperledger.iroha.android.model.instructions.BurnTriggerRepetitionsInstruction;
import org.hyperledger.iroha.android.model.instructions.CreateKaigiInstruction;
import org.hyperledger.iroha.android.model.instructions.InstructionBuilders;
import org.hyperledger.iroha.android.model.instructions.InstructionKind;
import org.hyperledger.iroha.android.model.instructions.ExecuteTriggerInstruction;
import org.hyperledger.iroha.android.model.instructions.EndKaigiInstruction;
import org.hyperledger.iroha.android.model.instructions.GrantPermissionInstruction;
import org.hyperledger.iroha.android.model.instructions.GrantRoleInstruction;
import org.hyperledger.iroha.android.model.instructions.GrantRolePermissionInstruction;
import org.hyperledger.iroha.android.model.instructions.JoinKaigiInstruction;
import org.hyperledger.iroha.android.model.instructions.LeaveKaigiInstruction;
import org.hyperledger.iroha.android.model.instructions.MintAssetInstruction;
import org.hyperledger.iroha.android.model.instructions.MintTriggerRepetitionsInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterAccountInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterAssetDefinitionInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterDomainInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterKaigiRelayInstruction;
import org.hyperledger.iroha.android.model.instructions.RevokePermissionInstruction;
import org.hyperledger.iroha.android.model.instructions.RevokeRoleInstruction;
import org.hyperledger.iroha.android.model.instructions.RevokeRolePermissionInstruction;
import org.hyperledger.iroha.android.model.instructions.RemoveAssetKeyValueInstruction;
import org.hyperledger.iroha.android.model.instructions.RemoveKeyValueInstruction;
import org.hyperledger.iroha.android.model.instructions.SetAssetKeyValueInstruction;
import org.hyperledger.iroha.android.model.instructions.SetParameterInstruction;
import org.hyperledger.iroha.android.model.instructions.SetKeyValueInstruction;
import org.hyperledger.iroha.android.model.instructions.RecordKaigiUsageInstruction;
import org.hyperledger.iroha.android.model.instructions.LogInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterPeerWithPopInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterPipelineTriggerInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterPrecommitTriggerInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterTimeTriggerInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterRoleInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterNftInstruction;
import org.hyperledger.iroha.android.model.instructions.SetKaigiRelayManifestInstruction;
import org.hyperledger.iroha.android.model.instructions.UnregisterInstruction;
import org.hyperledger.iroha.android.model.instructions.UpgradeInstruction;
import org.hyperledger.iroha.android.model.instructions.TransferAssetDefinitionInstruction;
import org.hyperledger.iroha.android.model.instructions.TransferAssetInstruction;
import org.hyperledger.iroha.android.model.instructions.TransferDomainInstruction;
import org.hyperledger.iroha.android.model.instructions.TransferNftInstruction;
import org.hyperledger.iroha.android.model.instructions.RegisterVerifyingKeyInstruction;
import org.hyperledger.iroha.android.model.instructions.UpdateVerifyingKeyInstruction;
import org.hyperledger.iroha.android.model.zk.VerifyingKeyBackendTag;
import org.hyperledger.iroha.android.model.zk.VerifyingKeyRecordDescription;
import org.hyperledger.iroha.android.model.zk.VerifyingKeyStatus;
import org.hyperledger.iroha.android.crypto.Blake2b;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.tx.SignedTransactionHasher;
import org.hyperledger.iroha.android.norito.SignedTransactionEncoder;
import org.hyperledger.iroha.android.tx.TransactionBuilder;
import org.hyperledger.iroha.android.SigningException;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;
import org.junit.Test;

public final class NoritoCodecAdapterTests {

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
    javaCodecEncodesChainIdLayout();
    javaCodecSupportsInstructionsVariant();
    javaCodecSupportsWireInstructionPayloads();
    javaCodecEncodesIvmBytecodeLayout();
    javaCodecEncodesInstructionLayout();
    javaCodecHydratesTypedRegisterInstructions();
    javaCodecHydratesTransferAssetInstruction();
    javaCodecHydratesTransferNftInstruction();
    javaCodecHydratesMintAndBurnInstructions();
    javaCodecHydratesTriggerRepetitionInstructions();
    javaCodecHydratesGrantAndRevokeInstructions();
    javaCodecHydratesGrantRoleVariants();
    javaCodecHydratesSetKeyValueInstructions();
    javaCodecHydratesSetParameterInstruction();
    javaCodecHydratesExecuteTriggerAndLogInstructions();
    javaCodecHydratesUnregisterInstructions();
    javaCodecHydratesUpgradeInstruction();
    javaCodecHydratesRegisterPeerWithPopInstruction();
    javaCodecHydratesRegisterRoleInstruction();
    javaCodecHydratesRegisterNftInstruction();
    javaCodecHydratesRegisterTimeTriggerInstruction();
    javaCodecHydratesRegisterPipelineTriggerInstruction();
    javaCodecHydratesRegisterPrecommitTriggerInstruction();
    javaCodecHydratesKaigiInstructions();
    javaCodecHydratesVerifyingKeyInstructions();
    System.out.println("[IrohaAndroid] Norito codec scaffolding tests passed.");
  }

  private static void javaCodecRoundTripsPayload() throws NoritoException {
    final byte[] instructions = "android-instructions".getBytes();
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000001")
            .setAuthority("alice@wonderland")
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
    assert decoded.authority().equals("alice@wonderland") : "Authority must round-trip";
    assert decoded.creationTimeMs() == 1_735_000_000_123L : "creation_time_ms must round-trip";
    assert Arrays.equals(instructions, decoded.executable().ivmBytes())
        : "Decoded payload should match original instructions";
    assert decoded.timeToLiveMs().orElseThrow() == 5_000L : "TTL must round-trip";
    assert decoded.nonce().orElseThrow() == 42 : "Nonce must round-trip";
    assert "unit-test".equals(decoded.metadata().get("purpose")) : "Metadata must round-trip";
    assertBarePayload(encoded);
  }

  private static void javaCodecEncodesAccountIdAuthority() throws NoritoException {
    final byte[] publicKey = new byte[32];
    Arrays.fill(publicKey, (byte) 0x3A);
    final String ih58;
    try {
      ih58 =
          AccountAddress.fromAccount("wonderland", publicKey, "ed25519")
              .toIH58(AccountAddress.DEFAULT_IH58_PREFIX);
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalStateException("Failed to build authority address", ex);
    }
    final String authority = ih58 + "@wonderland";
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

    assert authority.equals(decoded.authority()) : "AccountId authority must round-trip with domain";
    final NoritoDecoder decoder = new NoritoDecoder(encoded, NoritoHeader.MINOR_VERSION);
    readField(decoder, "payload.chain_id");
    final byte[] authorityField = readField(decoder, "payload.authority");
    final int expectedStringPayloadLen = 8 + authority.getBytes(StandardCharsets.UTF_8).length;
    assert authorityField.length != expectedStringPayloadLen
        : "AccountId authority should encode as struct, not legacy string";
    final long domainFieldLen = readU64(authorityField, 0, "authority.domain");
    final long domainNameFieldLen = readU64(authorityField, 8, "authority.domain.name");
    final long domainStringLen = readU64(authorityField, 16, "authority.domain.name.string");
    assert domainNameFieldLen == 8 + domainStringLen : "DomainId name must wrap a single string";
    assert domainFieldLen == 8 + domainNameFieldLen : "Domain field must wrap a DomainId payload";
    final int controllerFieldOffset = Math.toIntExact(8 + domainFieldLen);
    final long controllerFieldLen = readU64(authorityField, controllerFieldOffset, "authority.controller");
    final int controllerPayloadOffset = controllerFieldOffset + 8;
    final long controllerTag = readU32(authorityField, controllerPayloadOffset, "authority.controller.tag");
    assert controllerTag == 0L : "AccountController tag must be Single";
    final long publicKeyFieldLen =
        readU64(authorityField, controllerPayloadOffset + 4, "authority.controller.public_key");
    final long publicKeyStringLen =
        readU64(authorityField, controllerPayloadOffset + 12, "authority.controller.public_key.string");
    assert publicKeyFieldLen == 8 + publicKeyStringLen
        : "Public key field must wrap a single string";
    assert authorityField.length
        == Math.toIntExact(8 + domainFieldLen + 8 + controllerFieldLen)
        : "Authority payload must contain domain and controller fields only";
  }

  private static void javaCodecEncodesChainIdLayout() throws NoritoException {
    final String chainId = "00000003";
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId(chainId)
            .setAuthority("chain@wonderland")
            .setCreationTimeMs(1_735_000_000_789L)
            .setExecutable(Executable.ivm(new byte[] {0x01}))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final byte[] encoded = adapter.encodeTransaction(payload);
    final NoritoDecoder decoder = new NoritoDecoder(encoded, NoritoHeader.MINOR_VERSION);
    final byte[] chainField = readField(decoder, "payload.chain_id");
    final long chainInnerLen = readU64(chainField, 0, "payload.chain_id");
    assert chainField.length == 8 + chainInnerLen : "ChainId must be a sized struct";
    final long stringLen = readU64(chainField, 8, "payload.chain_id.string");
    assert chainInnerLen == 8 + stringLen : "ChainId must wrap a single string";
    final String decodedChain =
        new String(chainField, 16, Math.toIntExact(stringLen), StandardCharsets.UTF_8);
    assert chainId.equals(decodedChain) : "ChainId must round-trip via layout inspection";
  }

  private static void javaCodecSupportsInstructionsVariant() throws NoritoException {
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000009")
            .setAuthority("instructions@wonderland")
            .setCreationTimeMs(1_735_111_111_000L)
            .setExecutable(
                Executable.instructions(
                    listOf(
                        InstructionBuilders.custom(InstructionKind.REGISTER, mapOf("action", "InstrA", "arg", "1")),
                        InstructionBuilders.custom(InstructionKind.CUSTOM, mapOf("action", "InstrB")))))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final byte[] encoded = adapter.encodeTransaction(payload);
    final TransactionPayload decoded = adapter.decodeTransaction(encoded);

    assert decoded.executable().isInstructions() : "Executable should decode as instructions";
    assert decoded.executable().instructions().size() == payload.executable().instructions().size()
        : "Instruction count must match";
    for (int i = 0; i < payload.executable().instructions().size(); i++) {
      final InstructionBox expected = payload.executable().instructions().get(i);
      final InstructionBox actual = decoded.executable().instructions().get(i);
      assert expected.kind() == actual.kind() : "Instruction kind mismatch";
      assert expected.arguments().equals(actual.arguments()) : "Instruction arguments mismatch";
    }
  }

  private static void javaCodecSupportsWireInstructionPayloads() throws NoritoException {
    final byte[] wirePayload =
        NoritoCodec.encode("wire-payload", "iroha.test.WirePayload", NoritoAdapters.stringAdapter());
    final InstructionBox wireInstruction =
        InstructionBox.fromWirePayload("iroha.custom", wirePayload);

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000011")
            .setAuthority("wire@wonderland")
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
            .setAuthority("ivm@wonderland")
            .setCreationTimeMs(1_735_222_222_123L)
            .setExecutable(Executable.ivm(ivmBytes))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final byte[] encoded = adapter.encodeTransaction(payload);

    final NoritoDecoder decoder = new NoritoDecoder(encoded, NoritoHeader.MINOR_VERSION);
    readField(decoder, "payload.chain_id");
    readField(decoder, "payload.authority");
    readField(decoder, "payload.creation_time_ms");
    final byte[] executableField = readField(decoder, "payload.executable");
    readField(decoder, "payload.time_to_live_ms");
    readField(decoder, "payload.nonce");
    readField(decoder, "payload.metadata");
    assert decoder.remaining() == 0 : "Payload has trailing bytes";

    final NoritoDecoder execDecoder = new NoritoDecoder(executableField, NoritoHeader.MINOR_VERSION);
    final TypeAdapter<Long> uint32 = NoritoAdapters.uint(32);
    final long tag = uint32.decode(execDecoder);
    assert tag == 1L : "Executable should be Ivm";
    final byte[] ivmField = readField(execDecoder, "payload.executable.ivm");
    assert execDecoder.remaining() == 0 : "Executable has trailing bytes";

    final long ivmInnerLen = readU64(ivmField, 0, "payload.executable.ivm");
    assert ivmField.length == 8 + ivmInnerLen : "IVM bytecode must be sized";
    final byte[] ivmPayload =
        Arrays.copyOfRange(ivmField, 8, Math.toIntExact(8 + ivmInnerLen));
    final byte[] decodedIvm =
        decodeFieldPayload(ivmPayload, NoritoAdapters.byteVecAdapter(), "payload.executable.ivm.bytes");
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
            .setAuthority("layout@wonderland")
            .setCreationTimeMs(1_735_222_333_123L)
            .setExecutable(Executable.instructions(listOf(wireInstruction)))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final byte[] encoded = adapter.encodeTransaction(payload);

    final NoritoDecoder decoder = new NoritoDecoder(encoded, NoritoHeader.MINOR_VERSION);
    readField(decoder, "payload.chain_id");
    readField(decoder, "payload.authority");
    readField(decoder, "payload.creation_time_ms");
    final byte[] executableField = readField(decoder, "payload.executable");
    readField(decoder, "payload.time_to_live_ms");
    readField(decoder, "payload.nonce");
    readField(decoder, "payload.metadata");
    assert decoder.remaining() == 0 : "Payload has trailing bytes";

    final NoritoDecoder execDecoder = new NoritoDecoder(executableField, NoritoHeader.MINOR_VERSION);
    final TypeAdapter<Long> uint32 = NoritoAdapters.uint(32);
    final long tag = uint32.decode(execDecoder);
    assert tag == 0L : "Executable should be Instructions";
    final byte[] instructionsField = readField(execDecoder, "payload.executable.instructions");
    assert execDecoder.remaining() == 0 : "Executable has trailing bytes";

    final NoritoDecoder listDecoder = new NoritoDecoder(instructionsField, NoritoHeader.MINOR_VERSION);
    final long count = listDecoder.readLength(false);
    assert count == 1L : "Instruction list should contain one element";
    final long elementLength = listDecoder.readLength(listDecoder.compactLenActive());
    assert elementLength > 0 : "Instruction element must not be empty";
    if (elementLength > Integer.MAX_VALUE) {
      throw new IllegalArgumentException("Instruction element too large");
    }
    final byte[] elementPayload = listDecoder.readBytes((int) elementLength);
    assert listDecoder.remaining() == 0 : "Instruction list has trailing bytes";

    final NoritoDecoder elementDecoder = new NoritoDecoder(elementPayload, NoritoHeader.MINOR_VERSION);
    final byte[] nameField = readField(elementDecoder, "instruction.name");
    final byte[] payloadField = readField(elementDecoder, "instruction.payload");
    assert elementDecoder.remaining() == 0 : "Instruction element has trailing bytes";
    final String decodedName =
        decodeFieldPayload(nameField, NoritoAdapters.stringAdapter(), "instruction.name");
    final byte[] decodedPayload =
        decodeFieldPayload(payloadField, NoritoAdapters.byteVecAdapter(), "instruction.payload");
    assert "iroha.custom.layout".equals(decodedName) : "Instruction name must match wire payload";
    assert Arrays.equals(wirePayload, decodedPayload) : "Instruction payload must match wire bytes";
  }

  private static void javaCodecHydratesTypedRegisterInstructions() throws NoritoException {
    final InstructionBox registerDomain =
        InstructionBox.of(
            RegisterDomainInstruction.builder()
                .setDomainName("wonderland")
                .setLogo("ipfs://domain-logo")
                .putMetadata("tier", "governance")
                .build());
    final InstructionBox registerAccount =
        InstructionBox.of(
            RegisterAccountInstruction.builder()
                .setAccountId("ed0120ABC@wonderland")
                .putMetadata("role", "registrar")
                .build());
    final InstructionBox registerAsset =
        InstructionBox.of(
            RegisterAssetDefinitionInstruction.builder()
                .setAssetDefinitionId("rose#wonderland")
                .setDisplayName("Rose Token")
                .setDescription("Utility token for Wonderland")
                .setLogo("ipfs://rose-logo")
                .putMetadata("decimals", "2")
                .build());

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000010")
            .setAuthority("registrar@wonderland")
            .setExecutable(
                Executable.instructions(listOf(registerDomain, registerAccount, registerAsset)))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final byte[] encodedBytes = adapter.encodeTransaction(payload);
    final TransactionPayload decoded = adapter.decodeTransaction(encodedBytes);

    final List<InstructionBox> instructions = decoded.executable().instructions();
    assert instructions.size() == 3 : "Expected three instructions";

    final InstructionBox domainBox = instructions.get(0);
    assert domainBox.payload() instanceof RegisterDomainInstruction : "Domain instruction should hydrate";
    final RegisterDomainInstruction domain = (RegisterDomainInstruction) domainBox.payload();
    assert "wonderland".equals(domain.domainName()) : "Domain name must round-trip";
    assert "ipfs://domain-logo".equals(domain.logo()) : "Logo must round-trip";
    assert "governance".equals(domain.metadata().get("tier")) : "Domain metadata must round-trip";

    final InstructionBox accountBox = instructions.get(1);
    assert accountBox.payload() instanceof RegisterAccountInstruction : "Account instruction should hydrate";
    final RegisterAccountInstruction account = (RegisterAccountInstruction) accountBox.payload();
    assert "ed0120ABC@wonderland".equals(account.accountId()) : "Account id must round-trip";
    assert "registrar".equals(account.metadata().get("role")) : "Account metadata must round-trip";

    final InstructionBox assetBox = instructions.get(2);
    assert assetBox.payload() instanceof RegisterAssetDefinitionInstruction
        : "Asset definition instruction should hydrate";
    final RegisterAssetDefinitionInstruction asset =
        (RegisterAssetDefinitionInstruction) assetBox.payload();
    assert "rose#wonderland".equals(asset.assetDefinitionId()) : "Asset id must round-trip";
    assert "Rose Token".equals(asset.displayName()) : "Display name must round-trip";
    assert "Utility token for Wonderland".equals(asset.description()) : "Description must round-trip";
    assert "ipfs://rose-logo".equals(asset.logo()) : "Logo must round-trip";
    assert "2".equals(asset.metadata().get("decimals")) : "Asset metadata must round-trip";
  }

  private static void javaCodecHydratesTransferAssetInstruction() throws NoritoException {
    final InstructionBox transferAsset =
        InstructionBox.of(
            TransferAssetInstruction.builder()
                .setAssetId("rose#wonderland#alice@wonderland")
                .setQuantity("100.5000")
                .setDestinationAccountId("bob@wonderland")
                .build());
    final InstructionBox transferDomain =
        InstructionBox.of(
            TransferDomainInstruction.builder()
                .setSourceAccountId("alice@wonderland")
                .setDomainId("wonderland")
                .setDestinationAccountId("bob@wonderland")
                .build());
    final InstructionBox transferDefinition =
        InstructionBox.of(
            TransferAssetDefinitionInstruction.builder()
                .setSourceAccountId("alice@wonderland")
                .setAssetDefinitionId("rose#wonderland")
                .setDestinationAccountId("bob@wonderland")
                .build());

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000011")
            .setAuthority("alice@wonderland")
            .setExecutable(Executable.instructions(listOf(transferAsset, transferDomain, transferDefinition)))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final TransactionPayload decoded = adapter.decodeTransaction(adapter.encodeTransaction(payload));

    final List<InstructionBox> instructions = decoded.executable().instructions();
    final TransferAssetInstruction assetInstruction =
        (TransferAssetInstruction) instructions.get(0).payload();
    assert "rose#wonderland#alice@wonderland".equals(assetInstruction.assetId())
        : "Asset ID must round-trip";
    assert "100.5000".equals(assetInstruction.quantity()) : "Quantity must round-trip";
    assert "bob@wonderland".equals(assetInstruction.destinationAccountId())
        : "Destination must round-trip";

    final TransferDomainInstruction domainInstruction =
        (TransferDomainInstruction) instructions.get(1).payload();
    assert "alice@wonderland".equals(domainInstruction.sourceAccountId()) : "Domain source";
    assert "wonderland".equals(domainInstruction.domainId()) : "Domain ID";
    assert "bob@wonderland".equals(domainInstruction.destinationAccountId()) : "Domain destination";

    final TransferAssetDefinitionInstruction definitionInstruction =
        (TransferAssetDefinitionInstruction) instructions.get(2).payload();
    assert "alice@wonderland".equals(definitionInstruction.sourceAccountId()) : "Definition source";
    assert "rose#wonderland".equals(definitionInstruction.assetDefinitionId()) : "Definition ID";
    assert "bob@wonderland".equals(definitionInstruction.destinationAccountId()) : "Definition destination";
  }

  private static void javaCodecHydratesTransferNftInstruction() throws NoritoException {
    final InstructionBox transfer =
        InstructionBuilders.transferNft(
            "alice@wonderland", "rare_card$collectibles", "bob@wonderland");

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000020")
            .setAuthority("alice@wonderland")
            .setCreationTimeMs(1_735_003_300_000L)
            .setExecutable(Executable.instructions(listOf(transfer)))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final byte[] encodedBytes =
        emitFixtureMetadata(
            "transfer_nft_demo", payload, adapter, "transfer-nft-fixture");
    final TransactionPayload decoded = adapter.decodeTransaction(encodedBytes);

    final InstructionBox decodedBox = decoded.executable().instructions().get(0);
    assert decodedBox.payload() instanceof TransferNftInstruction
        : "TransferNft instruction should hydrate";
    final TransferNftInstruction instruction =
        (TransferNftInstruction) decodedBox.payload();
    assert "alice@wonderland".equals(instruction.sourceAccountId())
        : "Source account must round-trip";
    assert "rare_card$collectibles".equals(instruction.nftId()) : "NFT id must round-trip";
    assert "bob@wonderland".equals(instruction.destinationAccountId())
        : "Destination account must round-trip";
    assert "TransferNft".equals(decodedBox.arguments().get("action"))
        : "Action must remain canonical";
    assert "rare_card$collectibles".equals(decodedBox.arguments().get("nft"))
        : "NFT argument must remain canonical";
  }

  private static void javaCodecHydratesMintAndBurnInstructions() throws NoritoException {
    final InstructionBox mint =
        InstructionBox.of(
            MintAssetInstruction.builder()
                .setAssetId("rose#wonderland#alice@wonderland")
                .setQuantity("42.1000")
                .build());
    final InstructionBox burn =
        InstructionBox.of(
            BurnAssetInstruction.builder()
                .setAssetId("rose#wonderland#alice@wonderland")
                .setQuantity("5")
                .build());

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000012")
            .setAuthority("alice@wonderland")
            .setExecutable(Executable.instructions(listOf(mint, burn)))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final TransactionPayload decoded = adapter.decodeTransaction(adapter.encodeTransaction(payload));

    final List<InstructionBox> instructions = decoded.executable().instructions();
    final MintAssetInstruction mintInstruction = (MintAssetInstruction) instructions.get(0).payload();
    assert "rose#wonderland#alice@wonderland".equals(mintInstruction.assetId())
        : "Mint asset ID must round-trip";
    assert "42.1000".equals(mintInstruction.quantity()) : "Mint quantity must round-trip";

    final BurnAssetInstruction burnInstruction = (BurnAssetInstruction) instructions.get(1).payload();
    assert "rose#wonderland#alice@wonderland".equals(burnInstruction.assetId())
        : "Burn asset ID must round-trip";
    assert "5".equals(burnInstruction.quantity()) : "Burn quantity must round-trip";
  }

  private static void javaCodecHydratesTriggerRepetitionInstructions() throws NoritoException {
    final InstructionBox mint =
        InstructionBuilders.mintTriggerRepetitions("daily-report", 5);
    final InstructionBox burn =
        InstructionBuilders.burnTriggerRepetitions("daily-report", 2);

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("0000001C")
            .setAuthority("alice@wonderland")
            .setCreationTimeMs(1_735_002_900_000L)
            .setExecutable(Executable.instructions(listOf(mint, burn)))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final byte[] encodedBytes =
        emitFixtureMetadata(
            "trigger_repetitions_demo", payload, adapter, "trigger-repetitions-fixture");
    final TransactionPayload decoded = adapter.decodeTransaction(encodedBytes);

    final List<InstructionBox> instructions = decoded.executable().instructions();
    assert instructions.size() == 2 : "Expected two trigger repetition instructions";

    final MintTriggerRepetitionsInstruction mintInstruction =
        (MintTriggerRepetitionsInstruction) instructions.get(0).payload();
    assert "daily-report".equals(mintInstruction.triggerId()) : "Mint trigger id must round-trip";
    assert mintInstruction.repetitions() == 5 : "Mint trigger repetitions must round-trip";

    final BurnTriggerRepetitionsInstruction burnInstruction =
        (BurnTriggerRepetitionsInstruction) instructions.get(1).payload();
    assert "daily-report".equals(burnInstruction.triggerId()) : "Burn trigger id must round-trip";
    assert burnInstruction.repetitions() == 2 : "Burn trigger repetitions must round-trip";
  }

  private static void javaCodecHydratesGrantAndRevokeInstructions() throws NoritoException {
    final InstructionBox grant =
        InstructionBox.of(
            GrantPermissionInstruction.builder()
                .setDestinationId("bob@wonderland")
                .setPermissionName("CanRegisterDomain")
                .build());
    final InstructionBox revoke =
        InstructionBox.of(
            RevokePermissionInstruction.builder()
                .setDestinationId("bob@wonderland")
                .setPermissionName("CanRegisterDomain")
                .build());

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000013")
            .setAuthority("alice@wonderland")
            .setExecutable(Executable.instructions(listOf(grant, revoke)))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final TransactionPayload decoded = adapter.decodeTransaction(adapter.encodeTransaction(payload));

    final List<InstructionBox> instructions = decoded.executable().instructions();
    final GrantPermissionInstruction grantInstruction =
        (GrantPermissionInstruction) instructions.get(0).payload();
    assert "bob@wonderland".equals(grantInstruction.destinationId())
        : "Grant destination";
    assert "CanRegisterDomain".equals(grantInstruction.permissionName())
        : "Grant permission";

    final RevokePermissionInstruction revokeInstruction =
        (RevokePermissionInstruction) instructions.get(1).payload();
    assert "bob@wonderland".equals(revokeInstruction.destinationId())
        : "Revoke destination";
    assert "CanRegisterDomain".equals(revokeInstruction.permissionName())
        : "Revoke permission";
  }

  private static void javaCodecHydratesGrantRoleVariants() throws NoritoException {
    final InstructionBox grantRole =
        InstructionBox.of(
            GrantRoleInstruction.builder()
                .setDestinationAccountId("charlie@wonderland")
                .setRoleId("registrar")
                .build());
    final InstructionBox revokeRole =
        InstructionBox.of(
            RevokeRoleInstruction.builder()
                .setDestinationAccountId("charlie@wonderland")
                .setRoleId("registrar")
                .build());
    final InstructionBox grantRolePermission =
        InstructionBox.of(
            GrantRolePermissionInstruction.builder()
                .setDestinationRoleId("registrar")
                .setPermissionName("CanRegisterAssetDefinition")
                .build());
    final InstructionBox revokeRolePermission =
        InstructionBox.of(
            RevokeRolePermissionInstruction.builder()
                .setDestinationRoleId("registrar")
                .setPermissionName("CanRegisterAssetDefinition")
                .build());

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000014")
            .setAuthority("alice@wonderland")
            .setExecutable(
                Executable.instructions(
                    listOf(grantRole, revokeRole, grantRolePermission, revokeRolePermission)))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final TransactionPayload decoded = adapter.decodeTransaction(adapter.encodeTransaction(payload));

    final List<InstructionBox> instructions = decoded.executable().instructions();

    final GrantRoleInstruction grantRoleInstruction =
        (GrantRoleInstruction) instructions.get(0).payload();
    assert "charlie@wonderland".equals(grantRoleInstruction.destinationAccountId())
        : "Grant role destination";
    assert "registrar".equals(grantRoleInstruction.roleId()) : "Grant role id";

    final RevokeRoleInstruction revokeRoleInstruction =
        (RevokeRoleInstruction) instructions.get(1).payload();
    assert "charlie@wonderland".equals(revokeRoleInstruction.destinationAccountId())
        : "Revoke role destination";
    assert "registrar".equals(revokeRoleInstruction.roleId()) : "Revoke role id";

    final GrantRolePermissionInstruction grantRolePermInstruction =
        (GrantRolePermissionInstruction) instructions.get(2).payload();
    assert "registrar".equals(grantRolePermInstruction.destinationRoleId())
        : "Grant role permission destination";
    assert "CanRegisterAssetDefinition".equals(grantRolePermInstruction.permissionName())
        : "Grant role permission name";

    final RevokeRolePermissionInstruction revokeRolePermInstruction =
        (RevokeRolePermissionInstruction) instructions.get(3).payload();
    assert "registrar".equals(revokeRolePermInstruction.destinationRoleId())
        : "Revoke role permission destination";
    assert "CanRegisterAssetDefinition".equals(revokeRolePermInstruction.permissionName())
        : "Revoke role permission name";
  }

  private static void javaCodecHydratesSetKeyValueInstructions() throws NoritoException {
    final InstructionBox setDomain =
        InstructionBuilders.setDomainKeyValue("enterprise", "tier", "governance");
    final InstructionBox setAccount =
        InstructionBuilders.setAccountKeyValue("alice@enterprise", "nickname", "alice");
    final InstructionBox setDefinition =
        InstructionBuilders.setAssetDefinitionKeyValue("token#enterprise", "display_name", "Token");
    final InstructionBox setAsset =
        InstructionBuilders.setAssetKeyValue("token#enterprise#alice@enterprise", "notes", "vip");
    final InstructionBox removeAsset =
        InstructionBuilders.removeAssetKeyValue("token#enterprise#alice@enterprise", "obsolete");
    final InstructionBox removeAccount =
        InstructionBuilders.removeAccountKeyValue("alice@enterprise", "archived_key");
    final InstructionBox removeTrigger =
        InstructionBuilders.removeTriggerKeyValue("daily-report", "last_run");

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000015")
            .setAuthority("alice@enterprise")
            .setExecutable(
                Executable.instructions(
                    listOf(
                        setDomain,
                        setAccount,
                        setDefinition,
                        setAsset,
                        removeAsset,
                        removeAccount,
                        removeTrigger)))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final TransactionPayload decoded = adapter.decodeTransaction(adapter.encodeTransaction(payload));
    final List<InstructionBox> instructions = decoded.executable().instructions();

    final SetKeyValueInstruction domainInstruction =
        (SetKeyValueInstruction) instructions.get(0).payload();
    assert domainInstruction.target() == SetKeyValueInstruction.Target.DOMAIN
        : "Set domain key value target";
    assert "enterprise".equals(domainInstruction.objectId()) : "Set domain id";
    assert "tier".equals(domainInstruction.key()) : "Set domain key";
    assert "governance".equals(domainInstruction.value()) : "Set domain value";

    final SetKeyValueInstruction accountInstruction =
        (SetKeyValueInstruction) instructions.get(1).payload();
    assert accountInstruction.target() == SetKeyValueInstruction.Target.ACCOUNT
        : "Set account key value target";
    assert "alice@enterprise".equals(accountInstruction.objectId()) : "Set account id";
    assert "nickname".equals(accountInstruction.key()) : "Set account key";
    assert "alice".equals(accountInstruction.value()) : "Set account value";

    final SetKeyValueInstruction definitionInstruction =
        (SetKeyValueInstruction) instructions.get(2).payload();
    assert definitionInstruction.target() == SetKeyValueInstruction.Target.ASSET_DEFINITION
        : "Set definition key value target";
    assert "token#enterprise".equals(definitionInstruction.objectId()) : "Set definition id";
    assert "display_name".equals(definitionInstruction.key()) : "Set definition key";
    assert "Token".equals(definitionInstruction.value()) : "Set definition value";

    final SetAssetKeyValueInstruction assetInstruction =
        (SetAssetKeyValueInstruction) instructions.get(3).payload();
    assert "token#enterprise#alice@enterprise".equals(assetInstruction.assetId())
        : "Set asset id";
    assert "notes".equals(assetInstruction.key()) : "Set asset key";
    assert "vip".equals(assetInstruction.value()) : "Set asset value";

    final RemoveAssetKeyValueInstruction removeAssetInstruction =
        (RemoveAssetKeyValueInstruction) instructions.get(4).payload();
    assert "token#enterprise#alice@enterprise".equals(removeAssetInstruction.assetId())
        : "Remove asset id";
    assert "obsolete".equals(removeAssetInstruction.key()) : "Remove asset key";

    final RemoveKeyValueInstruction removeAccountInstruction =
        (RemoveKeyValueInstruction) instructions.get(5).payload();
    assert removeAccountInstruction.target() == RemoveKeyValueInstruction.Target.ACCOUNT
        : "Remove account key value target";
    assert "alice@enterprise".equals(removeAccountInstruction.objectId()) : "Remove account id";
    assert "archived_key".equals(removeAccountInstruction.key()) : "Remove account key";

    final RemoveKeyValueInstruction removeTriggerInstruction =
        (RemoveKeyValueInstruction) instructions.get(6).payload();
    assert removeTriggerInstruction.target() == RemoveKeyValueInstruction.Target.TRIGGER
        : "Remove trigger key value target";
    assert "daily-report".equals(removeTriggerInstruction.objectId())
        : "Remove trigger id";
    assert "last_run".equals(removeTriggerInstruction.key()) : "Remove trigger key";
  }

  private static void javaCodecHydratesSetParameterInstruction() throws NoritoException {
    final String parameterJson = "{\"Sumeragi\":{\"NextMode\":\"Npos\"}}";
    final InstructionBox setParameter = InstructionBuilders.setParameter(parameterJson);

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000011")
            .setAuthority("alice@wonderland")
            .setExecutable(Executable.instructions(listOf(setParameter)))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final TransactionPayload decoded = adapter.decodeTransaction(adapter.encodeTransaction(payload));

    final List<InstructionBox> instructions = decoded.executable().instructions();
    assert instructions.size() == 1 : "Expected single SetParameter instruction";
    final InstructionBox box = instructions.get(0);
    assert box.payload() instanceof SetParameterInstruction : "Payload should hydrate SetParameter";
    final SetParameterInstruction instruction = (SetParameterInstruction) box.payload();
    assert parameterJson.equals(instruction.parameterJson()) : "Parameter JSON must round-trip";
    assert "SetParameter".equals(box.arguments().get("action")) : "Action should remain SetParameter";
  }

  private static void javaCodecHydratesUnregisterInstructions() throws NoritoException {
    final InstructionBox unregisterDomain = InstructionBuilders.unregisterDomain("archive");
    final InstructionBox unregisterAccount =
        InstructionBuilders.unregisterAccount("retired@archive");
    final InstructionBox unregisterTrigger =
        InstructionBuilders.unregisterTrigger("cleanup");

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000016")
            .setAuthority("admin@wonderland")
            .setExecutable(
                Executable.instructions(
                    listOf(unregisterDomain, unregisterAccount, unregisterTrigger)))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final TransactionPayload decoded = adapter.decodeTransaction(adapter.encodeTransaction(payload));

    final List<InstructionBox> instructions = decoded.executable().instructions();
    assert instructions.size() == 3 : "Expected three unregister instructions";

    final UnregisterInstruction domain =
        (UnregisterInstruction) instructions.get(0).payload();
    assert domain.target() == UnregisterInstruction.Target.DOMAIN : "Domain target must hydrate";
    assert "archive".equals(domain.objectId()) : "Domain id must round-trip";

    final UnregisterInstruction account =
        (UnregisterInstruction) instructions.get(1).payload();
    assert account.target() == UnregisterInstruction.Target.ACCOUNT : "Account target must hydrate";
    assert "retired@archive".equals(account.objectId()) : "Account id must round-trip";

    final UnregisterInstruction trigger =
        (UnregisterInstruction) instructions.get(2).payload();
    assert trigger.target() == UnregisterInstruction.Target.TRIGGER : "Trigger target must hydrate";
    assert "cleanup".equals(trigger.objectId()) : "Trigger id must round-trip";
  }

  private static void javaCodecHydratesExecuteTriggerAndLogInstructions() throws NoritoException {
    final InstructionBox executeTrigger =
        InstructionBuilders.executeTrigger("daily-report", "reporter@wonderland");
    final InstructionBox logInstruction =
        InstructionBuilders.log("INFO", "cron job finished");

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000017")
            .setAuthority("ops@wonderland")
            .setExecutable(Executable.instructions(listOf(executeTrigger, logInstruction)))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final TransactionPayload decoded = adapter.decodeTransaction(adapter.encodeTransaction(payload));

    final List<InstructionBox> instructions = decoded.executable().instructions();
    assert instructions.size() == 2 : "Expected ExecuteTrigger and Log instructions";

    final ExecuteTriggerInstruction execute =
        (ExecuteTriggerInstruction) instructions.get(0).payload();
    assert "daily-report".equals(execute.triggerId()) : "Trigger id must round-trip";
    assert "reporter@wonderland".equals(execute.authorityOverride())
        : "Authority override must round-trip";

    final LogInstruction log = (LogInstruction) instructions.get(1).payload();
    assert "INFO".equals(log.level()) : "Log level must round-trip";
    assert "cron job finished".equals(log.message()) : "Log message must round-trip";
  }

  private static void javaCodecHydratesUpgradeInstruction() throws NoritoException {
    final byte[] bytecode = {0x01, 0x23, (byte) 0xFE, (byte) 0xDC};
    final InstructionBox upgrade = InstructionBuilders.upgradeExecutor(bytecode);

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000018")
            .setAuthority("governance@wonderland")
            .setExecutable(Executable.instructions(listOf(upgrade)))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final TransactionPayload decoded = adapter.decodeTransaction(adapter.encodeTransaction(payload));

    final List<InstructionBox> instructions = decoded.executable().instructions();
    assert instructions.size() == 1 : "Expected single Upgrade instruction";

    final InstructionBox upgradeBox = instructions.get(0);
    assert upgradeBox.payload() instanceof UpgradeInstruction : "Upgrade payload should hydrate";
    final UpgradeInstruction instruction = (UpgradeInstruction) upgradeBox.payload();
    assert Arrays.equals(bytecode, instruction.bytecode()) : "Executor bytecode must round-trip";
    final String encoded = Base64.getEncoder().encodeToString(bytecode);
    assert encoded.equals(upgradeBox.arguments().get("bytecode"))
        : "Upgrade bytecode argument must remain base64 encoded";
  }

  private static void javaCodecHydratesRegisterPeerWithPopInstruction() throws NoritoException {
    final String peerPublicKey =
        "ed012076E5CA9698296AF9BE2CA45F525CB3BCFDEB7EE068BA56F973E9DD90564EF4FC";
    final byte[] pop = new byte[] {0x00, 0x11, 0x22, 0x33, 0x44};
    final InstructionBox registerPeer = InstructionBuilders.registerPeerWithPop(peerPublicKey, pop);

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000019")
            .setAuthority("genesis@wonderland")
            .setExecutable(Executable.instructions(listOf(registerPeer)))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final TransactionPayload decoded = adapter.decodeTransaction(adapter.encodeTransaction(payload));

    final List<InstructionBox> instructions = decoded.executable().instructions();
    assert instructions.size() == 1 : "Expected single RegisterPeerWithPop instruction";

    final RegisterPeerWithPopInstruction instruction =
        (RegisterPeerWithPopInstruction) instructions.get(0).payload();
    assert peerPublicKey.equals(instruction.peerPublicKey()) : "Peer public key must round-trip";
    assert Arrays.equals(pop, instruction.proofOfPossession()) : "Proof-of-possession must round-trip";
    final String encodedPop = Base64.getEncoder().encodeToString(pop);
    assert encodedPop.equals(instructions.get(0).arguments().get("pop"))
        : "Proof-of-possession argument must remain base64 encoded";
  }

  private static void javaCodecHydratesRegisterRoleInstruction() throws NoritoException {
    final List<String> permissions = listOf("CanRegisterDomain", "CanUnregisterDomain");
    final InstructionBox registerRole =
        InstructionBuilders.registerRole("AUDITOR", "ops@wonderland", permissions);

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("0000001A")
            .setAuthority("ops@wonderland")
            .setExecutable(Executable.instructions(listOf(registerRole)))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final TransactionPayload decoded = adapter.decodeTransaction(adapter.encodeTransaction(payload));

    final List<InstructionBox> instructions = decoded.executable().instructions();
    assert instructions.size() == 1 : "Expected single RegisterRole instruction";

    final RegisterRoleInstruction instruction =
        (RegisterRoleInstruction) instructions.get(0).payload();
    assert "AUDITOR".equals(instruction.roleId()) : "Role id must round-trip";
    assert "ops@wonderland".equals(instruction.ownerAccountId()) : "Owner must round-trip";
    assert instruction.permissions().equals(permissions) : "Permissions must round-trip";
    assert String.join(",", permissions).equals(instructions.get(0).arguments().get("permissions"))
        : "Permissions argument must remain comma separated";
  }

  private static void javaCodecHydratesRegisterNftInstruction() throws NoritoException {
    final Map<String, String> metadata = mapOf("category", "collectible", "edition", "1");
    final InstructionBox registerNft =
        InstructionBuilders.registerNft("rare_card$wonderland", metadata);

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("0000001B")
            .setAuthority("alice@wonderland")
            .setCreationTimeMs(1_735_002_800_000L)
            .setExecutable(Executable.instructions(listOf(registerNft)))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final byte[] encodedBytes =
        emitFixtureMetadata(
            "register_nft_demo", payload, adapter, "register-nft-fixture");
    final TransactionPayload decoded = adapter.decodeTransaction(encodedBytes);

    final RegisterNftInstruction instruction =
        (RegisterNftInstruction) decoded.executable().instructions().get(0).payload();
    assert "rare_card$wonderland".equals(instruction.nftId()) : "NFT id must round-trip";
    assert instruction.metadata().equals(metadata) : "Metadata must round-trip";
    assert !decoded
            .executable()
            .instructions()
            .get(0)
            .arguments()
            .containsKey("owner")
        : "RegisterNft Norito payload should infer ownership from authority";
  }

  private static void javaCodecHydratesRegisterTimeTriggerInstruction() throws NoritoException {
    final InstructionBox nested =
        InstructionBuilders.mintAsset("rose#wonderland#treasury@wonderland", "5");
    final Map<String, String> metadata = mapOf("label", "periodic");
    final InstructionBox register =
        InstructionBuilders.registerTimeTrigger(
            "daily_report",
            "alice@wonderland",
            listOf(nested),
            1_735_000_000_000L,
            60_000L,
            3,
            metadata);

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("0000001C")
            .setAuthority("alice@wonderland")
            .setCreationTimeMs(1_735_003_000_000L)
            .setExecutable(Executable.instructions(listOf(register)))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final byte[] encodedBytes =
        emitFixtureMetadata(
            "register_time_trigger_demo",
            payload,
            adapter,
            "register-time-trigger-fixture");
    final TransactionPayload decoded = adapter.decodeTransaction(encodedBytes);

    final InstructionBox decodedBox = decoded.executable().instructions().get(0);
    assert decodedBox.payload() instanceof RegisterTimeTriggerInstruction
        : "RegisterTimeTrigger instruction should hydrate";
    final RegisterTimeTriggerInstruction instruction =
        (RegisterTimeTriggerInstruction) decodedBox.payload();
    assert "daily_report".equals(instruction.triggerId()) : "Trigger id must round-trip";
    assert "alice@wonderland".equals(instruction.authority()) : "Authority must round-trip";
    assert instruction.startMs() == 1_735_000_000_000L : "Start timestamp must round-trip";
    assert instruction.periodMs() != null && instruction.periodMs() == 60_000L
        : "Period must round-trip";
    assert instruction.repeats() != null && instruction.repeats() == 3
        : "Repeats must round-trip";
    assert instruction.metadata().equals(metadata) : "Metadata must round-trip";
    assert instruction.instructions().size() == 1 : "Nested instruction count must round-trip";
    assert instruction.instructions().get(0).equals(nested) : "Nested instruction must round-trip";

    assert "3".equals(decodedBox.arguments().get("repeats"))
        : "Repeats argument must persist canonical form";
    assert "60000".equals(decodedBox.arguments().get("period_ms"))
        : "Period argument must remain canonical";
  }

  private static void javaCodecHydratesRegisterPipelineTriggerInstruction() throws NoritoException {
    final InstructionBox nested =
        InstructionBuilders.grantPermission("bob@wonderland", "CanSubmitTransactions");
    final String sampleHash = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    final RegisterPipelineTriggerInstruction.PipelineFilter filter =
        RegisterPipelineTriggerInstruction.PipelineFilter.transaction()
            .setTransactionHash(sampleHash)
            .setTransactionBlockHeight(42L)
            .setTransactionStatus("Approved");
    final Map<String, String> metadata = mapOf("label", "pipeline");
    final InstructionBox register =
        InstructionBuilders.registerPipelineTrigger(
            "tx_monitor",
            "monitor@wonderland",
            listOf(nested),
            filter,
            5,
            metadata);

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("0000001E")
            .setAuthority("monitor@wonderland")
            .setCreationTimeMs(1_735_003_200_000L)
            .setExecutable(Executable.instructions(listOf(register)))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final byte[] encodedBytes =
        emitFixtureMetadata(
            "register_pipeline_trigger_demo",
            payload,
            adapter,
            "register-pipeline-trigger-fixture");
    final TransactionPayload decoded = adapter.decodeTransaction(encodedBytes);

    final InstructionBox decodedBox = decoded.executable().instructions().get(0);
    assert decodedBox.payload() instanceof RegisterPipelineTriggerInstruction
        : "RegisterPipelineTrigger instruction should hydrate";
    final RegisterPipelineTriggerInstruction instruction =
        (RegisterPipelineTriggerInstruction) decodedBox.payload();
    assert "tx_monitor".equals(instruction.triggerId()) : "Trigger id must round-trip";
    assert "monitor@wonderland".equals(instruction.authority()) : "Authority must round-trip";
    assert instruction.instructions().size() == 1 : "Nested instruction count must round-trip";
    assert instruction.instructions().get(0).equals(nested) : "Nested instruction must round-trip";
    final RegisterPipelineTriggerInstruction.PipelineFilter expectedFilter =
        RegisterPipelineTriggerInstruction.PipelineFilter.transaction()
            .setTransactionHash(sampleHash)
            .setTransactionBlockHeight(42L)
            .setTransactionStatus("Approved");
    assert instruction.filter().equals(expectedFilter) : "Pipeline filter must round-trip";
    assert instruction.repeats() != null && instruction.repeats() == 5
        : "Repeats must round-trip";
    assert instruction.metadata().equals(metadata) : "Metadata must round-trip";
    assert "Transaction".equals(decodedBox.arguments().get("filter.kind"))
        : "Filter kind must persist";
    assert sampleHash.equals(decodedBox.arguments().get("filter.transaction.hash"))
        : "Filter hash must persist";
    assert "42".equals(decodedBox.arguments().get("filter.transaction.block_height"))
        : "Block height must remain canonical";
    assert "Approved".equals(decodedBox.arguments().get("filter.transaction.status"))
        : "Transaction status must remain canonical";
    assert "5".equals(decodedBox.arguments().get("repeats"))
        : "Repeats argument must persist canonical form";
  }

  private static void javaCodecHydratesRegisterPrecommitTriggerInstruction() throws NoritoException {
    final InstructionBox nested =
        InstructionBuilders.grantPermission("bob@wonderland", "CanSubmitTransactions");
    final Map<String, String> metadata = mapOf("purpose", "guard");
    final InstructionBox register =
        InstructionBuilders.registerPrecommitTrigger(
            "pc_guard",
            "carol@wonderland",
            listOf(nested),
            null,
            metadata);

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("0000001D")
            .setAuthority("carol@wonderland")
            .setCreationTimeMs(1_735_003_100_000L)
            .setExecutable(Executable.instructions(listOf(register)))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final byte[] encodedBytes =
        emitFixtureMetadata(
            "register_precommit_trigger_demo",
            payload,
            adapter,
            "register-precommit-trigger-fixture");
    final TransactionPayload decoded = adapter.decodeTransaction(encodedBytes);

    final InstructionBox decodedBox = decoded.executable().instructions().get(0);
    assert decodedBox.payload() instanceof RegisterPrecommitTriggerInstruction
        : "RegisterPrecommitTrigger instruction should hydrate";
    final RegisterPrecommitTriggerInstruction instruction =
        (RegisterPrecommitTriggerInstruction) decodedBox.payload();
    assert "pc_guard".equals(instruction.triggerId()) : "Trigger id must round-trip";
    assert "carol@wonderland".equals(instruction.authority()) : "Authority must round-trip";
    assert instruction.repeats() == null : "Pre-commit triggers default to indefinite repeats";
    assert instruction.metadata().equals(metadata) : "Metadata must round-trip";
    assert instruction.instructions().size() == 1 : "Nested instruction count must round-trip";
    assert instruction.instructions().get(0).equals(nested) : "Nested instruction must round-trip";
    assert "indefinite".equals(decodedBox.arguments().get("repeats"))
        : "Repeats argument should default to 'indefinite'";
  }

  private static void javaCodecHydratesKaigiInstructions() throws NoritoException {
    final byte[] commitmentBytes = new byte[32];
    final byte[] nullifierBytes = new byte[32];
    final byte[] rosterRootBytes = new byte[32];
    final byte[] usageCommitmentBytes = new byte[32];
    final byte[] relayHpkeKey = new byte[32];
    final byte[] proofBytes = new byte[] {0x11, 0x23, 0x45};
    final byte[] usageProofBytes = new byte[] {0x55, 0x66};

    Arrays.fill(commitmentBytes, (byte) 0x01);
    Arrays.fill(nullifierBytes, (byte) 0x02);
    Arrays.fill(rosterRootBytes, (byte) 0x03);
    Arrays.fill(usageCommitmentBytes, (byte) 0x04);
    Arrays.fill(relayHpkeKey, (byte) 0x05);

    final CreateKaigiInstruction createInstruction =
        CreateKaigiInstruction.builder()
            .setCallId("wonderland", "weekly-sync")
            .setHost("ed0120HOST@wonderland")
            .setTitle("Weekly Sync")
            .setDescription("Roadmap alignment and status updates")
            .setMaxParticipants(16)
            .setGasRatePerMinute(120)
            .putMetadata("topic", "status")
            .setScheduledStartMs(1_700_000_000_000L)
            .setBillingAccount("ed0120BILL@wonderland")
            .setPrivacyMode("ZkRosterV1")
            .setRelayManifestExpiryMs(1_700_111_000_000L)
            .addRelayManifestHop(
                "relay-alpha@wonderland",
                Base64.getEncoder().encodeToString(relayHpkeKey),
                5)
            .build();

    final JoinKaigiInstruction joinInstruction =
        JoinKaigiInstruction.builder()
            .setCallId("wonderland", "weekly-sync")
            .setParticipant("ed0120PARTICIPANT@wonderland")
            .setCommitment(commitmentBytes)
            .setCommitmentAliasTag("guest")
            .setNullifierDigest(nullifierBytes)
            .setNullifierIssuedAtMs(99L)
            .setRosterRoot(rosterRootBytes)
            .setProof(proofBytes)
            .build();

    final LeaveKaigiInstruction leaveInstruction =
        LeaveKaigiInstruction.builder()
            .setCallId("wonderland", "weekly-sync")
            .setParticipant("ed0120PARTICIPANT@wonderland")
            .build();

    final EndKaigiInstruction endInstruction =
        EndKaigiInstruction.builder()
            .setCallId("wonderland", "weekly-sync")
            .setEndedAtMs(1_700_000_360_000L)
            .build();

    final RecordKaigiUsageInstruction usageInstruction =
        RecordKaigiUsageInstruction.builder()
            .setCallId("wonderland", "weekly-sync")
            .setDurationMs(600_000L)
            .setBilledGas(42L)
            .setUsageCommitment(usageCommitmentBytes)
            .setProof(usageProofBytes)
            .build();

    final SetKaigiRelayManifestInstruction manifestInstruction =
        SetKaigiRelayManifestInstruction.builder()
            .setCallId("wonderland", "weekly-sync")
            .setRelayManifestExpiryMs(1_700_222_000_000L)
            .addRelayManifestHop(
                "relay-bravo@wonderland",
                Base64.getEncoder().encodeToString(relayHpkeKey),
                7)
            .build();

    final RegisterKaigiRelayInstruction registerRelayInstruction =
        RegisterKaigiRelayInstruction.builder()
            .setRelayId("relay-bravo@wonderland")
            .setHpkePublicKey(relayHpkeKey)
            .setBandwidthClass(9)
            .build();

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("0000001E")
            .setAuthority("ed0120HOST@wonderland")
            .setExecutable(
                Executable.instructions(
                    listOf(
                        InstructionBox.of(createInstruction),
                        InstructionBox.of(joinInstruction),
                        InstructionBox.of(leaveInstruction),
                        InstructionBox.of(endInstruction),
                        InstructionBox.of(usageInstruction),
                        InstructionBox.of(manifestInstruction),
                        InstructionBox.of(registerRelayInstruction))))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final byte[] encoded = adapter.encodeTransaction(payload);
    final TransactionPayload decoded = adapter.decodeTransaction(encoded);

    final List<InstructionBox> instructions = decoded.executable().instructions();
    assert instructions.size() == 7 : "Expected seven Kaigi instructions";

    final InstructionBox createBox = instructions.get(0);
    assert createBox.payload() instanceof CreateKaigiInstruction
        : "CreateKaigi instruction should hydrate";
    final CreateKaigiInstruction decodedCreate = (CreateKaigiInstruction) createBox.payload();
    assert createInstruction.equals(decodedCreate) : "CreateKaigi fields must round-trip";

    final InstructionBox joinBox = instructions.get(1);
    assert joinBox.payload() instanceof JoinKaigiInstruction : "JoinKaigi should hydrate";
    final JoinKaigiInstruction decodedJoin = (JoinKaigiInstruction) joinBox.payload();
    assert joinInstruction.equals(decodedJoin) : "JoinKaigi fields must round-trip";

    final InstructionBox leaveBox = instructions.get(2);
    assert leaveBox.payload() instanceof LeaveKaigiInstruction : "LeaveKaigi should hydrate";
    final LeaveKaigiInstruction decodedLeave = (LeaveKaigiInstruction) leaveBox.payload();
    assert leaveInstruction.equals(decodedLeave) : "LeaveKaigi fields must round-trip";

    final InstructionBox endBox = instructions.get(3);
    assert endBox.payload() instanceof EndKaigiInstruction : "EndKaigi should hydrate";
    final EndKaigiInstruction decodedEnd = (EndKaigiInstruction) endBox.payload();
    assert endInstruction.equals(decodedEnd) : "EndKaigi fields must round-trip";

    final InstructionBox usageBox = instructions.get(4);
    assert usageBox.payload() instanceof RecordKaigiUsageInstruction
        : "RecordKaigiUsage should hydrate";
    final RecordKaigiUsageInstruction decodedUsage =
        (RecordKaigiUsageInstruction) usageBox.payload();
    assert usageInstruction.equals(decodedUsage) : "RecordKaigiUsage fields must round-trip";

    final InstructionBox manifestBox = instructions.get(5);
    assert manifestBox.payload() instanceof SetKaigiRelayManifestInstruction
        : "SetKaigiRelayManifest should hydrate";
    final SetKaigiRelayManifestInstruction decodedManifest =
        (SetKaigiRelayManifestInstruction) manifestBox.payload();
    assert manifestInstruction.equals(decodedManifest)
        : "SetKaigiRelayManifest fields must round-trip";

    final InstructionBox registerBox = instructions.get(6);
    assert registerBox.payload() instanceof RegisterKaigiRelayInstruction
        : "RegisterKaigiRelay should hydrate";
    final RegisterKaigiRelayInstruction decodedRegister =
        (RegisterKaigiRelayInstruction) registerBox.payload();
    assert registerRelayInstruction.equals(decodedRegister)
        : "RegisterKaigiRelay fields must round-trip";
  }
 
  private static void javaCodecHydratesVerifyingKeyInstructions() throws NoritoException {
    final byte[] registerVkBytes = new byte[] {0x01, 0x02, 0x03, 0x04};
    final byte[] updateVkBytes = new byte[] {0x05, 0x06, 0x07, 0x08, 0x09};
    final String schemaHashHex =
        "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    final VerifyingKeyRecordDescription registerRecord =
        VerifyingKeyRecordDescription.builder()
            .setVersion(1)
            .setCircuitId("halo2/ipa::transfer_v1")
            .setBackendTag(VerifyingKeyBackendTag.HALO2_IPA_PASTA)
            .setCurve("pallas")
            .setSchemaHashHex(schemaHashHex)
            .setInlineKeyBytes(registerVkBytes)
            .setGasScheduleId("halo2_default")
            .setMaxProofBytes(4096)
            .setMetadataUriCid("ipfs://vk_meta")
            .setVkBytesCid("ipfs://vk_bytes")
            .setActivationHeight(10L)
            .setStatus(VerifyingKeyStatus.ACTIVE)
            .build("halo2/ipa");

    final RegisterVerifyingKeyInstruction registerInstruction =
        RegisterVerifyingKeyInstruction.builder()
            .setBackend("halo2/ipa")
            .setName("vk_main")
            .setRecord(registerRecord)
            .build();

    final VerifyingKeyRecordDescription updateRecord =
        VerifyingKeyRecordDescription.builder()
            .setVersion(2)
            .setCircuitId("halo2/ipa::transfer_v1")
            .setBackendTag(VerifyingKeyBackendTag.HALO2_IPA_PASTA)
            .setCurve("pallas")
            .setSchemaHashHex(schemaHashHex)
            .setInlineKeyBytes(updateVkBytes)
            .setGasScheduleId("halo2_patch_1")
            .setMaxProofBytes(5000)
            .setStatus(VerifyingKeyStatus.WITHDRAWN)
            .build("halo2/ipa");

    final UpdateVerifyingKeyInstruction updateInstruction =
        UpdateVerifyingKeyInstruction.builder()
            .setBackend("halo2/ipa")
            .setName("vk_main")
            .setRecord(updateRecord)
            .build();

    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("0000002A")
            .setAuthority("ed0120VK@wonderland")
            .setExecutable(
                Executable.instructions(
                    listOf(
                        InstructionBox.of(registerInstruction),
                        InstructionBox.of(updateInstruction))))
            .build();

    final NoritoJavaCodecAdapter adapter = new NoritoJavaCodecAdapter();
    final byte[] encoded = adapter.encodeTransaction(payload);
    final TransactionPayload decoded = adapter.decodeTransaction(encoded);

    final List<InstructionBox> instructions = decoded.executable().instructions();
    assert instructions.size() == 2 : "Expected verifying key instruction pair";

    final InstructionBox registerBox = instructions.get(0);
    assert registerBox.payload() instanceof RegisterVerifyingKeyInstruction
        : "RegisterVerifyingKey should hydrate";
    final RegisterVerifyingKeyInstruction decodedRegister =
        (RegisterVerifyingKeyInstruction) registerBox.payload();
    assert registerInstruction.equals(decodedRegister)
        : "RegisterVerifyingKey fields must round-trip";

    final InstructionBox updateBox = instructions.get(1);
    assert updateBox.payload() instanceof UpdateVerifyingKeyInstruction
        : "UpdateVerifyingKey should hydrate";
    final UpdateVerifyingKeyInstruction decodedUpdate =
        (UpdateVerifyingKeyInstruction) updateBox.payload();
    assert updateInstruction.equals(decodedUpdate)
        : "UpdateVerifyingKey fields must round-trip";

  }

  private static byte[] readField(final NoritoDecoder decoder, final String field) {
    final long length = decoder.readLength(decoder.compactLenActive());
    if (length > Integer.MAX_VALUE) {
      throw new IllegalArgumentException(field + " length too large: " + length);
    }
    return decoder.readBytes((int) length);
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

  private static <T> T decodeFieldPayload(
      final byte[] payload, final TypeAdapter<T> adapter, final String field) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, NoritoHeader.MINOR_VERSION);
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
      final IrohaKeyManager keyManager = IrohaKeyManager.withSoftwareFallback();
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
}

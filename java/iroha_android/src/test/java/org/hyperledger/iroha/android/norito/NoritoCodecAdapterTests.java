package org.hyperledger.iroha.android.norito;

import java.util.Arrays;
import java.util.Base64;
import java.nio.charset.StandardCharsets;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.IrohaKeyManager;
import org.hyperledger.iroha.android.KeyManagementException;
import org.hyperledger.iroha.android.model.Executable;
import org.hyperledger.iroha.android.model.InstructionBox;
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
    javaCodecRejectsLegacyCanonicalAuthorityIdentifier();
    javaCodecRejectsNestedDomainAuthorityIdentifier();
    javaCodecEncodesMultisigSignatures();
    javaCodecEncodesChainIdLayout();
    javaCodecSupportsInstructionsVariant();
    javaCodecSupportsWireInstructionPayloads();
    javaCodecEncodesIvmBytecodeLayout();
    javaCodecEncodesInstructionLayout();
    System.out.println("[IrohaAndroid] Norito codec scaffolding tests passed.");
  }

  private static void javaCodecRoundTripsPayload() throws NoritoException {
    final byte[] instructions = "android-instructions".getBytes();
    final String authority = sampleAuthority(0x10);
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000001")
            .setAuthority(authority)
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
    assert decoded.authority().equals(authority) : "Authority must round-trip";
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
          AccountAddress.fromAccount(publicKey, "ed25519")
              .toIH58(AccountAddress.DEFAULT_IH58_PREFIX);
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalStateException("Failed to build authority address", ex);
    }
    final String authority = ih58;
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
    final NoritoDecoder decoder = new NoritoDecoder(encoded, NoritoHeader.MINOR_VERSION);
    readField(decoder, "payload.chain_id");
    final byte[] authorityField = readField(decoder, "payload.authority");
    final String encodedAuthority =
        decodeFieldPayload(authorityField, NoritoAdapters.stringAdapter(), "payload.authority");
    assert authority.equals(encodedAuthority)
        : "AccountId authority should be encoded as a normalized string";
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
    final String ih58;
    try {
      ih58 =
          AccountAddress.fromMultisigPolicy(policy)
              .toIH58(AccountAddress.DEFAULT_IH58_PREFIX);
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalStateException("Failed to build multisig authority address", ex);
    }
    final String authority = ih58;
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
    final NoritoDecoder decoder = new NoritoDecoder(encoded, NoritoHeader.MINOR_VERSION);
    readField(decoder, "payload.chain_id");
    final byte[] authorityField = readField(decoder, "payload.authority");
    final String encodedAuthority =
        decodeFieldPayload(authorityField, NoritoAdapters.stringAdapter(), "payload.authority");
    assert authority.equals(encodedAuthority)
        : "Multisig authority should be encoded as normalized string";
  }

  private static void javaCodecRejectsLegacyCanonicalAuthorityIdentifier() throws NoritoException {
    final byte[] publicKey = new byte[32];
    Arrays.fill(publicKey, (byte) 0x6C);
    final String canonical;
    try {
      canonical = AccountAddress.fromAccount(publicKey, "ed25519").canonicalHex();
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalStateException("Failed to build canonical authority address", ex);
    }
    try {
      TransactionPayload.builder()
          .setChainId("00000002")
          .setAuthority(canonical)
          .setCreationTimeMs(1_735_000_000_456L)
          .setExecutable(Executable.ivm(new byte[] {0x01}))
          .build();
      throw new AssertionError("expected canonical authority identifier rejection");
    } catch (final IllegalArgumentException expected) {
      assert expected.getMessage().contains("IH58 or compressed sora encoded")
          : "unexpected error message: " + expected.getMessage();
    }
  }

  private static void javaCodecRejectsNestedDomainAuthorityIdentifier() throws NoritoException {
    final byte[] publicKey = new byte[32];
    Arrays.fill(publicKey, (byte) 0x7D);
    final String ih58;
    try {
      ih58 =
          AccountAddress.fromAccount(publicKey, "ed25519")
              .toIH58(AccountAddress.DEFAULT_IH58_PREFIX);
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalStateException("Failed to build authority address", ex);
    }
    try {
      TransactionPayload.builder()
          .setChainId("00000002")
          .setAuthority(ih58 + "@wonderland@fallback")
          .setCreationTimeMs(1_735_000_000_456L)
          .setExecutable(Executable.ivm(new byte[] {0x01}))
          .build();
      throw new AssertionError("expected nested-domain authority identifier rejection");
    } catch (final IllegalArgumentException expected) {
      assert expected.getMessage().contains("must not include @domain suffix")
          : "unexpected error message: " + expected.getMessage();
    }
  }

  private static void javaCodecEncodesMultisigSignatures() throws NoritoException {
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000003")
            .setAuthority(sampleAuthority(0x31))
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

    final NoritoDecoder decoder = new NoritoDecoder(encodedSigned, NoritoHeader.MINOR_VERSION);
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
        new NoritoDecoder(multisigPayload, NoritoHeader.MINOR_VERSION);
    final long count = multisigDecoder.readLength(false);
    assert count == 2 : "Expected two multisig signatures";
    final boolean compact = multisigDecoder.compactLenActive();
    final byte[] firstPayload = readSequenceElement(multisigDecoder, compact, "multisig[0]");
    final NoritoDecoder firstDecoder = new NoritoDecoder(firstPayload, NoritoHeader.MINOR_VERSION);
    assertMultisigSignaturePayload(firstDecoder, sigA, "multisig[0]");
    assert firstDecoder.remaining() == 0 : "multisig[0] payload should not have trailing bytes";

    final byte[] secondPayload = readSequenceElement(multisigDecoder, compact, "multisig[1]");
    final NoritoDecoder secondDecoder = new NoritoDecoder(secondPayload, NoritoHeader.MINOR_VERSION);
    assertMultisigSignaturePayload(secondDecoder, sigB, "multisig[1]");
    assert secondDecoder.remaining() == 0 : "multisig[1] payload should not have trailing bytes";
    assert multisigDecoder.remaining() == 0 : "Multisig payload should not have trailing bytes";
  }

  private static void javaCodecEncodesChainIdLayout() throws NoritoException {
    final String chainId = "00000003";
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId(chainId)
            .setAuthority(sampleAuthority(0x41))
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
    final byte[] wirePayloadA =
        NoritoCodec.encode("wire-A", "iroha.test.WirePayload", NoritoAdapters.stringAdapter());
    final byte[] wirePayloadB =
        NoritoCodec.encode("wire-B", "iroha.test.WirePayload", NoritoAdapters.stringAdapter());
    final TransactionPayload payload =
        TransactionPayload.builder()
            .setChainId("00000009")
            .setAuthority(sampleAuthority(0x51))
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
            .setAuthority(sampleAuthority(0x61))
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
            .setAuthority(sampleAuthority(0x71))
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
            .setAuthority(sampleAuthority(0x81))
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

  private static int assertMultisigMember(
      final byte[] payload,
      final int offset,
      final String expectedPublicKey,
      final int expectedWeight,
      final String label) {
    final long memberLen = readU64(payload, offset, "authority.controller.policy." + label);
    final int memberOffset = offset + 8;
    final long publicKeyLen =
        readU64(payload, memberOffset, "authority.controller.policy." + label + ".public_key");
    final int publicKeyOffset = memberOffset + 8;
    final int publicKeySize = Math.toIntExact(publicKeyLen);
    final String publicKey =
        new String(payload, publicKeyOffset, publicKeySize, StandardCharsets.UTF_8);
    final int weightOffset = publicKeyOffset + publicKeySize;
    final int weight = readU16(payload, weightOffset, "authority.controller.policy." + label + ".weight");
    assert expectedPublicKey.equals(publicKey) : label + " public key must round-trip";
    assert weight == expectedWeight : label + " weight must round-trip";
    final int expectedLen = 8 + publicKeySize + 2;
    assert memberLen == expectedLen : label + " payload size mismatch";
    return memberOffset + Math.toIntExact(memberLen);
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

  private static byte[] fill(final int value, final int length) {
    final byte[] out = new byte[length];
    Arrays.fill(out, (byte) value);
    return out;
  }

  private static String sampleAuthority(final int fillByte) {
    final byte[] publicKey = new byte[32];
    Arrays.fill(publicKey, (byte) fillByte);
    try {
      return AccountAddress.fromAccount(publicKey, "ed25519")
          .toIH58(AccountAddress.DEFAULT_IH58_PREFIX);
    } catch (final AccountAddress.AccountAddressException ex) {
      throw new IllegalStateException("Failed to build sample authority", ex);
    }
  }

  private static java.util.Optional<byte[]> decodeOptionPayload(
      final byte[] payload, final String field) {
    final NoritoDecoder decoder = new NoritoDecoder(payload, NoritoHeader.MINOR_VERSION);
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

package org.hyperledger.iroha.android.model.instructions;

import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import org.hyperledger.iroha.android.client.IdentifierReceiptAttestation;
import org.hyperledger.iroha.android.client.IdentifierReceiptCanonicalEncoder;
import org.hyperledger.iroha.android.client.IdentifierResolutionExecutionPayload;
import org.hyperledger.iroha.android.client.IdentifierResolutionPayload;
import org.hyperledger.iroha.android.client.IdentifierResolutionReceipt;
import org.hyperledger.iroha.android.client.RamLfeOutputOpening;
import org.hyperledger.iroha.android.client.RamLfeOutputOpeningPayload;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoHeader;

public final class ClaimIdentifierWirePayloadEncoderTests {
  private static final String ACCOUNT_ID = canonicalAccountId();
  private static final String PARITY_ACCOUNT_MULTIHASH =
      "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03";
  private static final String PARITY_SIGNATURE_HEX = "CD".repeat(64);
  private static final String PARITY_HASH_HEX =
      "C6A23EC291940DF33EF948BCE1DF0FC42B8108661529A4E4CD6E084D3BD944C3";
  private static final String PARITY_RUST_BARE_HEX =
      "4C00000000474665643031323043453746413436433944434537454134423132354532453336424442363345413333303733453735393041433932383136414531453836314237303438423033DA03C8020D060570686F6E6505046531363486010D0C0B7061726974795F7465737420C6A23EC291940DF33EF948BCE1DF0FC42B8108661529A4E4CD6E084D3BD944C30400000000040000000020C6A23EC291940DF33EF948BCE1DF0FC42B8108661529A4E4CD6E084D3BD944C320C6A23EC291940DF33EF948BCE1DF0FC42B8108661529A4E4CD6E084D3BD944C30800060FF69301000001002120C6A23EC291940DF33EF948BCE1DF0FC42B8108661529A4E4CD6E084D3BD944C320C6A23EC291940DF33EF948BCE1DF0FC42B8108661529A4E4CD6E084D3BD944C32120C6A23EC291940DF33EF948BCE1DF0FC42B8108661529A4E4CD6E084D3BD944C34C000000004746656430313230434537464134364339444345374541344231323545324533364244423633454133333037334537353930414339323831364145314538363142373034384230338E01000000008801400000000000000001CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD";
  private static final String PARITY_RUST_FRAMED_HEX =
      "4E525430000020EF6431870B986820EF6431870B9868002902000000000000A4872DE026608AA0024C00000000474665643031323043453746413436433944434537454134423132354532453336424442363345413333303733453735393041433932383136414531453836314237303438423033DA03C8020D060570686F6E6505046531363486010D0C0B7061726974795F7465737420C6A23EC291940DF33EF948BCE1DF0FC42B8108661529A4E4CD6E084D3BD944C30400000000040000000020C6A23EC291940DF33EF948BCE1DF0FC42B8108661529A4E4CD6E084D3BD944C320C6A23EC291940DF33EF948BCE1DF0FC42B8108661529A4E4CD6E084D3BD944C30800060FF69301000001002120C6A23EC291940DF33EF948BCE1DF0FC42B8108661529A4E4CD6E084D3BD944C320C6A23EC291940DF33EF948BCE1DF0FC42B8108661529A4E4CD6E084D3BD944C32120C6A23EC291940DF33EF948BCE1DF0FC42B8108661529A4E4CD6E084D3BD944C34C000000004746656430313230434537464134364339444345374541344231323545324533364244423633454133333037334537353930414339323831364145314538363142373034384230338E01000000008801400000000000000001CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD";

  public static void main(String[] args) {
    claimIdentifierEncodesExpectedWirePayload();
    claimIdentifierMatchesRustCanonicalFixture();
    printClaimIdentifierWirePayloadHex();
  }

  private static void claimIdentifierEncodesExpectedWirePayload() {
    final String signatureHex = "A1B2C3D4";
    final IdentifierResolutionPayload payload =
        new IdentifierResolutionPayload(
            "phone#retail",
            new IdentifierResolutionExecutionPayload(
                "identifier_lookup_retail",
                "11".repeat(32),
                "bfv-affine-sha3-256-v1",
                "signed",
                "AA".repeat(32),
                "BB".repeat(32),
                "CC".repeat(32),
                "DD".repeat(32),
                "22".repeat(32),
                "33".repeat(32),
                42L,
                142L),
            sampleOpening("identifier_lookup_retail", signatureHex),
            "opaque:" + "44".repeat(32),
            "55".repeat(32),
            "uaid:" + "66".repeat(32),
            ACCOUNT_ID);
    final IdentifierResolutionReceipt receipt =
        new IdentifierResolutionReceipt(
            payload,
            new IdentifierReceiptAttestation("signed", signatureHex, null, null));

    final InstructionBox instruction = ClaimIdentifierWirePayloadEncoder.encode(ACCOUNT_ID, receipt);
    assert ClaimIdentifierWirePayloadEncoder.WIRE_NAME.equals(instruction.name())
        : "ClaimIdentifier wire name mismatch";
    assert instruction.payload() instanceof InstructionBox.WirePayload
        : "ClaimIdentifier must use a wire payload";

    final InstructionBox.WirePayload wirePayload = (InstructionBox.WirePayload) instruction.payload();
    final NoritoHeader.DecodeResult decoded =
        NoritoHeader.decode(wirePayload.payloadBytes(), null);
    decoded.header().validateChecksum(decoded.payload());

    final NoritoDecoder claimDecoder = new NoritoDecoder(decoded.payload(), decoded.header().flags(), 0);
    final byte[] encodedAccount = readSizedField(claimDecoder);
    final byte[] encodedReceipt = readSizedField(claimDecoder);
    assert claimDecoder.remaining() == 0 : "ClaimIdentifier must not contain trailing bytes";
    assert java.util.Arrays.equals(
            encodedAccount, TransferWirePayloadEncoder.encodeAccountIdPayload(ACCOUNT_ID))
        : "AccountId field mismatch";

    final NoritoDecoder receiptDecoder = new NoritoDecoder(encodedReceipt, decoded.header().flags(), 0);
    final byte[] embeddedPayload = readSizedField(receiptDecoder);
    final byte[] embeddedAttestation = readSizedField(receiptDecoder);
    assert receiptDecoder.remaining() == 0 : "Receipt payload must not contain trailing bytes";
    assert java.util.Arrays.equals(
            embeddedPayload, IdentifierReceiptCanonicalEncoder.encodePayload(receipt.payload()))
        : "Receipt payload bytes mismatch";
    assert java.util.Arrays.equals(
            embeddedAttestation,
            IdentifierReceiptCanonicalEncoder.encodeAttestation(receipt.attestation()))
        : "Receipt attestation bytes mismatch";
  }

  private static void printClaimIdentifierWirePayloadHex() {
    final String signatureHex = "AB".repeat(64);
    final IdentifierResolutionPayload payload =
        new IdentifierResolutionPayload(
            "email#retail",
            new IdentifierResolutionExecutionPayload(
                "email_retail",
                "11".repeat(32),
                "bfv-affine-sha3-256-v1",
                "signed",
                "AA".repeat(32),
                "BB".repeat(32),
                "CC".repeat(32),
                "DD".repeat(32),
                "22".repeat(32),
                "33".repeat(32),
                42L,
                142L),
            sampleOpening("email_retail", signatureHex),
            "opaque:" + "44".repeat(32),
            "55".repeat(32),
            "uaid:" + "66".repeat(32),
            ACCOUNT_ID);
    final IdentifierResolutionReceipt receipt =
        new IdentifierResolutionReceipt(
            payload,
            new IdentifierReceiptAttestation("signed", signatureHex, null, null));

    final InstructionBox instruction = ClaimIdentifierWirePayloadEncoder.encode(ACCOUNT_ID, receipt);
    final InstructionBox.WirePayload wirePayload = (InstructionBox.WirePayload) instruction.payload();
    final NoritoHeader.DecodeResult decoded =
        NoritoHeader.decode(wirePayload.payloadBytes(), null);
    decoded.header().validateChecksum(decoded.payload());

    System.out.println("JAVA_CLAIM_WIRE_NAME=" + instruction.name());
    System.out.println("JAVA_CLAIM_BARE_HEX=" + toHex(decoded.payload()));
    System.out.println("JAVA_CLAIM_FRAMED_HEX=" + toHex(wirePayload.payloadBytes()));
  }

  private static void claimIdentifierMatchesRustCanonicalFixture() {
    final String liveAccountId = canonicalI105AccountId(PARITY_ACCOUNT_MULTIHASH);
    final IdentifierResolutionPayload payload =
        new IdentifierResolutionPayload(
            "phone#e164",
            new IdentifierResolutionExecutionPayload(
                "parity_test",
                PARITY_HASH_HEX,
                "hkdf-sha3-512-prf-v1",
                "signed",
                PARITY_HASH_HEX,
                PARITY_HASH_HEX,
                PARITY_HASH_HEX,
                PARITY_HASH_HEX,
                PARITY_HASH_HEX,
                PARITY_HASH_HEX,
                1_735_000_000_000L,
                null),
            sampleOpening("parity_test", PARITY_SIGNATURE_HEX, PARITY_HASH_HEX),
            "opaque:" + PARITY_HASH_HEX,
            PARITY_HASH_HEX,
            "uaid:" + PARITY_HASH_HEX,
            liveAccountId);
    final IdentifierResolutionReceipt receipt =
        new IdentifierResolutionReceipt(
            payload,
            new IdentifierReceiptAttestation("signed", PARITY_SIGNATURE_HEX, null, null));

    final InstructionBox instruction =
        ClaimIdentifierWirePayloadEncoder.encode(liveAccountId, receipt);
    final InstructionBox.WirePayload wirePayload = (InstructionBox.WirePayload) instruction.payload();
    final NoritoHeader.DecodeResult decoded =
        NoritoHeader.decode(wirePayload.payloadBytes(), null);
    decoded.header().validateChecksum(decoded.payload());

    assert ClaimIdentifierWirePayloadEncoder.WIRE_NAME.equals(instruction.name())
        : "ClaimIdentifier parity wire name mismatch";
    final String actualBareHex = toHex(decoded.payload());
    final String actualFramedHex = toHex(wirePayload.payloadBytes());
    assert parityRustBareHex().equals(actualBareHex)
        : "ClaimIdentifier parity bare payload drifted from Rust\nexpected="
            + parityRustBareHex()
            + "\nactual="
            + actualBareHex;
    assert parityRustFramedHex().equals(actualFramedHex)
        : "ClaimIdentifier parity framed payload drifted from Rust\nexpected="
            + parityRustFramedHex()
            + "\nactual="
            + actualFramedHex;
  }

  private static String parityRustBareHex() {
    return "4F000000004A2100000000000000010001CE017F01A4016C019D01CE017E01A401B1012501E201E3016B01DB016301EA"
        + "01330107013E0175019001AC01920181016A01E101E8016101B70104018B0103DD03CB020D060570686F6E6505046531"
        + "363486010D0C0B7061726974795F7465737420C6A23EC291940DF33EF948BCE1DF0FC42B8108661529A4E4CD6E084D3B"
        + "D944C30400000000040000000020C6A23EC291940DF33EF948BCE1DF0FC42B8108661529A4E4CD6E084D3BD944C320C6"
        + "A23EC291940DF33EF948BCE1DF0FC42B8108661529A4E4CD6E084D3BD944C30800060FF69301000001002120C6A23EC2"
        + "91940DF33EF948BCE1DF0FC42B8108661529A4E4CD6E084D3BD944C320C6A23EC291940DF33EF948BCE1DF0FC42B8108"
        + "661529A4E4CD6E084D3BD944C32120C6A23EC291940DF33EF948BCE1DF0FC42B8108661529A4E4CD6E084D3BD944C34F"
        + "000000004A2100000000000000010001CE017F01A4016C019D01CE017E01A401B1012501E201E3016B01DB016301EA01"
        + "330107013E0175019001AC01920181016A01E101E8016101B70104018B01038E01000000008801400000000000000001"
        + "CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01"
        + "CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01"
        + "CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD";
  }

  private static String parityRustFramedHex() {
    return "4E5254300000D7895D1E8DAC6978699D9011F54021FB002F020000000000008BCC60A66DEAC9CB024F000000004A2100"
        + "000000000000010001CE017F01A4016C019D01CE017E01A401B1012501E201E3016B01DB016301EA01330107013E0175"
        + "019001AC01920181016A01E101E8016101B70104018B0103DD03CB020D060570686F6E6505046531363486010D0C0B70"
        + "61726974795F7465737420C6A23EC291940DF33EF948BCE1DF0FC42B8108661529A4E4CD6E084D3BD944C30400000000"
        + "040000000020C6A23EC291940DF33EF948BCE1DF0FC42B8108661529A4E4CD6E084D3BD944C320C6A23EC291940DF33E"
        + "F948BCE1DF0FC42B8108661529A4E4CD6E084D3BD944C30800060FF69301000001002120C6A23EC291940DF33EF948BC"
        + "E1DF0FC42B8108661529A4E4CD6E084D3BD944C320C6A23EC291940DF33EF948BCE1DF0FC42B8108661529A4E4CD6E08"
        + "4D3BD944C32120C6A23EC291940DF33EF948BCE1DF0FC42B8108661529A4E4CD6E084D3BD944C34F000000004A210000"
        + "0000000000010001CE017F01A4016C019D01CE017E01A401B1012501E201E3016B01DB016301EA01330107013E017501"
        + "9001AC01920181016A01E101E8016101B70104018B01038E01000000008801400000000000000001CD01CD01CD01CD01"
        + "CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01"
        + "CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01"
        + "CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD01CD";
  }

  private static RamLfeOutputOpening sampleOpening(
      final String programId, final String signatureHex) {
    return sampleOpening(programId, signatureHex, "EE".repeat(32));
  }

  private static RamLfeOutputOpening sampleOpening(
      final String programId, final String signatureHex, final String hashHex) {
    return new RamLfeOutputOpening(
        new RamLfeOutputOpeningPayload(
            programId,
            hashHex,
            hashHex,
            hashHex,
            hashHex,
            hashHex,
            1_735_000_000_000L,
            null),
        signatureHex);
  }

  private static byte[] readSizedField(final NoritoDecoder decoder) {
    final long length = decoder.readLength(decoder.compactLenActive());
    return decoder.readBytes(Math.toIntExact(length));
  }

  private static String toHex(final byte[] value) {
    final StringBuilder out = new StringBuilder(value.length * 2);
    for (final byte current : value) {
      out.append(String.format("%02X", current));
    }
    return out.toString();
  }

  private static String canonicalAccountId() {
    return canonicalI105AccountId(
        "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03");
  }

  private static String canonicalI105AccountId(final String multihash) {
    final PublicKeyCodec.PublicKeyPayload payload =
        PublicKeyCodec.decodePublicKeyLiteral(multihash);
    if (payload == null) {
      throw new IllegalStateException("expected valid ED25519 fixture");
    }
    try {
      return AccountAddress.fromAccount(
              payload.keyBytes(), PublicKeyCodec.algorithmForCurveId(payload.curveId()))
          .toI105(AccountAddress.DEFAULT_I105_DISCRIMINANT);
    } catch (final Exception ex) {
      throw new IllegalStateException("failed to build canonical account fixture", ex);
    }
  }
}

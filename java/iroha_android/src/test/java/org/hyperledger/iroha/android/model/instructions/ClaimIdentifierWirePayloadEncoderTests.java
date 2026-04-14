package org.hyperledger.iroha.android.model.instructions;

import org.hyperledger.iroha.android.address.AccountAddress;
import org.hyperledger.iroha.android.address.PublicKeyCodec;
import java.util.Optional;
import org.hyperledger.iroha.android.client.IdentifierResolutionPayload;
import org.hyperledger.iroha.android.client.IdentifierResolutionReceipt;
import org.hyperledger.iroha.android.model.InstructionBox;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoDecoder;
import org.hyperledger.iroha.norito.NoritoHeader;
import org.hyperledger.iroha.norito.TypeAdapter;

public final class ClaimIdentifierWirePayloadEncoderTests {
  private static final String ACCOUNT_ID = canonicalAccountId();
  private static final String LIVE_ACCOUNT_MULTIHASH =
      "ed01205634E9071E8662974A22F137972663C4644DC3546A1938E1CAC58DE4CBA8D965";
  private static final String LIVE_SIGNATURE_HEX =
      "9262CA8C755D47207ED0CD2E19892DFAA4612701A36DCAF87173D42CC754DFB6A66158856FDFD25974C2A11E9FC32940CA0DF18CAC25A38CB5DEDC4625E67900";
  private static final String LIVE_PAYLOAD_HEX =
      "2B000000000000000D000000000000000500000000000000656D61696C0E00000000000000060000000000000072657461696CDD000000000000001C0000000000000014000000000000000C00000000000000656D61696C5F72657461696C200000000000000075522459A6B0039705A18CE5D21050F39454F440203D6041C454658735DA1D070400000000000000020000000400000000000000000000002000000000000000E12E5429C9C8B146C4EC6DB972DCDEBD6BC84257A4503C0A152E052D345B303D2000000000000000444180A5ECCBC236041F1DC4D5E3BD220B0656261EB55B9D9AE32AA729B5437F0800000000000000987ABF0A9D0100001100000000000000010800000000000000780EC40A9D01000028000000000000002000000000000000D82F9EAB952F7A5241BB2339C0095EBC61958428164AB820FAD85952F35745852000000000000000032DF7E7370E04DDBABF0CD40932935A1D2C77A9B8D723BBB9F1472F2791CC7128000000000000002000000000000000C60973F731CCB57008687F9BC38CC712E3BE7AB46D99A1BEFFD1C9FD61E60A875A00000000000000000000004E00000000000000460000000000000065643031323035363334453930373145383636323937344132324631333739373236363343343634344443333534364131393338453143414335384445344342413844393635";
  private static final String LIVE_RUST_BARE_HEX =
      "5A00000000000000000000004E000000000000004600000000000000656430313230353633344539303731453836363239373441323246313337393732363633433436343444433335343641313933384531434143353844453443424138443936356C0400000000000002020000000000002B000000000000000D000000000000000500000000000000656D61696C0E00000000000000060000000000000072657461696CDD000000000000001C0000000000000014000000000000000C00000000000000656D61696C5F72657461696C200000000000000075522459A6B0039705A18CE5D21050F39454F440203D6041C454658735DA1D070400000000000000020000000400000000000000000000002000000000000000E12E5429C9C8B146C4EC6DB972DCDEBD6BC84257A4503C0A152E052D345B303D2000000000000000444180A5ECCBC236041F1DC4D5E3BD220B0656261EB55B9D9AE32AA729B5437F0800000000000000987ABF0A9D0100001100000000000000010800000000000000780EC40A9D01000028000000000000002000000000000000D82F9EAB952F7A5241BB2339C0095EBC61958428164AB820FAD85952F35745852000000000000000032DF7E7370E04DDBABF0CD40932935A1D2C77A9B8D723BBB9F1472F2791CC7128000000000000002000000000000000C60973F731CCB57008687F9BC38CC712E3BE7AB46D99A1BEFFD1C9FD61E60A875A00000000000000000000004E00000000000000460000000000000065643031323035363334453930373145383636323937344132324631333739373236363343343634344443333534364131393338453143414335384445344342413844393635510200000000000001480200000000000040000000000000000100000000000000920100000000000000620100000000000000CA01000000000000008C01000000000000007501000000000000005D01000000000000004701000000000000002001000000000000007E0100000000000000D00100000000000000CD01000000000000002E01000000000000001901000000000000008901000000000000002D0100000000000000FA0100000000000000A40100000000000000610100000000000000270100000000000000010100000000000000A301000000000000006D0100000000000000CA0100000000000000F80100000000000000710100000000000000730100000000000000D401000000000000002C0100000000000000C70100000000000000540100000000000000DF0100000000000000B60100000000000000A601000000000000006101000000000000005801000000000000008501000000000000006F0100000000000000DF0100000000000000D20100000000000000590100000000000000740100000000000000C20100000000000000A101000000000000001E01000000000000009F0100000000000000C30100000000000000290100000000000000400100000000000000CA01000000000000000D0100000000000000F101000000000000008C0100000000000000AC0100000000000000250100000000000000A301000000000000008C0100000000000000B50100000000000000DE0100000000000000DC0100000000000000460100000000000000250100000000000000E6010000000000000079010000000000000000010000000000000000";
  private static final String LIVE_RUST_FRAMED_HEX =
      "4E525430000020EF6431870B986820EF6431870B986800D604000000000000F0418260D4690AAD005A00000000000000000000004E000000000000004600000000000000656430313230353633344539303731453836363239373441323246313337393732363633433436343444433335343641313933384531434143353844453443424138443936356C0400000000000002020000000000002B000000000000000D000000000000000500000000000000656D61696C0E00000000000000060000000000000072657461696CDD000000000000001C0000000000000014000000000000000C00000000000000656D61696C5F72657461696C200000000000000075522459A6B0039705A18CE5D21050F39454F440203D6041C454658735DA1D070400000000000000020000000400000000000000000000002000000000000000E12E5429C9C8B146C4EC6DB972DCDEBD6BC84257A4503C0A152E052D345B303D2000000000000000444180A5ECCBC236041F1DC4D5E3BD220B0656261EB55B9D9AE32AA729B5437F0800000000000000987ABF0A9D0100001100000000000000010800000000000000780EC40A9D01000028000000000000002000000000000000D82F9EAB952F7A5241BB2339C0095EBC61958428164AB820FAD85952F35745852000000000000000032DF7E7370E04DDBABF0CD40932935A1D2C77A9B8D723BBB9F1472F2791CC7128000000000000002000000000000000C60973F731CCB57008687F9BC38CC712E3BE7AB46D99A1BEFFD1C9FD61E60A875A00000000000000000000004E00000000000000460000000000000065643031323035363334453930373145383636323937344132324631333739373236363343343634344443333534364131393338453143414335384445344342413844393635510200000000000001480200000000000040000000000000000100000000000000920100000000000000620100000000000000CA01000000000000008C01000000000000007501000000000000005D01000000000000004701000000000000002001000000000000007E0100000000000000D00100000000000000CD01000000000000002E01000000000000001901000000000000008901000000000000002D0100000000000000FA0100000000000000A40100000000000000610100000000000000270100000000000000010100000000000000A301000000000000006D0100000000000000CA0100000000000000F80100000000000000710100000000000000730100000000000000D401000000000000002C0100000000000000C70100000000000000540100000000000000DF0100000000000000B60100000000000000A601000000000000006101000000000000005801000000000000008501000000000000006F0100000000000000DF0100000000000000D20100000000000000590100000000000000740100000000000000C20100000000000000A101000000000000001E01000000000000009F0100000000000000C30100000000000000290100000000000000400100000000000000CA01000000000000000D0100000000000000F101000000000000008C0100000000000000AC0100000000000000250100000000000000A301000000000000008C0100000000000000B50100000000000000DE0100000000000000DC0100000000000000460100000000000000250100000000000000E6010000000000000079010000000000000000010000000000000000";

  private static final TypeAdapter<Optional<byte[]>> OPTIONAL_SIGNATURE_BYTES =
      NoritoAdapters.option(NoritoAdapters.byteVecAdapter());
  private static final TypeAdapter<Optional<byte[]>> OPTIONAL_PROOF_BYTES =
      NoritoAdapters.option(NoritoAdapters.rawByteVecAdapter());

  public static void main(String[] args) {
    claimIdentifierEncodesExpectedWirePayload();
    claimIdentifierMatchesRustCanonicalLiveFixture();
    printClaimIdentifierWirePayloadHex();
  }

  private static void claimIdentifierEncodesExpectedWirePayload() {
    final String payloadHex = "01020304A0";
    final String signatureHex = "A1B2C3D4";
    final IdentifierResolutionReceipt receipt =
        new IdentifierResolutionReceipt(
            "phone#retail",
            "opaque:" + "11".repeat(32),
            "22".repeat(32),
            "uaid:" + "33".repeat(31) + "35",
            ACCOUNT_ID,
            42L,
            142L,
            "bfv-affine-sha3-256-v1",
            signatureHex,
            payloadHex,
            new IdentifierResolutionPayload(
                "phone#retail",
                "opaque:" + "11".repeat(32),
                "22".repeat(32),
                "uaid:" + "33".repeat(31) + "35",
                ACCOUNT_ID,
                42L,
                142L));

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
    final Optional<byte[]> embeddedSignature =
        readSizedField(receiptDecoder, OPTIONAL_SIGNATURE_BYTES);
    final Optional<byte[]> embeddedProof =
        readSizedField(receiptDecoder, OPTIONAL_PROOF_BYTES);
    assert receiptDecoder.remaining() == 0 : "Receipt payload must not contain trailing bytes";
    assert java.util.Arrays.equals(embeddedPayload, hex(payloadHex))
        : "Receipt payload bytes mismatch";
    assert embeddedSignature.isPresent() : "Receipt signature must be present";
    assert java.util.Arrays.equals(embeddedSignature.orElseThrow(), hex(signatureHex))
        : "Receipt signature bytes mismatch";
    assert embeddedProof.isEmpty() : "Receipt proof must be absent";
  }

  private static void printClaimIdentifierWirePayloadHex() {
    final String payloadHex = "01020304A0";
    final String signatureHex = "AB".repeat(64);
    final IdentifierResolutionReceipt receipt =
        new IdentifierResolutionReceipt(
            "email#retail",
            "opaque:" + "11".repeat(32),
            "22".repeat(32),
            "uaid:" + "33".repeat(31) + "35",
            ACCOUNT_ID,
            42L,
            142L,
            "bfv-affine-sha3-256-v1",
            signatureHex,
            payloadHex,
            new IdentifierResolutionPayload(
                "email#retail",
                "opaque:" + "11".repeat(32),
                "22".repeat(32),
                "uaid:" + "33".repeat(31) + "35",
                ACCOUNT_ID,
                42L,
                142L));

    final InstructionBox instruction = ClaimIdentifierWirePayloadEncoder.encode(ACCOUNT_ID, receipt);
    final InstructionBox.WirePayload wirePayload = (InstructionBox.WirePayload) instruction.payload();
    final NoritoHeader.DecodeResult decoded =
        NoritoHeader.decode(wirePayload.payloadBytes(), null);
    decoded.header().validateChecksum(decoded.payload());

    System.out.println("JAVA_CLAIM_WIRE_NAME=" + instruction.name());
    System.out.println("JAVA_CLAIM_BARE_HEX=" + toHex(decoded.payload()));
    System.out.println("JAVA_CLAIM_FRAMED_HEX=" + toHex(wirePayload.payloadBytes()));
  }

  private static void claimIdentifierMatchesRustCanonicalLiveFixture() {
    final String liveAccountId = canonicalI105AccountId(LIVE_ACCOUNT_MULTIHASH);
    final IdentifierResolutionReceipt receipt =
        new IdentifierResolutionReceipt(
            "email#retail",
            "opaque:d82f9eab952f7a5241bb2339c0095ebc61958428164ab820fad85952f3574585",
            "032df7e7370e04ddbabf0cd40932935a1d2c77a9b8d723bbb9f1472f2791cc71",
            "uaid:c60973f731ccb57008687f9bc38cc712e3be7ab46d99a1beffd1c9fd61e60a87",
            liveAccountId,
            1764450000024L,
            1764453000056L,
            "bfv-programmed-sha3-256-v1",
            LIVE_SIGNATURE_HEX,
            LIVE_PAYLOAD_HEX,
            new IdentifierResolutionPayload(
                "email#retail",
                "opaque:d82f9eab952f7a5241bb2339c0095ebc61958428164ab820fad85952f3574585",
                "032df7e7370e04ddbabf0cd40932935a1d2c77a9b8d723bbb9f1472f2791cc71",
                "uaid:c60973f731ccb57008687f9bc38cc712e3be7ab46d99a1beffd1c9fd61e60a87",
                liveAccountId,
                1764450000024L,
                1764453000056L));

    final InstructionBox instruction =
        ClaimIdentifierWirePayloadEncoder.encode(liveAccountId, receipt);
    final InstructionBox.WirePayload wirePayload = (InstructionBox.WirePayload) instruction.payload();
    final NoritoHeader.DecodeResult decoded =
        NoritoHeader.decode(wirePayload.payloadBytes(), null);
    decoded.header().validateChecksum(decoded.payload());

    assert ClaimIdentifierWirePayloadEncoder.WIRE_NAME.equals(instruction.name())
        : "ClaimIdentifier live wire name mismatch";
    assert LIVE_RUST_BARE_HEX.equals(toHex(decoded.payload()))
        : "ClaimIdentifier live bare payload drifted from Rust";
    assert LIVE_RUST_FRAMED_HEX.equals(toHex(wirePayload.payloadBytes()))
        : "ClaimIdentifier live framed payload drifted from Rust";
  }

  private static byte[] readSizedField(final NoritoDecoder decoder) {
    final long length = decoder.readLength(decoder.compactLenActive());
    return decoder.readBytes(Math.toIntExact(length));
  }

  private static <T> T readSizedField(final NoritoDecoder decoder, final TypeAdapter<T> adapter) {
    final byte[] payload = readSizedField(decoder);
    final NoritoDecoder child = new NoritoDecoder(payload, decoder.flags(), decoder.flagsHint());
    final T value = adapter.decode(child);
    assert child.remaining() == 0 : "Field payload must be fully consumed";
    return value;
  }

  private static byte[] hex(final String value) {
    final byte[] out = new byte[value.length() / 2];
    for (int i = 0; i < value.length(); i += 2) {
      final int high = Character.digit(value.charAt(i), 16);
      final int low = Character.digit(value.charAt(i + 1), 16);
      out[i / 2] = (byte) ((high << 4) | low);
    }
    return out;
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

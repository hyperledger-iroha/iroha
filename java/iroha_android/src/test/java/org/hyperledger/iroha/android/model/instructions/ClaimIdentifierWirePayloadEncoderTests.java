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

  private static final TypeAdapter<Optional<byte[]>> OPTIONAL_BYTES =
      NoritoAdapters.option(NoritoAdapters.rawByteVecAdapter());

  public static void main(String[] args) {
    claimIdentifierEncodesExpectedWirePayload();
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
    final Optional<byte[]> embeddedSignature = readSizedField(receiptDecoder, OPTIONAL_BYTES);
    final Optional<byte[]> embeddedProof = readSizedField(receiptDecoder, OPTIONAL_BYTES);
    assert receiptDecoder.remaining() == 0 : "Receipt payload must not contain trailing bytes";
    assert java.util.Arrays.equals(embeddedPayload, hex(payloadHex))
        : "Receipt payload bytes mismatch";
    assert embeddedSignature.isPresent() : "Receipt signature must be present";
    assert java.util.Arrays.equals(embeddedSignature.orElseThrow(), hex(signatureHex))
        : "Receipt signature bytes mismatch";
    assert embeddedProof.isEmpty() : "Receipt proof must be absent";
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

  private static String canonicalAccountId() {
    final PublicKeyCodec.PublicKeyPayload payload =
        PublicKeyCodec.decodePublicKeyLiteral(
            "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03");
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

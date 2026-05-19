package org.hyperledger.iroha.android.model.instructions;

import java.io.File;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;
import java.util.Locale;
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

  public static void main(String[] args) throws Exception {
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

  private static void claimIdentifierMatchesRustCanonicalFixture() throws Exception {
    final String rustFramedHex =
        runFixtureGenerator("claim-identifier")[0].toUpperCase(Locale.ROOT);
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
    final String actualFramedHex = toHex(wirePayload.payloadBytes());
    assert rustFramedHex.equals(actualFramedHex)
        : "ClaimIdentifier parity framed payload drifted from Rust\nexpected="
            + rustFramedHex
            + "\nactual="
            + actualFramedHex;
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

  private static String[] runFixtureGenerator(final String subcommand) throws Exception {
    final File repoRoot = locateRepoRoot();
    final File targetDir = new File(repoRoot, "target/kotlin-fixture-gen-test");
    final File binary = new File(targetDir, "debug/kotlin-fixture-gen");
    if (!binary.exists()) {
      final ProcessBuilder build =
          new ProcessBuilder("cargo", "build", "-p", "kotlin-fixture-gen")
              .directory(repoRoot)
              .redirectErrorStream(true);
      build.environment().put("CARGO_TARGET_DIR", targetDir.getAbsolutePath());
      final Process process = build.start();
      final String output = readStream(process.getInputStream());
      final int exit = process.waitFor();
      if (exit != 0) {
        throw new IllegalStateException("cargo build failed (" + exit + "): " + output);
      }
    }

    final Process process =
        new ProcessBuilder(binary.getAbsolutePath(), subcommand)
            .directory(repoRoot)
            .redirectErrorStream(false)
            .start();
    final String stdout = readStream(process.getInputStream()).trim();
    final String stderr = readStream(process.getErrorStream()).trim();
    final int exit = process.waitFor();
    if (exit != 0) {
      throw new IllegalStateException(
          "kotlin-fixture-gen " + subcommand + " failed (" + exit + "): " + stderr);
    }
    if (stdout.isEmpty()) {
      throw new IllegalStateException("kotlin-fixture-gen " + subcommand + " produced no output");
    }
    return stdout.split("\\R");
  }

  private static String readStream(final InputStream stream) throws IOException {
    final byte[] buffer = new byte[8192];
    final StringBuilder out = new StringBuilder();
    int read;
    while ((read = stream.read(buffer)) != -1) {
      out.append(new String(buffer, 0, read, StandardCharsets.UTF_8));
    }
    return out.toString();
  }

  private static File locateRepoRoot() throws IOException {
    File dir = new File("").getAbsoluteFile();
    while (!new File(dir, "Cargo.toml").exists()) {
      dir = dir.getParentFile();
      if (dir == null) {
        throw new IOException("Could not locate Iroha repo root from current directory");
      }
    }
    return dir;
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

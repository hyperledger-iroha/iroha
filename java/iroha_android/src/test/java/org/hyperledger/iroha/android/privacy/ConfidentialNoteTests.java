package org.hyperledger.iroha.android.privacy;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import org.hyperledger.iroha.android.model.instructions.ConfidentialEncryptedPayload;

public final class ConfidentialNoteTests {
  private static final String U128_OVERFLOW = "340282366920938463463374607431768211456";

  private ConfidentialNoteTests() {}

  public static void main(final String[] args) {
    derivesRustConfidentialV2Vectors();
    constructorsAndAccessorsAreDefensive();
    rejectsMalformedAndAmbiguousInputs();
    derivationsAreDomainSeparated();
    encryptsAndDecryptsPlaintextContract();
    System.out.println("[IrohaAndroid] ConfidentialNoteTests passed.");
  }

  private static void derivesRustConfidentialV2Vectors() {
    final byte[] spendKey = repeated(0x11, 32);
    final byte[] rho = repeated(0x22, 32);
    final byte[] ownerTag = ConfidentialOwnerTag.deriveFromSpendKey(spendKey);
    final ConfidentialNoteOpening opening =
        new ConfidentialNoteOpening(
            rho, spendKey, ownerTag, "rose#wonderland", "confidential-sdk-chain", "7");

    assertBytes("owner tag",
        "5bd47275e203cc0f57ca4ac1b280f9cfe4709e2932f0ac2f6e78d5bcc9cc1e3a",
        ownerTag);
    assertBytes("commitment",
        "2d6a7673e8120943d9ec65584117bf16c689094a98eec66a6740b677e92a3f3d",
        ConfidentialNoteCommitment.deriveFromOpening(opening));
    assertBytes("nullifier",
        "35230c0fd55b2f43f23150b36663728e0fcbc62ef97e591e730c13bbc5625f25",
        ConfidentialNoteNullifier.deriveFromOpening(opening));
    assertBytes("asset tag",
        "aa6427acbb05173d9c5ee0698832c7e5d80002937595326ce3915b9d37a30d2f",
        ConfidentialNoteTags.deriveAssetTag("rose#wonderland"));
    assertBytes("chain tag",
        "17870127066ce27fda568817c7a8705c878f18abb56e7653dd30f6157de7a237",
        ConfidentialNoteTags.deriveChainTag("confidential-sdk-chain"));

    final byte[] diversifier =
        ConfidentialOwnerTag.deriveDiversifier("recipient".getBytes(StandardCharsets.UTF_8));
    assertBytes("diversifier",
        "0e200699218253a789fd3cd2c5bc5fe7ec4ad663ca35804554fd60cd89cd2525",
        diversifier);
    assertBytes("diversified owner tag",
        "5c7dd75a2bb565931e3cc4badba834e976e251e63bc9dbb911b884a27250b53a",
        ConfidentialOwnerTag.deriveFromSpendKeyWithDiversifier(spendKey, diversifier));
  }

  private static void constructorsAndAccessorsAreDefensive() {
    final byte[] spendKey = repeated(0x11, 32);
    final byte[] rho = repeated(0x22, 32);
    final byte[] ownerTag = ConfidentialOwnerTag.deriveFromSpendKey(spendKey);
    final ConfidentialNoteOpening opening =
        new ConfidentialNoteOpening(rho, spendKey, ownerTag, "rose#wonderland", "chain", "1");

    rho[0] = 0x55;
    spendKey[0] = 0x66;
    ownerTag[0] = 0x77;
    final byte[] exposedRho = opening.rho();
    final byte[] exposedSpendKey = opening.spendKey();
    final byte[] exposedOwnerTag = opening.ownerTag();
    exposedRho[0] = 0x44;
    exposedSpendKey[0] = 0x33;
    exposedOwnerTag[0] = 0x22;

    assert Arrays.equals(repeated(0x22, 32), opening.rho()) : "rho must be defensive";
    assert Arrays.equals(repeated(0x11, 32), opening.spendKey()) : "spendKey must be defensive";
    assert Arrays.equals(ConfidentialOwnerTag.deriveFromSpendKey(repeated(0x11, 32)), opening.ownerTag())
        : "ownerTag must be defensive";
  }

  private static void rejectsMalformedAndAmbiguousInputs() {
    final byte[] spendKey = repeated(0x11, 32);
    final byte[] rho = repeated(0x22, 32);
    final byte[] ownerTag = ConfidentialOwnerTag.deriveFromSpendKey(spendKey);
    final ConfidentialNoteOpening opening =
        new ConfidentialNoteOpening(rho, spendKey, ownerTag, "rose#wonderland", "chain", "1");

    expectThrows(() -> new ConfidentialNoteOpening(new byte[31], spendKey, ownerTag, "rose#wonderland", "chain", "1"));
    expectThrows(() -> new ConfidentialNoteOpening(rho, new byte[0], ownerTag, "rose#wonderland", "chain", "1"));
    expectThrows(() -> new ConfidentialNoteOpening(rho, spendKey, repeated(0xff, 32), "rose#wonderland", "chain", "1"));
    expectThrows(() -> new ConfidentialNoteOpening(rho, spendKey, ownerTag, " rose#wonderland", "chain", "1"));
    expectThrows(() -> new ConfidentialNoteOpening(rho, spendKey, ownerTag, "rose#wonderland", "chain", "01"));
    expectThrows(() -> new ConfidentialNoteOpening(rho, spendKey, ownerTag, "rose#wonderland", "chain", U128_OVERFLOW));
    expectThrows(() -> ConfidentialNoteEncryption.publicKeyFromPrivateKey(new byte[32]));
    final byte[] nonZeroLowOrder = new byte[32];
    nonZeroLowOrder[0] = 1;
    expectThrows(
        () -> ConfidentialNoteEncryption.encryptNote(
            opening, nonZeroLowOrder, repeated(0x66, 32), repeated(0x77, 24)));
    expectThrows(
        () -> ConfidentialNoteEncryption.encryptNote(
            opening,
            ConfidentialNoteEncryption.publicKeyFromPrivateKey(repeated(0x55, 32)),
            new byte[32],
            repeated(0x77, 24)));
  }

  private static void derivationsAreDomainSeparated() {
    final ConfidentialNoteOpening first =
        ConfidentialNoteOpening.fromSpendKey(
            repeated(0x22, 32), repeated(0x11, 32), "rose#wonderland", "chain-a", "7");
    final ConfidentialNoteOpening second =
        ConfidentialNoteOpening.fromSpendKey(
            repeated(0x23, 32), repeated(0x11, 32), "rose#wonderland", "chain-b", "7");

    assert !Arrays.equals(
            ConfidentialNoteCommitment.deriveFromOpening(first),
            ConfidentialNoteCommitment.deriveFromOpening(second))
        : "commitment must change with rho";
    assert !Arrays.equals(
            ConfidentialNoteNullifier.deriveFromOpening(first),
            ConfidentialNoteNullifier.deriveFromOpening(second))
        : "nullifier must change with chain/rho";
  }

  private static void encryptsAndDecryptsPlaintextContract() {
    final byte[] spendKey = repeated(0x11, 32);
    final ConfidentialNoteOpening opening =
        ConfidentialNoteOpening.fromSpendKey(
            repeated(0x22, 32), spendKey, "rose#wonderland", "confidential-sdk-chain", "7");
    final byte[] recipientPrivateKey = repeated(0x55, 32);
    final byte[] recipientPublicKey =
        ConfidentialNoteEncryption.publicKeyFromPrivateKey(recipientPrivateKey);
    final byte[] ephemeralPrivateKey = repeated(0x66, 32);
    final byte[] nonce = repeated(0x77, 24);

    final ConfidentialEncryptedPayload payload =
        ConfidentialNoteEncryption.encryptNote(
            opening, recipientPublicKey, ephemeralPrivateKey, nonce);
    final ConfidentialNoteOpening decrypted =
        ConfidentialNoteDecryption.decryptNote(
            payload, recipientPrivateKey, spendKey, "confidential-sdk-chain");

    assert payload.version() == ConfidentialEncryptedPayload.VERSION_V1 : "wrong payload version";
    assertBytes("recipient public key",
        "38ab664bd86f77d7e66bdd9ae0792913a94fd8b33a1260027e4b46c1f4884c67",
        recipientPublicKey);
    assert Arrays.equals(
            ConfidentialNoteEncryption.publicKeyFromPrivateKey(ephemeralPrivateKey),
            payload.ephemeralPublicKey())
        : "wrong ephemeral public key";
    assertBytes("ephemeral public key",
        "219e4d800da968d2a5fcb009c784f4746c7138edb9ee4844b739e830b05cf424",
        payload.ephemeralPublicKey());
    assert Arrays.equals(nonce, payload.nonce()) : "wrong nonce";
    assertBytes("ciphertext",
        "86c7d4b51314553a9f72fa2207969a7bec6626e3c75943c5c7794a660ed54e76"
            + "371555e888bde13b513f434beef43f5558f1d8fdcd63ac6f40a42c6c90bf26e07d0"
            + "26dd8a3c632afae83d0aea120fa2886dc97f1dc8a91c6b78de3a57e22da75d217e"
            + "4924da954b2b2a758df8cacb2ea153d70a756b7f1b8921e",
        payload.ciphertext());
    assertOpeningEquals(opening, decrypted);
    assert Arrays.equals(
            ConfidentialNoteCommitment.deriveFromOpening(opening),
            ConfidentialNoteCommitment.deriveFromOpening(decrypted))
        : "commitment changed after decrypt";
    assert Arrays.equals(
            ConfidentialNoteNullifier.deriveFromOpening(opening),
            ConfidentialNoteNullifier.deriveFromOpening(decrypted))
        : "nullifier changed after decrypt";

    final byte[] tamperedCiphertext = payload.ciphertext();
    tamperedCiphertext[tamperedCiphertext.length - 1] =
        (byte) (tamperedCiphertext[tamperedCiphertext.length - 1] ^ 0x01);
    final ConfidentialEncryptedPayload tamperedPayload =
        new ConfidentialEncryptedPayload(
            payload.ephemeralPublicKey(), payload.nonce(), tamperedCiphertext);
    expectSecurityException(
        () -> ConfidentialNoteDecryption.decryptNote(tamperedPayload, recipientPrivateKey, spendKey));
    expectSecurityException(
        () -> ConfidentialNoteDecryption.decryptNote(payload, repeated(0x56, 32), spendKey));
    expectThrows(
        () -> ConfidentialNoteDecryption.decryptNote(
            payload, recipientPrivateKey, spendKey, "other-chain"));
    expectThrows(
        () -> ConfidentialNoteDecryption.decryptNote(payload, new byte[32], spendKey));
  }

  private static void assertOpeningEquals(
      final ConfidentialNoteOpening expected, final ConfidentialNoteOpening actual) {
    assert Arrays.equals(expected.rho(), actual.rho()) : "rho changed";
    assert Arrays.equals(expected.spendKey(), actual.spendKey()) : "spendKey changed";
    assert Arrays.equals(expected.ownerTag(), actual.ownerTag()) : "ownerTag changed";
    assert expected.asset().equals(actual.asset()) : "asset changed";
    assert expected.chainId().equals(actual.chainId()) : "chainId changed";
    assert expected.amount().equals(actual.amount()) : "amount changed";
  }

  private static void assertBytes(final String label, final String expectedHex, final byte[] actual) {
    assert Arrays.equals(hex(expectedHex), actual) : label + " mismatch: " + hexLower(actual);
  }

  private static byte[] repeated(final int value, final int len) {
    final byte[] out = new byte[len];
    Arrays.fill(out, (byte) value);
    return out;
  }

  private static byte[] hex(final String value) {
    if ((value.length() & 1) != 0) {
      throw new IllegalArgumentException("hex length must be even");
    }
    final byte[] out = new byte[value.length() / 2];
    for (int i = 0; i < out.length; i++) {
      out[i] = (byte) Integer.parseInt(value.substring(i * 2, i * 2 + 2), 16);
    }
    return out;
  }

  private static String hexLower(final byte[] bytes) {
    final char[] hex = "0123456789abcdef".toCharArray();
    final StringBuilder out = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      final int value = b & 0xff;
      out.append(hex[value >>> 4]);
      out.append(hex[value & 0x0f]);
    }
    return out.toString();
  }

  private static void expectThrows(final Runnable runnable) {
    try {
      runnable.run();
      throw new AssertionError("expected IllegalArgumentException");
    } catch (final IllegalArgumentException expected) {
      // Expected path.
    }
  }

  private static void expectSecurityException(final Runnable runnable) {
    try {
      runnable.run();
      throw new AssertionError("expected SecurityException");
    } catch (final SecurityException expected) {
      // Expected path.
    }
  }
}

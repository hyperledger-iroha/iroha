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
    decryptionFailsClosedUntilPlaintextContractExists();
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

    expectThrows(() -> new ConfidentialNoteOpening(new byte[31], spendKey, ownerTag, "rose#wonderland", "chain", "1"));
    expectThrows(() -> new ConfidentialNoteOpening(rho, new byte[0], ownerTag, "rose#wonderland", "chain", "1"));
    expectThrows(() -> new ConfidentialNoteOpening(rho, spendKey, repeated(0xff, 32), "rose#wonderland", "chain", "1"));
    expectThrows(() -> new ConfidentialNoteOpening(rho, spendKey, ownerTag, " rose#wonderland", "chain", "1"));
    expectThrows(() -> new ConfidentialNoteOpening(rho, spendKey, ownerTag, "rose#wonderland", "chain", "01"));
    expectThrows(() -> new ConfidentialNoteOpening(rho, spendKey, ownerTag, "rose#wonderland", "chain", U128_OVERFLOW));
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

  private static void decryptionFailsClosedUntilPlaintextContractExists() {
    final ConfidentialEncryptedPayload payload =
        new ConfidentialEncryptedPayload(repeated(0x11, 32), repeated(0x22, 24), new byte[] {0x33});
    try {
      ConfidentialNoteDecryption.decryptNote(payload, repeated(0x44, 32));
      throw new AssertionError("expected fail-closed decryption");
    } catch (final UnsupportedOperationException expected) {
      assert expected.getMessage().contains("plaintext layout") : "wrong message";
    }
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
}

package org.hyperledger.iroha.android.privacy;

import java.nio.charset.StandardCharsets;
import java.util.Arrays;
import org.hyperledger.iroha.android.model.NetworkId;
import org.hyperledger.iroha.android.model.instructions.ConfidentialEncryptedPayload;
import org.hyperledger.iroha.android.testing.TestNetworkIds;

public final class ConfidentialNoteTests {
  private static final String U128_OVERFLOW = "340282366920938463463374607431768211456";
  private static final NetworkId NETWORK_ID = TestNetworkIds.canonical();
  private static final NetworkId OTHER_NETWORK_ID = TestNetworkIds.fromSeed(0x42L);

  private ConfidentialNoteTests() {}

  public static void main(final String[] args) {
    derivesCanonicalConfidentialV3Values();
    constructorsAndAccessorsAreDefensive();
    rejectsMalformedAndAmbiguousInputs();
    derivationsAreDomainSeparated();
    encryptsAndDecryptsPlaintextContract();
    System.out.println("[IrohaAndroid] ConfidentialNoteTests passed.");
  }

  private static void derivesCanonicalConfidentialV3Values() {
    assert PrivacyNativeBridge.isNativeAvailable() : "native confidential V3 bridge unavailable";
    assert PrivacyNativeBridge.CONFIDENTIAL_DERIVATION_CONTRACT_REVISION_V3 == 1
        : "unexpected confidential derivation contract revision";
    final byte[] spendKey = repeated(0x11, 32);
    final byte[] rho = repeated(0x22, 32);
    final byte[] ownerTag = ConfidentialOwnerTag.deriveFromSpendKey(spendKey);
    final ConfidentialNoteOpening opening =
        new ConfidentialNoteOpening(
            rho, spendKey, ownerTag, "rose#wonderland", NETWORK_ID, "7");

    assertNonZeroDigest("owner tag", ownerTag);
    assertNonZeroDigest("commitment", ConfidentialNoteCommitment.deriveFromOpening(opening));
    assertNonZeroDigest("nullifier", ConfidentialNoteNullifier.deriveFromOpening(opening));
    assertNonZeroDigest("asset tag", ConfidentialNoteTags.deriveAssetTag("rose#wonderland"));
    assertNonZeroDigest("network tag", ConfidentialNoteTags.deriveNetworkTag(NETWORK_ID));
    assert !Arrays.equals(
            ConfidentialNoteTags.deriveNetworkTag(NETWORK_ID),
            ConfidentialNoteTags.deriveNetworkTag(OTHER_NETWORK_ID))
        : "network tag must bind the exact NetworkId";

    final byte[] diversifier =
        ConfidentialOwnerTag.deriveDiversifier("recipient".getBytes(StandardCharsets.UTF_8));
    assertNonZeroDigest("diversifier", diversifier);
    assertNonZeroDigest(
        "diversified owner tag",
        ConfidentialOwnerTag.deriveFromSpendKeyWithDiversifier(spendKey, diversifier));
    assert Arrays.equals(
            ConfidentialOwnerTag.deriveFromSpendKeyWithDiversifier(spendKey, diversifier),
            ConfidentialNoteOpening.fromSpendKeyWithDiversifier(
                    rho,
                    spendKey,
                    diversifier,
                    "rose#wonderland",
                    NETWORK_ID,
                    "7")
                .ownerTag())
        : "diversified opening ownerTag mismatch";
  }

  private static void constructorsAndAccessorsAreDefensive() {
    final byte[] spendKey = repeated(0x11, 32);
    final byte[] rho = repeated(0x22, 32);
    final byte[] ownerTag = ConfidentialOwnerTag.deriveFromSpendKey(spendKey);
    final ConfidentialNoteOpening opening =
        new ConfidentialNoteOpening(rho, spendKey, ownerTag, "rose#wonderland", NETWORK_ID, "1");

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
        new ConfidentialNoteOpening(rho, spendKey, ownerTag, "rose#wonderland", NETWORK_ID, "1");

    expectThrows(() -> new ConfidentialNoteOpening(new byte[31], spendKey, ownerTag, "rose#wonderland", NETWORK_ID, "1"));
    expectThrows(() -> new ConfidentialNoteOpening(new byte[32], spendKey, ownerTag, "rose#wonderland", NETWORK_ID, "1"));
    expectThrows(() -> new ConfidentialNoteOpening(rho, new byte[0], ownerTag, "rose#wonderland", NETWORK_ID, "1"));
    expectThrows(() -> new ConfidentialNoteOpening(rho, new byte[32], ownerTag, "rose#wonderland", NETWORK_ID, "1"));
    expectThrows(() -> new ConfidentialNoteOpening(rho, spendKey, new byte[32], "rose#wonderland", NETWORK_ID, "1"));
    expectThrows(() -> new ConfidentialNoteOpening(rho, spendKey, repeated(0xff, 32), "rose#wonderland", NETWORK_ID, "1"));
    expectThrows(() -> new ConfidentialNoteOpening(rho, spendKey, ownerTag, " rose#wonderland", NETWORK_ID, "1"));
    expectThrows(() -> new ConfidentialNoteOpening(rho, spendKey, ownerTag, "rose#wonderland", NETWORK_ID, "01"));
    expectThrows(() -> new ConfidentialNoteOpening(rho, spendKey, ownerTag, "rose#wonderland", NETWORK_ID, "0"));
    expectThrows(() -> new ConfidentialNoteOpening(rho, spendKey, ownerTag, "rose#wonderland", NETWORK_ID, U128_OVERFLOW));
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
            repeated(0x22, 32), repeated(0x11, 32), "rose#wonderland", NETWORK_ID, "7");
    final ConfidentialNoteOpening second =
        ConfidentialNoteOpening.fromSpendKey(
            repeated(0x23, 32), repeated(0x11, 32), "rose#wonderland", OTHER_NETWORK_ID, "7");

    assert !Arrays.equals(
            ConfidentialNoteCommitment.deriveFromOpening(first),
            ConfidentialNoteCommitment.deriveFromOpening(second))
        : "commitment must change with rho";
    assert !Arrays.equals(
            ConfidentialNoteNullifier.deriveFromOpening(first),
            ConfidentialNoteNullifier.deriveFromOpening(second))
        : "nullifier must change with network/rho";
  }

  private static void encryptsAndDecryptsPlaintextContract() {
    final byte[] spendKey = repeated(0x11, 32);
    final ConfidentialNoteOpening opening =
        ConfidentialNoteOpening.fromSpendKey(
            repeated(0x22, 32), spendKey, "rose#wonderland", NETWORK_ID, "7");
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
            payload, recipientPrivateKey, spendKey, NETWORK_ID);

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
    assert payload.ciphertext().length > 0 : "ciphertext must not be empty";
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
        () -> ConfidentialNoteDecryption.decryptNote(
            tamperedPayload, recipientPrivateKey, spendKey, NETWORK_ID));
    expectSecurityException(
        () -> ConfidentialNoteDecryption.decryptNote(
            payload, repeated(0x56, 32), spendKey, NETWORK_ID));
    expectThrows(
        () -> ConfidentialNoteDecryption.decryptNote(
            payload, recipientPrivateKey, spendKey, OTHER_NETWORK_ID));
    expectThrows(
        () -> ConfidentialNoteDecryption.decryptNote(payload, new byte[32], spendKey, NETWORK_ID));
    expectThrows(
        () -> ConfidentialNoteDecryption.decryptNote(
            payload, recipientPrivateKey, repeated(0x12, 32), NETWORK_ID));
    expectThrows(
        () -> ConfidentialNoteDecryption.decryptNoteWithOwnerTag(
            payload,
            recipientPrivateKey,
            spendKey,
            ConfidentialOwnerTag.deriveFromSpendKey(repeated(0x12, 32)),
            NETWORK_ID));

    final byte[] diversifier =
        ConfidentialOwnerTag.deriveDiversifier("invoice-1".getBytes(StandardCharsets.UTF_8));
    final ConfidentialNoteOpening diversifiedOpening =
        ConfidentialNoteOpening.fromSpendKeyWithDiversifier(
            repeated(0x24, 32),
            spendKey,
            diversifier,
            "rose#wonderland",
            NETWORK_ID,
            "11");
    final ConfidentialEncryptedPayload diversifiedPayload =
        ConfidentialNoteEncryption.encryptNote(
            diversifiedOpening, recipientPublicKey, repeated(0x68, 32), repeated(0x79, 24));
    expectThrows(
        () -> ConfidentialNoteDecryption.decryptNote(
            diversifiedPayload, recipientPrivateKey, spendKey, NETWORK_ID));
    assertOpeningEquals(
        diversifiedOpening,
        ConfidentialNoteDecryption.decryptNoteWithOwnerTag(
            diversifiedPayload,
            recipientPrivateKey,
            spendKey,
            diversifiedOpening.ownerTag(),
            NETWORK_ID));
  }

  private static void assertOpeningEquals(
      final ConfidentialNoteOpening expected, final ConfidentialNoteOpening actual) {
    assert Arrays.equals(expected.rho(), actual.rho()) : "rho changed";
    assert Arrays.equals(expected.spendKey(), actual.spendKey()) : "spendKey changed";
    assert Arrays.equals(expected.ownerTag(), actual.ownerTag()) : "ownerTag changed";
    assert expected.asset().equals(actual.asset()) : "asset changed";
    assert expected.networkId().equals(actual.networkId()) : "networkId changed";
    assert expected.amount().equals(actual.amount()) : "amount changed";
  }

  private static void assertBytes(final String label, final String expectedHex, final byte[] actual) {
    assert Arrays.equals(hex(expectedHex), actual) : label + " mismatch: " + hexLower(actual);
  }

  private static void assertNonZeroDigest(final String label, final byte[] actual) {
    assert actual.length == 32 : label + " must be 32 bytes";
    assert !Arrays.equals(actual, new byte[32]) : label + " must be non-zero";
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

// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertThrows;
import static org.junit.Assert.assertTrue;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Arrays;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import org.hyperledger.iroha.sdk.crypto.IrohaHash;
import org.hyperledger.iroha.sdk.offline.KagemushaDurabilityAnchorStatementV1;
import org.hyperledger.iroha.sdk.offline.KagemushaDeviceLaneIdV1;
import org.hyperledger.iroha.sdk.offline.KagemushaDeviceHardwareEpochV1;
import org.hyperledger.iroha.sdk.offline.KagemushaDevicePolicyBindingV1;
import org.hyperledger.iroha.sdk.offline.KagemushaEnrolledOpenAccountChallengeV1;
import org.hyperledger.iroha.sdk.offline.KagemushaEnrolledOpenAuthoritySourceV1;
import org.hyperledger.iroha.sdk.offline.KagemushaEnrolledOpenSelectorV1;
import org.hyperledger.iroha.sdk.offline.KagemushaRetailEnrollmentOwnerV1;
import org.junit.Test;
import org.bouncycastle.crypto.params.Ed25519PublicKeyParameters;
import org.bouncycastle.crypto.signers.Ed25519Signer;

/** Structural projection tests; these values are not issuer or hardware evidence. */
public final class KagemushaEnrolledOpenChallengeV1Tests {
  @Test public void actualRustChallengeAndAuthorityArchivesMatchExactly() throws Exception {
    for (final String kind : Arrays.asList("initial", "recovery")) {
      final byte[] archive = fixture(kind + "_challenge_canonical_hex");
      final KagemushaEnrolledOpenAccountChallengeV1 value = KagemushaEnrolledOpenChallengeCodecV1.decodeAccountChallengeShapeExact(archive);
      assertArrayEquals(archive, KagemushaEnrolledOpenChallengeCodecV1.encodeAccountChallengeShape(value));
      final byte[] source = fixture(kind + "_authority_source_canonical_hex");
      assertArrayEquals(source, KagemushaEnrolledOpenChallengeCodecV1.encodeAuthoritySourceShape(value.authoritySource));
      assertArrayEquals(source, KagemushaEnrolledOpenChallengeCodecV1.encodeAuthoritySourceShape(
          KagemushaEnrolledOpenChallengeCodecV1.decodeAuthoritySourceShapeExact(source)));
    }
  }

  @Test public void actualRustSigningHashesAndSignaturesVerifyWithoutDoubleHashing() throws Exception {
    final byte[] publicKey = fixture("account_public_key_hex", "kagemusha_enrolled_open_selector_v1.json");
    for (final String kind : Arrays.asList("initial", "recovery")) {
      final KagemushaEnrolledOpenAccountChallengeV1 value = KagemushaEnrolledOpenChallengeCodecV1.decodeAccountChallengeShapeExact(
          fixture(kind + "_challenge_canonical_hex"));
      final byte[] hash = message(value, value.nonce(), value.releaseId(), value.hardwarePolicyDigest(), value.coreAuthorizationKeyReference());
      final byte[] signature = fixture(kind + "_account_signature_hex");
      assertArrayEquals(fixture(kind + "_account_signing_message_hex"), hash);
      assertTrue(verify(publicKey, hash, signature));
      assertFalse(verify(publicKey, IrohaHash.prehash(hash), signature));
      assertFalse(verify(publicKey, KagemushaEnrolledOpenChallengeCodecV1.encodeAccountChallengeShape(value), signature));
    }
  }

  @Test public void actualRustRecoveryRevisionsPreserveAllBitsBeyondUnsigned64() throws Exception {
    final KagemushaEnrolledOpenAccountChallengeV1 value = KagemushaEnrolledOpenChallengeCodecV1.decodeAccountChallengeShapeExact(
        fixture("recovery_challenge_canonical_hex"));
    final KagemushaDurabilityAnchorStatementV1 statement =
        ((KagemushaEnrolledOpenAuthoritySourceV1.RecoveryCheckpoint) value.authoritySource).statement;
    assertEquals(BigInteger.ONE.shiftLeft(80).add(BigInteger.valueOf(7)), statement.metadataRevision);
    assertEquals(BigInteger.ONE.shiftLeft(72).add(BigInteger.ONE), statement.hardwareEpoch.generation);
    assertEquals(BigInteger.ONE.shiftLeft(90).add(BigInteger.valueOf(19)), statement.logicalSequence);
    assertEquals(BigInteger.ONE.shiftLeft(91).add(BigInteger.valueOf(20)), statement.journalRevision);
    assertEquals(BigInteger.ONE.shiftLeft(92).add(BigInteger.valueOf(21)), statement.inboxRevision);
  }

  private static boolean verify(final byte[] key, final byte[] message, final byte[] signature) {
    final Ed25519Signer verifier = new Ed25519Signer();
    verifier.init(false, new Ed25519PublicKeyParameters(key, 0));
    verifier.update(message, 0, message.length);
    return verifier.verifySignature(signature);
  }

  @Test public void bothSourceVariantsAndCompleteRecoveryMetadataRoundtrip() throws Exception {
    for (final boolean recovered : new boolean[] { false, true }) {
      final KagemushaEnrolledOpenAccountChallengeV1 value = challenge(recovered);
      final byte[] encoded = KagemushaEnrolledOpenChallengeCodecV1.encodeAccountChallengeShape(value);
      final KagemushaEnrolledOpenAccountChallengeV1 decoded =
          KagemushaEnrolledOpenChallengeCodecV1.decodeAccountChallengeShapeExact(encoded);
      assertArrayEquals(encoded, KagemushaEnrolledOpenChallengeCodecV1.encodeAccountChallengeShape(decoded));
      final byte[] source = KagemushaEnrolledOpenChallengeCodecV1.encodeAuthoritySourceShape(value.authoritySource);
      assertArrayEquals(source, KagemushaEnrolledOpenChallengeCodecV1.encodeAuthoritySourceShape(
          KagemushaEnrolledOpenChallengeCodecV1.decodeAuthoritySourceShapeExact(source)));
      if (recovered) {
        final KagemushaDurabilityAnchorStatementV1 statement =
            ((KagemushaEnrolledOpenAuthoritySourceV1.RecoveryCheckpoint) decoded.authoritySource).statement;
        assertEquals(MAX, statement.metadataRevision);
        assertEquals(MAX, statement.hardwareEpoch.generation);
        assertEquals(MAX, statement.logicalSequence);
        assertEquals(MAX.subtract(BigInteger.ONE), statement.journalRevision);
        assertEquals(BigInteger.ZERO, statement.inboxRevision);
        assertArrayEquals(bytes(6), statement.stateCommitment());
        assertArrayEquals(bytes(10), statement.stateNonceCommitment());
        assertArrayEquals(bytes(11), statement.snapshotCommitment());
      }
    }
  }

  @Test public void signingRequiresExactOwnerAndNativePinsAndExcludesArchiveHeader() throws Exception {
    final KagemushaEnrolledOpenAccountChallengeV1 value = challenge(true);
    final byte[] hash = message(value, value.nonce(), value.releaseId(), value.hardwarePolicyDigest(), value.coreAuthorizationKeyReference());
    final byte[] archive = KagemushaEnrolledOpenChallengeCodecV1.encodeAccountChallengeShape(value);
    assertArrayEquals(IrohaHash.prehash(Arrays.copyOfRange(archive, 48, archive.length)), hash);
    assertFalse(Arrays.equals(IrohaHash.prehash(archive), hash));
    assertFalse(Arrays.equals(IrohaHash.prehash(hash), hash));
    final byte[][] pins = {value.nonce(), value.releaseId(), value.hardwarePolicyDigest(), value.coreAuthorizationKeyReference()};
    for (int index = 0; index < pins.length; index++) {
      final byte[][] changed = {pins[0].clone(), pins[1].clone(), pins[2].clone(), pins[3].clone()};
      changed[index][0] ^= 1;
      assertThrows(IllegalArgumentException.class, () -> message(value, changed[0], changed[1], changed[2], changed[3]));
    }
  }

  @Test public void everyTruncationAndOversizedArchivesAreRejected() throws Exception {
    final byte[] original = KagemushaEnrolledOpenChallengeCodecV1.encodeAccountChallengeShape(challenge(true));
    for (int length = 0; length < original.length; length++) {
      final byte[] truncated = Arrays.copyOf(original, length);
      assertThrows(IllegalArgumentException.class,
          () -> KagemushaEnrolledOpenChallengeCodecV1.decodeAccountChallengeShapeExact(truncated));
    }
    assertThrows(IllegalArgumentException.class, () -> KagemushaEnrolledOpenChallengeCodecV1.decodeAccountChallengeShapeExact(
        new byte[KagemushaEnrolledOpenChallengeCodecV1.MAXIMUM_ARCHIVE_BYTES + 1]));
    assertThrows(IllegalArgumentException.class, () -> KagemushaEnrolledOpenChallengeCodecV1.decodeAccountChallengeShapeExact(
        Arrays.copyOf(original, original.length + 1)));
  }

  @Test public void challengeAndCheckpointArraysRemainDefensive() throws Exception {
    final KagemushaEnrolledOpenAccountChallengeV1 value = challenge(true);
    final byte[] before = KagemushaEnrolledOpenChallengeCodecV1.encodeAccountChallengeShape(value);
    Arrays.fill(value.nonce(), (byte) 0);
    Arrays.fill(value.enrollmentId(), (byte) 0);
    Arrays.fill(value.releaseId(), (byte) 0);
    final KagemushaEnrolledOpenAuthoritySourceV1.RecoveryCheckpoint source =
        (KagemushaEnrolledOpenAuthoritySourceV1.RecoveryCheckpoint) value.authoritySource;
    Arrays.fill(source.terminalCertificateDigest(), (byte) 0);
    Arrays.fill(source.statement.stateCommitment(), (byte) 0);
    Arrays.fill(source.statement.stateNonceCommitment(), (byte) 0);
    Arrays.fill(source.statement.snapshotCommitment(), (byte) 0);
    assertArrayEquals(before, KagemushaEnrolledOpenChallengeCodecV1.encodeAccountChallengeShape(value));
  }

  private static final BigInteger MAX = BigInteger.ONE.shiftLeft(128).subtract(BigInteger.ONE);

  private static KagemushaEnrolledOpenAccountChallengeV1 challenge(final boolean recovered) throws Exception {
    final KagemushaEnrolledOpenSelectorV1 selector = selector();
    final KagemushaRetailEnrollmentOwnerV1 owner = selector.owner;
    final KagemushaEnrolledOpenAuthoritySourceV1 source;
    if (recovered) {
      source = new KagemushaEnrolledOpenAuthoritySourceV1.RecoveryCheckpoint(
          new KagemushaDurabilityAnchorStatementV1(MAX, 1,
              new KagemushaDeviceLaneIdV1(owner.runtime.networkId.bytes(), owner.laneId(), owner.runtime.asset.canonicalPayload(), owner.runtime.scale),
              bytes(6), new KagemushaDeviceHardwareEpochV1(MAX, bytes(7)),
              new KagemushaDevicePolicyBindingV1(bytes(8), bytes(9)), bytes(10), MAX, MAX.subtract(BigInteger.ONE), BigInteger.ZERO, bytes(11)), bytes(12));
    } else source = new KagemushaEnrolledOpenAuthoritySourceV1.InitialCertificate(bytes(4));
    return new KagemushaEnrolledOpenAccountChallengeV1(1, KagemushaEnrolledOpenAccountChallengeV1.ACCOUNT_DOMAIN,
        selector.enrollmentId(), owner, bytes(1), source, bytes(2), bytes(3), bytes(4), 120000L);
  }

  private static byte[] message(final KagemushaEnrolledOpenAccountChallengeV1 value, final byte[] nonce,
      final byte[] release, final byte[] policy, final byte[] core) throws Exception {
    return KagemushaEnrolledOpenChallengeCodecV1.accountSigningMessageShape(value, selector(), nonce, release, policy, core);
  }

  private static KagemushaEnrolledOpenSelectorV1 selector() throws Exception {
    return KagemushaNoritoV1.decodeEnrolledOpenSelectorShapeExact(fixture("selector_canonical_hex", "kagemusha_enrolled_open_selector_v1.json"));
  }

  private static byte[] fixture(final String field) throws Exception {
    return fixture(field, "kagemusha_enrolled_open_challenge_v1.json");
  }

  private static byte[] fixture(final String field, final String name) throws Exception {
    Path fixture = null;
    for (final String prefix : Arrays.asList("../../../", "../../", "../", "")) {
      final Path candidate = Paths.get(prefix + "fixtures/offline/" + name);
      if (Files.isRegularFile(candidate)) { fixture = candidate; break; }
    }
    if (fixture == null) throw new AssertionError("shared Rust selector fixture missing");
    final String json = new String(Files.readAllBytes(fixture), StandardCharsets.UTF_8);
    final Matcher match = Pattern.compile("\"" + field + "\"\\s*:\\s*\"([^\"]+)\"").matcher(json);
    if (!match.find()) throw new AssertionError("selector fixture missing");
    final String hex = match.group(1);
    final byte[] bytes = new byte[hex.length() / 2];
    for (int index = 0; index < bytes.length; index++) bytes[index] = (byte) Integer.parseInt(hex.substring(index * 2, index * 2 + 2), 16);
    return bytes;
  }

  private static byte[] bytes(final int value) {
    final byte[] bytes = new byte[32];
    Arrays.fill(bytes, (byte) value);
    return bytes;
  }
}

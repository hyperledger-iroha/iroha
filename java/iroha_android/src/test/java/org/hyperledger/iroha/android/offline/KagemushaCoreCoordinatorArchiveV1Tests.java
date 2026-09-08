// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertFalse;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.Arrays;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import org.hyperledger.iroha.sdk.offline.KagemushaDeviceSenderPreparationSelectorV1;
import org.hyperledger.iroha.sdk.offline.KagemushaDeviceSenderPublicInputsV1;
import org.hyperledger.iroha.sdk.offline.KagemushaNativeSenderCandidateV1;
import org.hyperledger.iroha.sdk.offline.KagemushaNativeSenderPreparationV1;
import org.junit.Test;

/** Java mirror consumes the same Rust-generated canonical archives as Kotlin and Swift. */
public final class KagemushaCoreCoordinatorArchiveV1Tests {
  @Test public void terminalEnvelopeUsesTheExactNativeLimit() {
    assertEquals(32, KagemushaCoreCoordinatorArchiveV1.terminalEnvelopeDigestShape(new byte[7936]).length);
    assertThrows(IllegalArgumentException.class,
        () -> KagemushaCoreCoordinatorArchiveV1.terminalEnvelopeDigestShape(new byte[7937]));
    assertThrows(IllegalArgumentException.class,
        () -> KagemushaCoreCoordinatorArchiveV1.terminalEnvelopeDigestShape(new byte[0]));
  }
  @Test public void rustArchivesReencodeExactly() throws Exception {
    final byte[] preparation = archive("preparation");
    final byte[] candidate = archive("candidate");
    final byte[] recovery = archive("recovery");
    final byte[] receipt = archive("redemption_terminal_receipt");
    assertArrayEquals(preparation, KagemushaCoreCoordinatorArchiveV1.encodePreparationShape(
        KagemushaCoreCoordinatorArchiveV1.decodePreparationShapeExact(preparation)));
    assertArrayEquals(candidate, KagemushaCoreCoordinatorArchiveV1.encodeCandidateShape(
        KagemushaCoreCoordinatorArchiveV1.decodeCandidateShapeExact(candidate)));
    assertArrayEquals(recovery, KagemushaCoreCoordinatorArchiveV1.encodeRecoveryShape(
        KagemushaCoreCoordinatorArchiveV1.decodeRecoveryShapeExact(recovery)));
    assertArrayEquals(receipt, KagemushaCoreCoordinatorArchiveV1.encodeRedemptionReceiptShape(
        KagemushaCoreCoordinatorArchiveV1.decodeRedemptionReceiptShapeExact(receipt)));
  }

  @Test public void inputDigestMatchesRustSignedCandidateAndRejectsChangedOperation() throws Exception {
    final KagemushaNativeSenderPreparationV1 preparation =
        KagemushaCoreCoordinatorArchiveV1.decodeCandidateShapeExact(archive("candidate")).preparation;
    final KagemushaDeviceSenderPublicInputsV1 inputs = new KagemushaDeviceSenderPublicInputsV1.SendSplit(
        section(load("kagemusha_v1.json"), "payment_request"));
    assertArrayEquals(preparation.inputsDigest(), KagemushaCoreCoordinatorArchiveV1.inputsDigestShape(
        preparation.operationId(), preparation.context, inputs));
    final byte[] changed = preparation.operationId();
    changed[0] ^= 1;
    assertFalse(Arrays.equals(preparation.inputsDigest(), KagemushaCoreCoordinatorArchiveV1.inputsDigestShape(
        changed, preparation.context, inputs)));
  }

  @Test public void trailingBytesAndSubstitutedCandidateInputsAreRejected() throws Exception {
    final byte[] original = archive("candidate");
    final byte[] trailing = Arrays.copyOf(original, original.length + 1);
    assertThrows(IllegalArgumentException.class, () -> KagemushaCoreCoordinatorArchiveV1.decodeCandidateShapeExact(trailing));
    final KagemushaNativeSenderCandidateV1 candidate = KagemushaCoreCoordinatorArchiveV1.decodeCandidateShapeExact(original);
    final byte[] changed = candidate.preparation.inputsDigest();
    changed[0] ^= 1;
    final KagemushaNativeSenderCandidateV1 swapped = new KagemushaNativeSenderCandidateV1(candidate.preparation,
        new KagemushaDeviceSenderPreparationSelectorV1(changed, candidate.selector.preparationId()),
        candidate.candidateDigest(), candidate.hardwareCommitAuthorization());
    assertThrows(IllegalArgumentException.class, () -> KagemushaCoreCoordinatorArchiveV1.encodeCandidateShape(swapped));
  }

  private static byte[] archive(final String section) throws Exception {
    return section(load("kagemusha_core_coordinator_archives_v1.json"), section);
  }

  private static byte[] section(final String json, final String section) {
    final Matcher match = Pattern.compile("\"" + section + "\"\\s*:\\s*\\{.*?\"norito_hex\"\\s*:\\s*\"([^\"]+)\"",
        Pattern.DOTALL).matcher(json);
    if (!match.find()) throw new AssertionError("missing fixture section " + section);
    final String hex = match.group(1);
    final byte[] bytes = new byte[hex.length() / 2];
    for (int index = 0; index < bytes.length; index++) {
      bytes[index] = (byte) Integer.parseInt(hex.substring(index * 2, index * 2 + 2), 16);
    }
    return bytes;
  }

  private static String load(final String name) throws Exception {
    Path directory = Paths.get("").toAbsolutePath().normalize();
    while (directory != null) {
      final Path path = directory.resolve("fixtures/offline/" + name);
      if (Files.isRegularFile(path)) return new String(Files.readAllBytes(path), StandardCharsets.UTF_8);
      directory = directory.getParent();
    }
    throw new AssertionError("missing shared fixture " + name);
  }
}

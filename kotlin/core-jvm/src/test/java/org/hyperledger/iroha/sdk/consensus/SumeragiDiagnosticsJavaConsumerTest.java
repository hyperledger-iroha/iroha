// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.consensus;

import static org.junit.jupiter.api.Assertions.*;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import kotlinx.serialization.json.Json;
import kotlinx.serialization.json.JsonObject;
import org.junit.jupiter.api.Test;

/** Java callers use the canonical immutable Kotlin diagnostics and Rust-owned fixture. */
final class SumeragiDiagnosticsJavaConsumerTest {
  @Test
  void seedConstructionOwnsInputAndExposesImmutableValues() {
    final List<Integer> seed = new ArrayList<>(Collections.nCopies(32, 1));
    final SumeragiNposDiagnostics value =
        new SumeragiNposDiagnostics(BigInteger.TEN, seed, BigInteger.ONE, BigInteger.ZERO);
    final int hash = value.hashCode();
    seed.clear();
    assertEquals(Collections.nCopies(32, 1), value.getEpochSeed());
    assertThrows(UnsupportedOperationException.class, () -> value.getEpochSeed().set(0, 0));
    assertEquals(hash, value.hashCode());
    assertEquals(new SumeragiNposDiagnostics(BigInteger.TEN, Collections.nCopies(32, 1),
        BigInteger.ONE, BigInteger.ZERO), value);
    assertNotEquals(new SumeragiNposDiagnostics(BigInteger.TEN, Collections.nCopies(32, 2),
        BigInteger.ONE, BigInteger.ZERO), value);
    assertThrows(IllegalArgumentException.class,
        () -> new SumeragiNposDiagnostics(BigInteger.TEN, Collections.nCopies(32, 0),
            BigInteger.ONE, BigInteger.ZERO));
  }

  @Test
  void canonicalDiagnosticsRemainImmutableThroughJavaCollections() throws Exception {
    final JsonObject fixture = (JsonObject) Json.Default.parseToJsonElement(
        new String(Files.readAllBytes(fixturePath()), StandardCharsets.UTF_8));
    final JsonObject golden = (JsonObject) fixture.get("golden");
    final String wire = golden.get("expected_diagnostics").toString();
    final SumeragiDiagnosticsStatus value = SumeragiDiagnosticsStatus.parseJson(wire);
    assertEquals(value, SumeragiDiagnosticsStatus.parseJson(wire.getBytes(StandardCharsets.UTF_8)));
    assertEquals(value.hashCode(), SumeragiDiagnosticsStatus.parseJson(wire).hashCode());
    assertFalse(value.getNativeAmxParticipantApplications().isEmpty());
    assertThrows(UnsupportedOperationException.class,
        () -> value.getNativeAmxParticipantApplications().clear());
    assertThrows(UnsupportedOperationException.class, () -> value.getAutonomousLaneExecutions().clear());
    assertThrows(UnsupportedOperationException.class, () -> value.getLaneCommitments().clear());
    assertThrows(UnsupportedOperationException.class, () -> value.getDataspaceCommitments().clear());
    assertThrows(UnsupportedOperationException.class, () -> value.getLaneSettlementCommitments().clear());
    assertThrows(UnsupportedOperationException.class, () -> value.getLaneRelayEnvelopes().clear());
    assertThrows(UnsupportedOperationException.class, () -> value.getLanePayloadOwnerships().clear());
    assertThrows(UnsupportedOperationException.class, () -> value.getCommittedLaneBlocks().clear());
    assertThrows(UnsupportedOperationException.class, () -> value.getLaneBlockSessions().clear());
    assertThrows(UnsupportedOperationException.class, () -> value.getLaneGovernance().clear());
    assertThrows(UnsupportedOperationException.class, () -> value.getLaneGovernanceSealedAliases().clear());
  }

  @Test
  void directPipelineConstructionEnforcesUnsignedCountersAndValueEquality() {
    assertThrows(IllegalArgumentException.class, () -> pipeline(BigInteger.valueOf(-1)));
    assertThrows(IllegalArgumentException.class, () -> pipeline(BigInteger.ONE.shiftLeft(64)));
    assertEquals(pipeline(BigInteger.ZERO), pipeline(BigInteger.ZERO));
    assertEquals(pipeline(BigInteger.ZERO).hashCode(), pipeline(BigInteger.ZERO).hashCode());
    assertNotEquals(pipeline(BigInteger.ZERO), pipeline(BigInteger.ONE));
  }

  private static SumeragiPipelineExecutionStatus pipeline(BigInteger count) {
    final BigInteger zero = BigInteger.ZERO;
    return new SumeragiPipelineExecutionStatus(count, zero, zero, zero, zero, zero,
        zero, zero, zero, zero, zero, zero, zero, zero, zero, zero, zero);
  }

  private static Path fixturePath() {
    for (Path root = Paths.get("").toAbsolutePath(); root != null; root = root.getParent()) {
      final Path candidate = root.resolve("fixtures/sumeragi_v2/native_amx_v2_grouped.json");
      if (Files.isRegularFile(candidate)) return candidate;
    }
    throw new AssertionError("Rust-owned Native AMX grouped fixture was not found");
  }
}

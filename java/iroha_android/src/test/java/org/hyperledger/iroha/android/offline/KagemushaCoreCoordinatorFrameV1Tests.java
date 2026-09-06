// Copyright 2026 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.android.offline;

import static org.junit.Assert.assertArrayEquals;
import static org.junit.Assert.assertEquals;
import static org.junit.Assert.assertThrows;

import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.util.HashSet;
import java.util.List;
import java.util.Set;
import org.hyperledger.iroha.sdk.offline.KagemushaCoreCoordinatorMethodV1;
import org.junit.Test;

/** The Java mirror validates every coordinator frame against the same native/Kotlin/Swift corpus. */
public final class KagemushaCoreCoordinatorFrameV1Tests {
  @Test
  public void everyMethodMatchesSharedFramesAndRejectsRetiredSchemas() throws Exception {
    final Set<Integer> methods = new HashSet<>();
    int count = 0;
    for (final String line : fixture()) {
      if (line.startsWith("#") || line.isEmpty()) continue;
      final String[] columns = line.split("\t", -1);
      assertEquals(4, columns.length);
      final int code = Integer.parseInt(columns[1]);
      final KagemushaCoreCoordinatorMethodV1 method = KagemushaCoreCoordinatorMethodV1.values()[code - 1];
      methods.add(method.code);
      final byte[] request = hex(columns[2]);
      final byte[] response = hex(columns[3]);
      assertArrayEquals(request, KagemushaCoreCoordinatorFrameV1.encodeRequest(
          method, KagemushaCoreCoordinatorFrameV1.decodeRequest(method, request)));
      assertArrayEquals(response, KagemushaCoreCoordinatorFrameV1.encodeResponse(
          method, request, KagemushaCoreCoordinatorFrameV1.decodeResponse(method, request, response)));
      final byte[] retired = request.clone();
      retired[8] = 1;
      assertThrows(IllegalArgumentException.class, () -> KagemushaCoreCoordinatorFrameV1.decodeRequest(method, retired));
      count++;
    }
    assertEquals(14, count);
    assertEquals(10, methods.size());
  }

  @Test
  public void reservationResponseCannotSubstituteCallerIdentity() throws Exception {
    final String[] columns = fixture().stream().filter(line -> line.startsWith("reserve\t")).findFirst().get().split("\t");
    final byte[] request = hex(columns[2]);
    final byte[] response = hex(columns[3]);
    response[20] ^= 1;
    assertThrows(IllegalArgumentException.class, () -> KagemushaCoreCoordinatorFrameV1.decodeResponse(
        KagemushaCoreCoordinatorMethodV1.RESERVE_OPERATION_ID, request, response));
  }

  private static List<String> fixture() throws Exception {
    Path directory = Paths.get("").toAbsolutePath().normalize();
    while (directory != null) {
      final Path fixture = directory.resolve("fixtures/offline/kagemusha_core_coordinator_frame_v1.tsv");
      if (Files.isRegularFile(fixture)) return Files.readAllLines(fixture, StandardCharsets.UTF_8);
      directory = directory.getParent();
    }
    throw new AssertionError("missing coordinator fixture");
  }

  private static byte[] hex(final String value) {
    final byte[] result = new byte[value.length() / 2];
    for (int index = 0; index < result.length; index++) {
      result[index] = (byte) Integer.parseInt(value.substring(index * 2, index * 2 + 2), 16);
    }
    return result;
  }
}

package org.hyperledger.iroha.sdk.consensus;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.util.HashSet;
import java.util.List;
import java.util.Map;
import org.junit.jupiter.api.Test;

/** Java source consumers exercise the canonical Kotlin native evidence decoder. */
public final class NativeExecutionEvidenceFixtureTests {
  @Test
  public void completeRustNativeCapturesRemainConsumableFromJava() {
    for (int lanes : new int[] {1, 4}) {
      byte[] capture = NativeExecutionEvidenceFixtures.load(lanes);
      List<Map<String, Object>> rows = NativeExecutionEvidenceFixtures.inspect(capture);
      assertEquals(8, rows.size());
      HashSet<Object> identities = new HashSet<>();
      HashSet<String> routes = new HashSet<>();
      for (Map<String, Object> row : rows) {
        identities.add(row.get("logical_id"));
        routes.add(row.get("lane_id") + ":" + row.get("dataspace_id"));
      }
      assertEquals(8, identities.size());
      assertEquals(lanes, routes.size());
      byte[] invalid = capture.clone();
      invalid[0] = '[';
      assertThrows(IllegalArgumentException.class,
          () -> NativeExecutionEvidenceFixtures.inspect(invalid));
      assertEquals(8, NativeExecutionEvidenceFixtures.inspect(capture).size());
    }
  }
}

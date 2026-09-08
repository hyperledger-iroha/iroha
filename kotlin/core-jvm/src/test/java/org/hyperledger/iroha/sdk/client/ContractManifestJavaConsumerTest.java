package org.hyperledger.iroha.sdk.client;

import static org.junit.jupiter.api.Assertions.assertEquals;
import static org.junit.jupiter.api.Assertions.assertThrows;

import java.io.File;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import org.junit.jupiter.api.Test;

/** Java consumers use the Kotlin-owned exact nominal manifest parser and immutable models. */
final class ContractManifestJavaConsumerTest {
  @Test
  void sharedNominalErrorsPreserveJapaneseIdentityAndUnit() throws Exception {
    File directory = new File(".").getAbsoluteFile();
    File fixture = null;
    while (directory != null) {
      File candidate = new File(directory, "fixtures/kotodama/nominal_errors_v1.json");
      if (candidate.isFile()) { fixture = candidate; break; }
      directory = directory.getParentFile();
    }
    if (fixture == null) throw new AssertionError("missing shared nominal fixture");
    String payload = new String(Files.readAllBytes(fixture.toPath()), StandardCharsets.UTF_8);
    ContractManifest manifest = ContractJsonParser.parseManifestRecord(payload.getBytes(StandardCharsets.UTF_8)).manifest;
    EntrypointValueTypeV1 schema = manifest.entrypoints.get(0).returnSchema;
    assertEquals("Result<(), example/vault@1.0.0::金庫::拒否>", schema.canonicalTypeName);
    assertEquals(EntrypointValueTypeNodeKindV1.UNIT, schema.nodes.get(1).kind);
    assertEquals("不足", schema.nodes.get(2).errorType.variants.get(0).name);
    assertEquals(2, manifest.errorTypes.size());
    EntrypointValueTypeV1 cursor = manifest.entrypoints.get(1).returnSchema;
    assertEquals("Option<StateCursor<int>>", cursor.canonicalTypeName);
    assertEquals(EntrypointValueTypeNodeKindV1.STATE_CURSOR, cursor.nodes.get(1).kind);
    assertEquals(EntrypointValueKindV1.INT, cursor.nodes.get(1).leafKind);
    assertEquals(1, cursor.wordCount);
    assertEquals("StatePage<int, bool, 8>", manifest.entrypoints.get(2).returnSchema.canonicalTypeName);
    assertEquals(2, manifest.entrypoints.get(2).returnSchema.wordCount);
    assertThrows(UnsupportedOperationException.class, () -> manifest.errorTypes.clear());
    assertThrows(IllegalStateException.class, () -> ContractJsonParser.parseManifestRecord(
        payload.replaceFirst("CapacityExceeded", "DifferentMeaning").getBytes(StandardCharsets.UTF_8)));
    String stateOnlyUnknown = payload.replace(
        "\"type_name\": \"Result<(), example/vault@1.0.0::金庫::拒否>\"",
        "\"type_name\": \"Result<(), missing/vault@1.0.0::金庫::拒否>\"");
    assertThrows(IllegalStateException.class, () -> ContractJsonParser.parseManifestRecord(
        stateOnlyUnknown.getBytes(StandardCharsets.UTF_8)));
    for (String forged : new String[] {"StatePage{anything: int}", "StatePage{items: List<(int, bool), 8>, next: Option<StateCursor<bool>>}"}) {
      assertThrows(IllegalStateException.class, () -> ContractJsonParser.parseManifestRecord(payload.replace(
          "StatePage{items: List<(int, bool), 8>, next: Option<StateCursor<int>>}", forged).getBytes(StandardCharsets.UTF_8)));
    }
  }
}

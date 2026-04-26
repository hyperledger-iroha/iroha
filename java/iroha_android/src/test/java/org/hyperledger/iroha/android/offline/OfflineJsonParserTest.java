package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;

public final class OfflineJsonParserTest {

  private OfflineJsonParserTest() {}

  public static void main(final String[] args) {
    parsesOfflineV2Readiness();
    canonicalizesJson();
    System.out.println("[IrohaAndroid] OfflineJsonParserTest passed.");
  }

  private static void parsesOfflineV2Readiness() {
    final String json =
        """
        {
          "offline_note_v2": true,
          "offline_one_use_keys": true,
          "offline_recursive_note_proof": false,
          "offline_fountain_qr_v1": true,
          "offline_sync_optional": true,
          "offline_telemetry": false
        }
        """;
    final OfflineV2Readiness readiness =
        OfflineJsonParser.parseOfflineV2Readiness(json.getBytes(StandardCharsets.UTF_8));
    assert readiness.offlineNoteV2();
    assert readiness.offlineOneUseKeys();
    assert !readiness.offlineRecursiveNoteProof();
    assert readiness.offlineFountainQrV1();
    assert readiness.offlineSyncOptional();
    assert !readiness.offlineTelemetry();
  }

  private static void canonicalizesJson() {
    final String encoded =
        OfflineJsonParser.canonicalJson("{\"b\":2,\"a\":1}".getBytes(StandardCharsets.UTF_8));
    assert "{\"a\":1,\"b\":2}".equals(encoded) : "canonical JSON mismatch: " + encoded;
  }
}

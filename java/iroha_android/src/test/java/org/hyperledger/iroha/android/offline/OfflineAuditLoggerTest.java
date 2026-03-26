package org.hyperledger.iroha.android.offline;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.List;

public final class OfflineAuditLoggerTest {

  private OfflineAuditLoggerTest() {}

  public static void main(final String[] args) throws Exception {
    recordsAndExportsEntries();
    rejectsFractionalTimestamp();
    System.out.println("[IrohaAndroid] OfflineAuditLoggerTest passed.");
  }

  private static void recordsAndExportsEntries() throws IOException {
    final Path logFile = Files.createTempFile("offline_audit", ".json");
    final OfflineAuditLogger logger = new OfflineAuditLogger(logFile, true);
    logger.record(
        new OfflineAuditEntry(
            "tx1", "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn", "6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9", "7EAD8EFYUx1aVKZPUU1fyKvr8dF1", "10", 1));
    logger.record(
        new OfflineAuditEntry(
            "tx2", "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn", "6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9", "7EAD8EFYUx1aVKZPUU1fyKvr8dF1", "20", 2));

    final List<OfflineAuditEntry> entries = logger.entries();
    assert entries.size() == 2 : "entries size mismatch";
    assert Files.size(logFile) > 0 : "log file should not be empty";

    final String exported = new String(logger.exportJson(), StandardCharsets.UTF_8);
    assert exported.contains("tx1") : "export missing tx1";
    assert exported.contains("tx2") : "export missing tx2";

    logger.clear();
    assert logger.entries().isEmpty() : "clear should remove entries";

    Files.deleteIfExists(logFile);
  }

  private static void rejectsFractionalTimestamp() throws IOException {
    final Path logFile = Files.createTempFile("offline_audit_invalid", ".json");
    final String json =
        """
        [
          {
            "tx_id": "tx1",
            "sender_id": "6cmzPVPX944pj7vVyADRpma2DCcBUsG1mhz8VrXArhXaGsjvRUcnbVn",
            "receiver_id": "6cmzPVPX9mKibcHVns59R11W7wkcZTg7r71RLbydDr2HGf5MdMCQRm9",
            "asset_id": "7EAD8EFYUx1aVKZPUU1fyKvr8dF1",
            "amount": "10",
            "timestamp_ms": 1.5
          }
        ]
        """;
    Files.writeString(logFile, json, StandardCharsets.UTF_8);
    boolean thrown = false;
    try {
      new OfflineAuditLogger(logFile, true);
    } catch (Exception ex) {
      thrown = true;
    }
    assert thrown : "expected fractional timestamps to be rejected";
    Files.deleteIfExists(logFile);
  }
}

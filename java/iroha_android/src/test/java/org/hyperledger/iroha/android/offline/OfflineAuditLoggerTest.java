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
    System.out.println("[IrohaAndroid] OfflineAuditLoggerTest passed.");
  }

  private static void recordsAndExportsEntries() throws IOException {
    final Path logFile = Files.createTempFile("offline_audit", ".json");
    final OfflineAuditLogger logger = new OfflineAuditLogger(logFile, true);
    logger.record(
        new OfflineAuditEntry(
            "tx1", "alice@wonderland", "bob@wonderland", "usd#wonderland", "10", 1));
    logger.record(
        new OfflineAuditEntry(
            "tx2", "carol@wonderland", "dave@wonderland", "usd#wonderland", "20", 2));

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
}

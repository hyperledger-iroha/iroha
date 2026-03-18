package org.hyperledger.iroha.android.client;

import java.nio.file.Files;
import java.nio.file.Path;
import org.hyperledger.iroha.android.client.HttpClientTransport;
import org.hyperledger.iroha.android.client.queue.DirectoryPendingTransactionQueue;
import org.hyperledger.iroha.android.client.queue.PendingTransactionQueue;
import org.hyperledger.iroha.android.client.queue.OfflineJournalPendingTransactionQueue;

public final class ClientConfigOfflineQueueTests {

  private ClientConfigOfflineQueueTests() {}

  public static void main(final String[] args) throws Exception {
    builderEnablesJournalQueue();
    builderEnablesDirectoryQueue();
    helperReturnsConfigWithDirectoryQueue();
    System.out.println("[IrohaAndroid] ClientConfigOfflineQueueTests passed.");
  }

  private static void builderEnablesJournalQueue() throws Exception {
    final Path journalPath = Files.createTempFile("offline_journal_queue_builder", ".bin");
    try {
      final ClientConfig config =
          ClientConfig.builder()
              .setBaseUri(java.net.URI.create("https://example.com"))
              .enableOfflineJournalQueue(journalPath, "builder-passphrase".toCharArray())
              .build();
      final PendingTransactionQueue queue = config.pendingQueue();
      assert queue instanceof OfflineJournalPendingTransactionQueue
          : "pending queue is not journal-backed";
      assert Files.exists(journalPath) : "journal file was not created";
    } finally {
      Files.deleteIfExists(journalPath);
    }
  }

  private static void builderEnablesDirectoryQueue() throws Exception {
    final Path directory = Files.createTempDirectory("dir_queue_builder");
    final ClientConfig config =
        ClientConfig.builder()
            .setBaseUri(java.net.URI.create("https://example.com"))
            .enableDirectoryPendingQueue(directory)
            .build();
    final PendingTransactionQueue queue = config.pendingQueue();
    assert queue instanceof DirectoryPendingTransactionQueue
        : "pending queue is not directory-backed";
    assert Files.exists(directory) : "queue directory was not created";
  }

  private static void helperReturnsConfigWithDirectoryQueue() throws Exception {
    final ClientConfig config =
        ClientConfig.builder().setBaseUri(java.net.URI.create("https://example.com")).build();
    final Path queueDir = Files.createTempDirectory("http_client_transport_dir_helper");
    try {
      final ClientConfig updated =
          HttpClientTransport.withDirectoryPendingQueue(config, queueDir);
      final PendingTransactionQueue queue = updated.pendingQueue();
      assert queue != null : "pending queue should be configured";
      assert queue
          .getClass()
          .getSimpleName()
          .equals("DirectoryPendingTransactionQueue")
          : "expected directory queue implementation";
    } finally {
      Files.walk(queueDir)
          .sorted(java.util.Comparator.reverseOrder())
          .forEach(path -> {
            try {
              Files.deleteIfExists(path);
            } catch (final Exception ignored) {
            }
          });
    }
  }
}

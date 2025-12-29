package org.hyperledger.iroha.android.client;

import java.net.URI;
import java.nio.file.Files;
import java.nio.file.Path;
import org.hyperledger.iroha.android.client.queue.PendingTransactionQueue;

public final class HttpClientTransportOfflineQueueTests {

  private HttpClientTransportOfflineQueueTests() {}

  public static void main(final String[] args) throws Exception {
    helperReturnsConfigWithJournalQueue();
    System.out.println("[IrohaAndroid] HttpClientTransportOfflineQueueTests passed.");
  }

  private static void helperReturnsConfigWithJournalQueue() throws Exception {
    final ClientConfig config =
        ClientConfig.builder().setBaseUri(URI.create("https://example.com")).build();
    final Path journalPath =
        Files.createTempFile("http_client_transport_journal_helper", ".bin");
    try {
      final ClientConfig updated =
          HttpClientTransport.withOfflineJournalQueue(
              config, journalPath, "transport-passphrase".toCharArray());
      final PendingTransactionQueue queue = updated.pendingQueue();
      assert queue != null : "pending queue should be configured";
      assert queue
          .getClass()
          .getSimpleName()
          .equals("OfflineJournalPendingTransactionQueue")
          : "expected journal queue implementation";
    } finally {
      Files.deleteIfExists(journalPath);
    }
  }
}

package org.hyperledger.iroha.android.client.queue;

import java.nio.file.Files;
import java.nio.file.Path;
import java.security.SecureRandom;
import java.util.List;
import org.hyperledger.iroha.android.tx.SignedTransaction;

public final class FilePendingTransactionQueueTests {

  private static final SecureRandom RNG = new SecureRandom();

  private FilePendingTransactionQueueTests() {}

  public static void main(final String[] args) throws Exception {
    shouldPersistAndDrainInOrder();
    shouldComputeSizeWithoutDrain();
    shouldPersistExportedKeyBundle();
    System.out.println("[IrohaAndroid] Pending transaction queue tests passed.");
  }

  private static void shouldPersistAndDrainInOrder() throws Exception {
    final Path tempDir = Files.createTempDirectory("iroha-queue-test-");
    final Path queueFile = tempDir.resolve("pending.queue");
    final FilePendingTransactionQueue queue = new FilePendingTransactionQueue(queueFile);

    final SignedTransaction first = randomTransaction();
    final SignedTransaction second = randomTransaction();
    queue.enqueue(first);
    queue.enqueue(second);

    final List<SignedTransaction> drained = queue.drain();
    assert drained.size() == 2 : "Expected two drained transactions";
    assert payloadEquals(first, drained.get(0)) : "First transaction must be preserved";
    assert payloadEquals(second, drained.get(1)) : "Second transaction must be preserved";
    assert queue.size() == 0 : "Queue must be empty after drain";
  }

  private static void shouldComputeSizeWithoutDrain() throws Exception {
    final Path tempDir = Files.createTempDirectory("iroha-queue-size-");
    final Path queueFile = tempDir.resolve("pending.queue");
    final FilePendingTransactionQueue queue = new FilePendingTransactionQueue(queueFile);
    queue.enqueue(randomTransaction());
    queue.enqueue(randomTransaction());
    assert queue.size() == 2 : "Size should reflect queued entries";
    queue.drain();
    assert queue.size() == 0 : "Size should be zero after draining";
  }

  private static void shouldPersistExportedKeyBundle() throws Exception {
    final Path tempDir = Files.createTempDirectory("iroha-queue-export-");
    final Path queueFile = tempDir.resolve("pending.queue");
    final FilePendingTransactionQueue queue = new FilePendingTransactionQueue(queueFile);

    final byte[] bundle = randomBytes(48);
    final SignedTransaction original =
        new SignedTransaction(
            randomBytes(32),
            randomBytes(64),
            randomBytes(32),
            "schema.v1",
            "alias-export",
            bundle);
    queue.enqueue(original);

    final List<SignedTransaction> drained = queue.drain();
    assert drained.size() == 1 : "Queue should return the enqueued transaction";
    final SignedTransaction restored = drained.get(0);
    assert restored.keyAlias().orElseThrow(() -> new AssertionError("Alias missing"))
        .equals("alias-export") : "Alias must round-trip";
    assert restored.exportedKeyBundle().isPresent() : "Export bundle must be preserved";
    assert java.util.Arrays.equals(bundle, restored.exportedKeyBundle().get())
        : "Export bundle bytes must match";
  }

  private static SignedTransaction randomTransaction() {
    final byte[] payload = randomBytes(32);
    final byte[] signature = randomBytes(64);
    final byte[] publicKey = randomBytes(32);
    return new SignedTransaction(payload, signature, publicKey, "schema.v1", nextAlias());
  }

  private static byte[] randomBytes(final int length) {
    final byte[] bytes = new byte[length];
    RNG.nextBytes(bytes);
    return bytes;
  }

  private static boolean payloadEquals(final SignedTransaction expected, final SignedTransaction actual) {
    return java.util.Arrays.equals(expected.encodedPayload(), actual.encodedPayload())
        && java.util.Arrays.equals(expected.signature(), actual.signature())
        && java.util.Arrays.equals(expected.publicKey(), actual.publicKey())
        && expected.schemaName().equals(actual.schemaName())
        && expected.keyAlias().equals(actual.keyAlias())
        && expected.exportedKeyBundle().equals(actual.exportedKeyBundle());
  }

  private static int aliasCounter = 0;

  private static String nextAlias() {
    return "alias-" + aliasCounter++;
  }
}

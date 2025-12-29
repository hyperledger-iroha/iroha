package org.hyperledger.iroha.android.client.queue;

import java.nio.file.Files;
import java.nio.file.Path;
import java.security.SecureRandom;
import java.util.List;
import org.hyperledger.iroha.android.tx.SignedTransaction;

/** Tests for {@link DirectoryPendingTransactionQueue}. */
public final class DirectoryPendingTransactionQueueTests {

  private static final SecureRandom RNG = new SecureRandom();

  private DirectoryPendingTransactionQueueTests() {}

  public static void main(final String[] args) throws Exception {
    shouldPersistAndDrainInOrder();
    shouldSurviveProcessRestarts();
    shouldReportDepthWithoutDrain();
    shouldPersistExportBundles();
    System.out.println("[IrohaAndroid] Directory pending queue tests passed.");
  }

  private static void shouldPersistAndDrainInOrder() throws Exception {
    final Path root = Files.createTempDirectory("dir-queue-basic-");
    final DirectoryPendingTransactionQueue queue =
        new DirectoryPendingTransactionQueue(root);

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

  private static void shouldSurviveProcessRestarts() throws Exception {
    final Path root = Files.createTempDirectory("dir-queue-restart-");
    final DirectoryPendingTransactionQueue firstInstance =
        new DirectoryPendingTransactionQueue(root);
    final SignedTransaction initial = randomTransaction();
    final SignedTransaction second = randomTransaction();
    firstInstance.enqueue(initial);
    firstInstance.enqueue(second);

    // Simulate process restart: create a fresh queue pointing at the same directory.
    final DirectoryPendingTransactionQueue secondInstance =
        new DirectoryPendingTransactionQueue(root);
    final SignedTransaction third = randomTransaction();
    secondInstance.enqueue(third);

    final List<SignedTransaction> drained = secondInstance.drain();
    assert drained.size() == 3 : "All queued transactions should drain after restart";
    assert payloadEquals(initial, drained.get(0)) : "Initial transaction should drain first";
    assert payloadEquals(second, drained.get(1)) : "Second transaction should retain ordering";
    assert payloadEquals(third, drained.get(2)) : "Newly enqueued transaction should drain last";
  }

  private static void shouldReportDepthWithoutDrain() throws Exception {
    final Path root = Files.createTempDirectory("dir-queue-depth-");
    final DirectoryPendingTransactionQueue queue =
        new DirectoryPendingTransactionQueue(root);
    queue.enqueue(randomTransaction());
    queue.enqueue(randomTransaction());
    assert queue.size() == 2 : "Depth should reflect un-drained entries";
  }

  private static void shouldPersistExportBundles() throws Exception {
    final Path root = Files.createTempDirectory("dir-queue-export-");
    final DirectoryPendingTransactionQueue queue =
        new DirectoryPendingTransactionQueue(root);
    final byte[] bundle = randomBytes(48);
    final SignedTransaction original =
        new SignedTransaction(
            randomBytes(32),
            randomBytes(64),
            randomBytes(32),
            "schema.v2",
            "alias-dir",
            bundle);
    queue.enqueue(original);
    final List<SignedTransaction> drained = queue.drain();
    assert drained.size() == 1 : "Queue should return the enqueued transaction";
    final SignedTransaction restored = drained.get(0);
    assert restored.exportedKeyBundle().isPresent() : "Export bundle must be preserved";
    assert java.util.Arrays.equals(bundle, restored.exportedKeyBundle().get())
        : "Export bundle bytes must match";
    assert restored.keyAlias().orElse("").equals("alias-dir") : "Alias should round-trip";
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
    return "dir-alias-" + aliasCounter++;
  }
}

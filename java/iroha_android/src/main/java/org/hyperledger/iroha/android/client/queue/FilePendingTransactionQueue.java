package org.hyperledger.iroha.android.client.queue;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.ArrayList;
import java.util.Base64;
import java.util.Collections;
import java.util.List;
import java.util.Objects;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.tx.SignedTransaction;

/**
 * File-backed queue that persists transactions as Base64-encoded records separated by newlines.
 *
 * <p>Each line contains a Base64-encoded canonical pending-transaction record.
 *
 * <p>The queue preserves insertion order and deletes the underlying file when drained.
 */
public final class FilePendingTransactionQueue implements PendingTransactionQueue {

  private static final Base64.Encoder ENCODER = Base64.getEncoder();
  private static final Base64.Decoder DECODER = Base64.getDecoder();
  private final Path queueFile;
  private final Object lock = new Object();

  public FilePendingTransactionQueue(final Path queueFile) throws IOException {
    this.queueFile = Objects.requireNonNull(queueFile, "queueFile");
    final Path parent = queueFile.getParent();
    if (parent != null) {
      Files.createDirectories(parent);
    }
    if (!Files.exists(queueFile)) {
      try {
        Files.createFile(queueFile);
      } catch (final java.nio.file.FileAlreadyExistsException ignored) {
        // Another thread/process created the file between the exists check and create call.
      }
    }
  }

  @Override
  public void enqueue(final SignedTransaction transaction) throws IOException {
    Objects.requireNonNull(transaction, "transaction");
    final String line;
    try {
      line = ENCODER.encodeToString(PendingTransactionRecordCodec.encode(transaction));
    } catch (final NoritoException ex) {
      throw new IOException("Failed to encode pending transaction record", ex);
    }
    synchronized (lock) {
      Files.write(
          queueFile,
          (line + System.lineSeparator()).getBytes(StandardCharsets.UTF_8),
          StandardOpenOption.CREATE,
          StandardOpenOption.APPEND);
    }
  }

  @Override
  public List<SignedTransaction> drain() throws IOException {
    synchronized (lock) {
      if (!Files.exists(queueFile)) {
        return Collections.emptyList();
      }
      final List<String> lines = Files.readAllLines(queueFile, StandardCharsets.UTF_8);
      final List<SignedTransaction> transactions = new ArrayList<>(lines.size());
      for (final String line : lines) {
        if (line.trim().isEmpty()) {
          continue;
        }
        transactions.add(decodeEntry(line));
      }
      Files.write(queueFile, new byte[0], StandardOpenOption.TRUNCATE_EXISTING);
      return transactions;
    }
  }

  @Override
  public int size() throws IOException {
    synchronized (lock) {
      if (!Files.exists(queueFile)) {
        return 0;
      }
      int count = 0;
      for (final String line : Files.readAllLines(queueFile, StandardCharsets.UTF_8)) {
        if (!line.trim().isEmpty()) {
          count++;
        }
      }
      return count;
    }
  }

  /** Removes all queued transactions without returning them. Primarily useful for tests. */
  public void clear() throws IOException {
    synchronized (lock) {
      if (Files.exists(queueFile)) {
        Files.write(queueFile, new byte[0], StandardOpenOption.TRUNCATE_EXISTING);
      }
    }
  }

  @Override
  public String telemetryQueueName() {
    return "file";
  }

  private SignedTransaction decodeEntry(final String line) throws IOException {
    final byte[] envelopeBytes;
    try {
      envelopeBytes = DECODER.decode(line);
    } catch (final IllegalArgumentException ex) {
      throw new IOException("Failed to decode queue entry", ex);
    }
    try {
      return PendingTransactionRecordCodec.decode(envelopeBytes);
    } catch (final NoritoException ex) {
      throw new IOException("Failed to decode queue entry", ex);
    }
  }
}

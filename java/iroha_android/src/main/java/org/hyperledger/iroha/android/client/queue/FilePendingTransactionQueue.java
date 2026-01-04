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
import org.hyperledger.iroha.android.tx.offline.OfflineSigningEnvelope;
import org.hyperledger.iroha.android.tx.offline.OfflineSigningEnvelopeCodec;

/**
 * File-backed queue that persists transactions as Base64-encoded records separated by newlines.
 *
 * <p>Each line contains a Base64-encoded {@link OfflineSigningEnvelope}.
 *
 * <p>The queue preserves insertion order and deletes the underlying file when drained.
 */
public final class FilePendingTransactionQueue implements PendingTransactionQueue {

  private static final Base64.Encoder ENCODER = Base64.getEncoder();
  private static final Base64.Decoder DECODER = Base64.getDecoder();
  private static final OfflineSigningEnvelopeCodec ENVELOPE_CODEC =
      new OfflineSigningEnvelopeCodec();
  private static final String DEFAULT_KEY_ALIAS = "pending.queue";

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
    final OfflineSigningEnvelope envelope =
        OfflineSigningEnvelope.builder()
            .setEncodedPayload(transaction.encodedPayload())
            .setSignature(transaction.signature())
            .setPublicKey(transaction.publicKey())
            .setSchemaName(transaction.schemaName())
            .setKeyAlias(transaction.keyAlias().orElse(DEFAULT_KEY_ALIAS))
            .setIssuedAtMs(System.currentTimeMillis())
            .setExportedKeyBundle(transaction.exportedKeyBundle().orElse(null))
            .build();
    final String line;
    try {
      line = ENCODER.encodeToString(ENVELOPE_CODEC.encode(envelope));
    } catch (final NoritoException ex) {
      throw new IOException("Failed to encode offline signing envelope", ex);
    }
    synchronized (lock) {
      Files.writeString(
          queueFile,
          line + System.lineSeparator(),
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
        if (line.isBlank()) {
          continue;
        }
        transactions.add(decodeEntry(line));
      }
      Files.writeString(queueFile, "", StandardCharsets.UTF_8, StandardOpenOption.TRUNCATE_EXISTING);
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
        if (!line.isBlank()) {
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
        Files.writeString(
            queueFile, "", StandardCharsets.UTF_8, StandardOpenOption.TRUNCATE_EXISTING);
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
      final OfflineSigningEnvelope envelope = ENVELOPE_CODEC.decode(envelopeBytes);
      return new SignedTransaction(
          envelope.encodedPayload(),
          envelope.signature(),
          envelope.publicKey(),
          envelope.schemaName(),
          envelope.keyAlias(),
          envelope.exportedKeyBundle().orElse(null));
    } catch (final NoritoException ex) {
      throw new IOException("Failed to decode queue entry", ex);
    }
  }
}

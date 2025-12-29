package org.hyperledger.iroha.android.client.queue;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Objects;
import java.util.regex.Matcher;
import java.util.regex.Pattern;
import java.util.stream.Collectors;
import java.util.stream.Stream;
import org.hyperledger.iroha.android.norito.NoritoException;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.tx.offline.OfflineSigningEnvelope;
import org.hyperledger.iroha.android.tx.offline.OfflineSigningEnvelopeCodec;

/**
 * Directory-backed pending queue that persists each transaction as a discrete envelope file.
 *
 * <p>This variant is intended for OEM/governed environments that want a stable on-disk layout
 * (one file per queued transaction) rather than a single append-only log. Entries survive process
 * restarts and preserve insertion order via monotonically increasing file names.
 */
public final class DirectoryPendingTransactionQueue implements PendingTransactionQueue {

  private static final OfflineSigningEnvelopeCodec ENVELOPE_CODEC =
      new OfflineSigningEnvelopeCodec();
  private static final Pattern ENTRY_PATTERN = Pattern.compile("^pending-(\\d+)\\.bin$");
  private static final String DEFAULT_KEY_ALIAS = "pending.queue";

  private final Path rootDir;
  private final Object lock = new Object();
  private long nextId;

  public DirectoryPendingTransactionQueue(final Path rootDir) throws IOException {
    this.rootDir = Objects.requireNonNull(rootDir, "rootDir");
    Files.createDirectories(rootDir);
    this.nextId = initialiseNextId(rootDir);
  }

  @Override
  public void enqueue(final SignedTransaction transaction) throws IOException {
    Objects.requireNonNull(transaction, "transaction");
    final OfflineSigningEnvelope envelope = buildEnvelope(transaction);
    final byte[] encoded;
    try {
      encoded = ENVELOPE_CODEC.encode(envelope);
    } catch (final NoritoException ex) {
      throw new IOException("Failed to encode offline signing envelope", ex);
    }
    synchronized (lock) {
      final Path path = rootDir.resolve(formatEntryName(nextId++));
      Files.write(path, encoded, StandardOpenOption.CREATE_NEW, StandardOpenOption.WRITE);
    }
  }

  @Override
  public List<SignedTransaction> drain() throws IOException {
    final List<Path> entries = listEntriesInOrder();
    final List<SignedTransaction> transactions = new ArrayList<>(entries.size());
    for (final Path entry : entries) {
      final byte[] payload = Files.readAllBytes(entry);
      transactions.add(decodeEnvelope(payload));
      Files.deleteIfExists(entry);
    }
    return transactions;
  }

  @Override
  public int size() throws IOException {
    return listEntriesInOrder().size();
  }

  @Override
  public String telemetryQueueName() {
    return "oem_directory";
  }

  private static long initialiseNextId(final Path rootDir) throws IOException {
    long highest = -1L;
    try (Stream<Path> stream = Files.list(rootDir)) {
      for (final Path entry : stream.collect(Collectors.toList())) {
        final Matcher matcher = ENTRY_PATTERN.matcher(entry.getFileName().toString());
        if (matcher.matches()) {
          try {
            final long id = Long.parseLong(matcher.group(1));
            highest = Math.max(highest, id);
          } catch (final NumberFormatException ignored) {
            // Skip malformed files; the queue only operates on the pending-<id>.bin pattern.
          }
        }
      }
    }
    return highest + 1L;
  }

  private List<Path> listEntriesInOrder() throws IOException {
    final List<Path> entries = new ArrayList<>();
    synchronized (lock) {
      try (Stream<Path> stream = Files.list(rootDir)) {
        stream
            .filter(path -> ENTRY_PATTERN.matcher(path.getFileName().toString()).matches())
            .sorted(Comparator.comparingLong(DirectoryPendingTransactionQueue::extractId))
            .forEach(entries::add);
      }
    }
    return entries;
  }

  private static long extractId(final Path path) {
    final Matcher matcher = ENTRY_PATTERN.matcher(path.getFileName().toString());
    if (!matcher.matches()) {
      return Long.MAX_VALUE;
    }
    try {
      return Long.parseLong(matcher.group(1));
    } catch (final NumberFormatException ex) {
      return Long.MAX_VALUE;
    }
  }

  private static OfflineSigningEnvelope buildEnvelope(final SignedTransaction transaction) {
    return OfflineSigningEnvelope.builder()
        .setEncodedPayload(transaction.encodedPayload())
        .setSignature(transaction.signature())
        .setPublicKey(transaction.publicKey())
        .setSchemaName(transaction.schemaName())
        .setKeyAlias(transaction.keyAlias().orElse(DEFAULT_KEY_ALIAS))
        .setIssuedAtMs(System.currentTimeMillis())
        .setExportedKeyBundle(transaction.exportedKeyBundle().orElse(null))
        .build();
  }

  private static SignedTransaction decodeEnvelope(final byte[] payload) throws IOException {
    try {
      final OfflineSigningEnvelope envelope = ENVELOPE_CODEC.decode(payload);
      return new SignedTransaction(
          envelope.encodedPayload(),
          envelope.signature(),
          envelope.publicKey(),
          envelope.schemaName(),
          envelope.keyAlias(),
          envelope.exportedKeyBundle().orElse(null));
    } catch (final NoritoException ex) {
      throw new IOException("Failed to decode offline signing envelope", ex);
    }
  }

  private static String formatEntryName(final long id) {
    return String.format("pending-%020d.bin", id);
  }
}

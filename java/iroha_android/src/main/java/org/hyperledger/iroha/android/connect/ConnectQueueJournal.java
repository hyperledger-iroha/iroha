package org.hyperledger.iroha.android.connect;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.Paths;
import java.nio.file.StandardCopyOption;
import java.nio.file.StandardOpenOption;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Base64;
import java.util.Collections;
import java.util.Iterator;
import java.util.List;
import java.util.Objects;
import java.util.concurrent.locks.ReentrantLock;

/** Append-only Connect queue journal mirrored across SDKs (IOS-CONNECT-003). */
public final class ConnectQueueJournal {

  public static final class Configuration {
    private final Path rootDirectory;
    private final int maxRecordsPerQueue;
    private final int maxBytesPerQueue;
    private final long retentionMillis;

    public Configuration() {
      this(defaultRootDirectory(), 32, 1 << 20, Duration.ofHours(24).toMillis());
    }

    public Configuration(
        final Path rootDirectory,
        final int maxRecordsPerQueue,
        final int maxBytesPerQueue,
        final long retentionMillis) {
      this.rootDirectory = Objects.requireNonNull(rootDirectory, "rootDirectory");
      if (maxRecordsPerQueue <= 0) {
        throw new IllegalArgumentException("maxRecordsPerQueue must be positive");
      }
      if (maxBytesPerQueue <= 0) {
        throw new IllegalArgumentException("maxBytesPerQueue must be positive");
      }
      if (retentionMillis <= 0) {
        throw new IllegalArgumentException("retentionMillis must be positive");
      }
      this.maxRecordsPerQueue = maxRecordsPerQueue;
      this.maxBytesPerQueue = maxBytesPerQueue;
      this.retentionMillis = retentionMillis;
    }

    public Path rootDirectory() {
      return rootDirectory;
    }

    public int maxRecordsPerQueue() {
      return maxRecordsPerQueue;
    }

    public int maxBytesPerQueue() {
      return maxBytesPerQueue;
    }

    public long retentionMillis() {
      return retentionMillis;
    }
  }

  private final byte[] sessionId;
  private final Configuration configuration;
  private final ConnectQueueStorage storage;
  private final ConnectJournalFile appQueue;
  private final ConnectJournalFile walletQueue;

  public ConnectQueueJournal(final byte[] sessionId) throws ConnectJournalException {
    this(sessionId, new Configuration());
  }

  public ConnectQueueJournal(final byte[] sessionId, final Configuration configuration)
      throws ConnectJournalException {
    this.sessionId = Objects.requireNonNull(sessionId, "sessionId").clone();
    this.configuration = Objects.requireNonNull(configuration, "configuration");
    this.storage = new ConnectQueueStorage(this.sessionId, configuration.rootDirectory());
    this.appQueue =
        new ConnectJournalFile(
            ConnectDirection.APP_TO_WALLET, storage.appQueuePath(), configuration);
    this.walletQueue =
        new ConnectJournalFile(
            ConnectDirection.WALLET_TO_APP, storage.walletQueuePath(), configuration);
  }

  public void append(
      final ConnectDirection direction, final long sequence, final byte[] ciphertext)
      throws ConnectJournalException {
    append(direction, sequence, ciphertext, timestampNow(), null);
  }

  public void append(
      final ConnectDirection direction,
      final long sequence,
      final byte[] ciphertext,
      final long receivedAtMs,
      final Long ttlOverrideMs)
      throws ConnectJournalException {
    Objects.requireNonNull(direction, "direction");
    Objects.requireNonNull(ciphertext, "ciphertext");
    final long retention = ttlOverrideMs != null ? ttlOverrideMs : configuration.retentionMillis();
    final long expiresAt = Math.addExact(receivedAtMs, Math.max(retention, 1L));
    final byte[] digest = ConnectJournalRecord.computePayloadHash(ciphertext);
    final ConnectJournalRecord record =
      new ConnectJournalRecord(direction, sequence, digest, ciphertext, receivedAtMs, expiresAt);
    fileFor(direction).append(record, receivedAtMs);
  }

  public List<ConnectJournalRecord> records(final ConnectDirection direction)
      throws ConnectJournalException {
    return records(direction, timestampNow());
  }

  public List<ConnectJournalRecord> records(
      final ConnectDirection direction, final long nowMillis) throws ConnectJournalException {
    return fileFor(direction).records(nowMillis);
  }

  public List<ConnectJournalRecord> popOldest(
      final ConnectDirection direction, final int count) throws ConnectJournalException {
    return popOldest(direction, count, timestampNow());
  }

  public List<ConnectJournalRecord> popOldest(
      final ConnectDirection direction, final int count, final long nowMillis)
      throws ConnectJournalException {
    return fileFor(direction).popOldest(count, nowMillis);
  }

  private static long timestampNow() {
    return System.currentTimeMillis();
  }

  private ConnectJournalFile fileFor(final ConnectDirection direction) {
    return direction == ConnectDirection.APP_TO_WALLET ? appQueue : walletQueue;
  }

  private static Path defaultRootDirectory() {
    final String home = System.getProperty("user.home");
    final Path base = home != null ? Paths.get(home) : Paths.get(".");
    return base.resolve(".iroha").resolve("connect").resolve("queues");
  }

  private static final class ConnectQueueStorage {
    private final Path sessionDirectory;
    private final Path appQueuePath;
    private final Path walletQueuePath;

    ConnectQueueStorage(final byte[] sessionId, final Path rootDirectory)
        throws ConnectJournalException {
      final String directoryName = hashSessionId(sessionId);
      this.sessionDirectory =
          rootDirectory.toAbsolutePath().normalize().resolve(directoryName).normalize();
      this.appQueuePath = sessionDirectory.resolve("app_to_wallet.queue");
      this.walletQueuePath = sessionDirectory.resolve("wallet_to_app.queue");
    }

    Path appQueuePath() {
      return appQueuePath;
    }

    Path walletQueuePath() {
      return walletQueuePath;
    }

    void ensureSessionDirectory() throws IOException {
      Files.createDirectories(sessionDirectory);
    }

    private static String hashSessionId(final byte[] sessionId) throws ConnectJournalException {
      try {
        final MessageDigest digest = MessageDigest.getInstance("SHA-256");
        final byte[] hash = digest.digest(sessionId);
        final StringBuilder builder = new StringBuilder(hash.length * 2);
        for (byte b : hash) {
          builder.append(String.format("%02x", b));
        }
        return builder.toString();
      } catch (final NoSuchAlgorithmException ex) {
        return Base64.getUrlEncoder()
            .withoutPadding()
            .encodeToString(sessionId)
            .replace('=', '_');
      } catch (final Exception ex) {
        throw new ConnectJournalException("failed to hash Connect session id", ex);
      }
    }
  }

  private final class ConnectJournalFile {
    private final ConnectDirection direction;
    private final Path path;
    private final Configuration configuration;
    private final ReentrantLock lock = new ReentrantLock();

    ConnectJournalFile(
        final ConnectDirection direction, final Path path, final Configuration configuration) {
      this.direction = direction;
      this.path = path;
      this.configuration = configuration;
    }

    void append(final ConnectJournalRecord record, final long nowMillis)
        throws ConnectJournalException {
      lock.lock();
      try {
        storage.ensureSessionDirectory();
        final List<ConnectJournalRecord> records = readLocked();
        records.add(record);
        pruneExpired(records, nowMillis);
        pruneLimits(records);
        writeLocked(records);
      } catch (final IOException ex) {
        throw new ConnectJournalException("failed to append journal record", ex);
      } finally {
        lock.unlock();
      }
    }

    List<ConnectJournalRecord> records(final long nowMillis) throws ConnectJournalException {
      lock.lock();
      try {
        final List<ConnectJournalRecord> records = readLocked();
        if (pruneExpired(records, nowMillis)) {
          writeLocked(records);
        }
        return Collections.unmodifiableList(new ArrayList<>(records));
      } finally {
        lock.unlock();
      }
    }

    List<ConnectJournalRecord> popOldest(final int count, final long nowMillis)
        throws ConnectJournalException {
      if (count <= 0) {
        throw new IllegalArgumentException("count must be positive");
      }
      lock.lock();
      try {
        final List<ConnectJournalRecord> records = readLocked();
        pruneExpired(records, nowMillis);
        final List<ConnectJournalRecord> removed = new ArrayList<>();
        final int toRemove = Math.min(count, records.size());
        for (int i = 0; i < toRemove; i++) {
          removed.add(records.remove(0));
        }
        if (toRemove > 0) {
          writeLocked(records);
        }
        return removed;
      } finally {
        lock.unlock();
      }
    }

    private List<ConnectJournalRecord> readLocked() throws ConnectJournalException {
      if (!Files.exists(path)) {
        return new ArrayList<>();
      }
      try {
        final byte[] bytes = Files.readAllBytes(path);
        final List<ConnectJournalRecord> records = new ArrayList<>();
        int offset = 0;
        while (offset < bytes.length) {
          final ConnectJournalRecord.DecodeResult result =
              ConnectJournalRecord.decode(bytes, offset);
          records.add(result.record());
          offset += result.bytesConsumed();
        }
        return records;
      } catch (final IOException ex) {
        throw new ConnectJournalException("failed to read journal file", ex);
      }
    }

    private void writeLocked(final List<ConnectJournalRecord> records)
        throws ConnectJournalException {
      try {
        if (records.isEmpty()) {
          Files.deleteIfExists(path);
          return;
        }
        final ByteArrayOutputStream buffer = new ByteArrayOutputStream();
        for (final ConnectJournalRecord record : records) {
          buffer.writeBytes(record.encode());
        }
        final Path temp =
            path.resolveSibling(path.getFileName().toString() + ".tmp-" + System.nanoTime());
        Files.createDirectories(path.getParent());
        Files.write(
            temp,
            buffer.toByteArray(),
            StandardOpenOption.CREATE,
            StandardOpenOption.WRITE,
            StandardOpenOption.TRUNCATE_EXISTING);
        Files.move(temp, path, StandardCopyOption.REPLACE_EXISTING, StandardCopyOption.ATOMIC_MOVE);
      } catch (final IOException ex) {
        throw new ConnectJournalException("failed to write journal file", ex);
      }
    }

    private boolean pruneExpired(final List<ConnectJournalRecord> records, final long nowMillis) {
      final Iterator<ConnectJournalRecord> iterator = records.iterator();
      boolean changed = false;
      while (iterator.hasNext()) {
        final ConnectJournalRecord record = iterator.next();
        if (record.expiresAtMs() <= nowMillis) {
          iterator.remove();
          changed = true;
        }
      }
      return changed;
    }

    private boolean pruneLimits(final List<ConnectJournalRecord> records) {
      boolean changed = false;
      while (records.size() > configuration.maxRecordsPerQueue()) {
        records.remove(0);
        changed = true;
      }
      while (totalEncodedSize(records) > configuration.maxBytesPerQueue() && !records.isEmpty()) {
        records.remove(0);
        changed = true;
      }
      return changed;
    }

    private int totalEncodedSize(final List<ConnectJournalRecord> records) {
      int total = 0;
      for (final ConnectJournalRecord record : records) {
        total += record.encodedLength();
      }
      return total;
    }
  }
}

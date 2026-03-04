package org.hyperledger.iroha.android.offline;

import java.io.IOException;
import java.nio.ByteBuffer;
import java.nio.ByteOrder;
import java.nio.channels.FileChannel;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.security.GeneralSecurityException;
import java.util.ArrayList;
import java.util.Arrays;
import java.util.Collections;
import java.util.Comparator;
import java.util.HashMap;
import java.util.HashSet;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.Set;
import java.util.concurrent.locks.ReentrantLock;
import javax.crypto.Mac;
import javax.crypto.spec.SecretKeySpec;
import org.hyperledger.iroha.android.crypto.Blake2b;

/**
 * Append-only journal mirroring the Rust/Swift {@code OfflineJournal} implementations.
 *
 * <p>Records are stored as: {@code [kind (1)] [timestamp_le (8)] [payload_len_le (4)] [tx_id (32)]
 * [payload] [chain (32)] [hmac (32)]} where {@code chain = BLAKE2b-256(prev_chain || tx_id)} and
 * {@code hmac = HMAC-SHA256(prev_chain || record_without_hmac)}.
 */
public final class OfflineJournal implements AutoCloseable {

  private enum EntryKind {
    PENDING((byte) 0),
    COMMITTED((byte) 1);

    private final byte tag;

    EntryKind(final byte tag) {
      this.tag = tag;
    }

    static EntryKind fromByte(final byte value) throws OfflineJournalException {
      for (final EntryKind kind : values()) {
        if (kind.tag == value) {
          return kind;
        }
      }
      throw new OfflineJournalException(
          OfflineJournalException.Reason.INTEGRITY_VIOLATION,
          "unknown offline journal entry kind: " + value);
    }
  }

  private static final byte[] MAGIC = new byte[] {0x49, 0x4a, 0x4e, 0x4c}; // "IJNL"
  private static final byte VERSION = 1;
  private static final int HASH_LENGTH = 32;
  private static final int HMAC_LENGTH = 32;
  private static final int HEADER_LENGTH = MAGIC.length + 1;
  private static final int MIN_RECORD_LENGTH = 1 + 8 + 4 + HASH_LENGTH + HASH_LENGTH + HMAC_LENGTH;

  private final Path path;
  private final byte[] key;
  private final FileChannel channel;
  private final ReentrantLock lock = new ReentrantLock();
  private byte[] lastChain;
  private final Map<HashKey, OfflineJournalEntry> pending;
  private final Set<HashKey> committed;

  public OfflineJournal(final Path path, final OfflineJournalKey key)
      throws OfflineJournalException {
    this.path = Objects.requireNonNull(path, "path");
    Objects.requireNonNull(key, "key");
    this.key = key.raw();
    try {
      final Path parent = path.getParent();
      if (parent != null) {
        Files.createDirectories(parent);
      }
      this.channel =
          FileChannel.open(
              path,
              StandardOpenOption.CREATE,
              StandardOpenOption.READ,
              StandardOpenOption.WRITE);
      ensureHeader();
      final JournalState state = loadState();
      this.pending = state.pending;
      this.committed = state.committed;
      this.lastChain = state.lastChain;
      channel.position(channel.size());
    } catch (final IOException ex) {
      throw new OfflineJournalException(
          OfflineJournalException.Reason.IO, "failed to open offline journal", ex);
    }
  }

  /** Append a pending entry using the current wall clock timestamp. */
  public OfflineJournalEntry appendPending(final byte[] txId, final byte[] payload)
      throws OfflineJournalException {
    return appendPending(txId, payload, System.currentTimeMillis());
  }

  /** Append a pending entry with a custom timestamp (primarily for tests). */
  public OfflineJournalEntry appendPending(
      final byte[] txId, final byte[] payload, final long timestampMs)
      throws OfflineJournalException {
    final byte[] normalizedId = requireTxId(txId);
    final byte[] normalizedPayload = payload == null ? new byte[0] : payload.clone();
    lock.lock();
    try {
      final HashKey key = HashKey.of(normalizedId);
      if (pending.containsKey(key)) {
        throw new OfflineJournalException(
            OfflineJournalException.Reason.DUPLICATE_PENDING,
            "transaction already pending in offline journal");
      }
      if (committed.contains(key)) {
        throw new OfflineJournalException(
            OfflineJournalException.Reason.ALREADY_COMMITTED,
            "transaction already committed in offline journal");
      }
      final byte[] chain = computeChain(lastChain, normalizedId);
      final byte[] record =
          encodeRecord(EntryKind.PENDING, normalizedId, normalizedPayload, timestampMs, chain);
      writeRecord(record);
      lastChain = chain;
      final OfflineJournalEntry entry =
          new OfflineJournalEntry(normalizedId, normalizedPayload, timestampMs, chain);
      pending.put(key, entry);
      return entry;
    } finally {
      lock.unlock();
    }
  }

  /** Marks a pending entry as committed. */
  public void markCommitted(final byte[] txId) throws OfflineJournalException {
    markCommitted(txId, System.currentTimeMillis());
  }

  public void markCommitted(final byte[] txId, final long timestampMs)
      throws OfflineJournalException {
    final byte[] normalizedId = requireTxId(txId);
    lock.lock();
    try {
      final HashKey key = HashKey.of(normalizedId);
      if (!pending.containsKey(key)) {
        if (committed.contains(key)) {
          throw new OfflineJournalException(
              OfflineJournalException.Reason.ALREADY_COMMITTED,
              "transaction already committed in offline journal");
        }
        throw new OfflineJournalException(
            OfflineJournalException.Reason.NOT_PENDING,
            "transaction not pending in offline journal");
      }
      pending.remove(key);
      final byte[] chain = computeChain(lastChain, normalizedId);
      final byte[] record =
          encodeRecord(EntryKind.COMMITTED, normalizedId, new byte[0], timestampMs, chain);
      writeRecord(record);
      lastChain = chain;
      committed.add(key);
    } finally {
      lock.unlock();
    }
  }

  /** Returns the pending entries sorted by {@code tx_id}. */
  public List<OfflineJournalEntry> pendingEntries() {
    lock.lock();
    try {
      final List<OfflineJournalEntry> entries = new ArrayList<>(pending.values());
      entries.sort(
          Comparator.comparing(
              OfflineJournalEntry::txId,
              (a, b) -> Arrays.compareUnsigned(a, b)));
      return Collections.unmodifiableList(entries);
    } finally {
      lock.unlock();
    }
  }

  public Path path() {
    return path;
  }

  @Override
  public void close() throws OfflineJournalException {
    lock.lock();
    try {
      channel.close();
    } catch (final IOException ex) {
      throw new OfflineJournalException(
          OfflineJournalException.Reason.IO, "failed to close offline journal", ex);
    } finally {
      lock.unlock();
    }
  }

  private void ensureHeader() throws IOException, OfflineJournalException {
    channel.position(0);
    final ByteBuffer header = ByteBuffer.allocate(HEADER_LENGTH);
    int read = channel.read(header);
    if (read >= HEADER_LENGTH) {
      header.flip();
      final byte[] magic = new byte[MAGIC.length];
      header.get(magic);
      final byte version = header.get();
      if (!Arrays.equals(magic, MAGIC) || version != VERSION) {
        throw new OfflineJournalException(
            OfflineJournalException.Reason.INTEGRITY_VIOLATION,
            "offline journal header mismatch");
      }
      channel.position(channel.size());
      return;
    }
    channel.position(0);
    channel.write(ByteBuffer.wrap(MAGIC));
    channel.write(ByteBuffer.wrap(new byte[] {VERSION}));
    channel.force(true);
  }

  private JournalState loadState() throws OfflineJournalException {
    try {
      final byte[] bytes = Files.readAllBytes(path);
      if (bytes.length < HEADER_LENGTH) {
        return JournalState.empty();
      }
      final byte[] magic = Arrays.copyOfRange(bytes, 0, MAGIC.length);
      final byte version = bytes[MAGIC.length];
      if (!Arrays.equals(magic, MAGIC) || version != VERSION) {
        throw new OfflineJournalException(
            OfflineJournalException.Reason.INTEGRITY_VIOLATION,
            "offline journal header mismatch");
      }
      final Map<HashKey, OfflineJournalEntry> pendingEntries = new HashMap<>();
      final Set<HashKey> committedEntries = new HashSet<>();
      byte[] prevChain = new byte[HASH_LENGTH];
      int offset = HEADER_LENGTH;
      while (offset < bytes.length) {
        if (bytes.length - offset < MIN_RECORD_LENGTH) {
          throw new OfflineJournalException(
              OfflineJournalException.Reason.INTEGRITY_VIOLATION,
              "truncated offline journal record");
        }
        final byte kindByte = bytes[offset++];
        final long recordedAt = readUInt64LE(bytes, offset);
        offset += 8;
        final long payloadLength = readUInt32LE(bytes, offset);
        offset += 4;
        if (payloadLength > Integer.MAX_VALUE) {
          throw new OfflineJournalException(
              OfflineJournalException.Reason.PAYLOAD_TOO_LARGE,
              "payload too large in offline journal");
        }
        ensureAvailable(bytes, offset, HASH_LENGTH + (int) payloadLength + HASH_LENGTH + HMAC_LENGTH);
        final byte[] txId = Arrays.copyOfRange(bytes, offset, offset + HASH_LENGTH);
        offset += HASH_LENGTH;
        final byte[] payload =
            Arrays.copyOfRange(bytes, offset, offset + (int) payloadLength);
        offset += (int) payloadLength;
        final byte[] storedChain = Arrays.copyOfRange(bytes, offset, offset + HASH_LENGTH);
        offset += HASH_LENGTH;
        final byte[] storedHmac = Arrays.copyOfRange(bytes, offset, offset + HMAC_LENGTH);
        offset += HMAC_LENGTH;

        final EntryKind kind = EntryKind.fromByte(kindByte);
        final byte[] record =
            buildRecordWithoutHmac(kindByte, recordedAt, payloadLength, txId, payload, storedChain);
        final byte[] expectedChain = computeChain(prevChain, txId);
        if (!Arrays.equals(storedChain, expectedChain)) {
          throw new OfflineJournalException(
              OfflineJournalException.Reason.INTEGRITY_VIOLATION,
              "hash chain mismatch for tx " + toHex(txId));
        }
        final byte[] expectedHmac = computeHmac(prevChain, record);
        if (!Arrays.equals(storedHmac, expectedHmac)) {
          throw new OfflineJournalException(
              OfflineJournalException.Reason.INTEGRITY_VIOLATION,
              "HMAC mismatch for tx " + toHex(txId));
        }
        final HashKey key = HashKey.of(txId);
        if (kind == EntryKind.PENDING) {
          final OfflineJournalEntry entry =
              new OfflineJournalEntry(txId, payload, recordedAt, storedChain);
          pendingEntries.put(key, entry);
        } else {
          pendingEntries.remove(key);
          committedEntries.add(key);
        }
        prevChain = storedChain;
      }
      return new JournalState(pendingEntries, committedEntries, prevChain);
    } catch (final IOException ex) {
      throw new OfflineJournalException(
          OfflineJournalException.Reason.IO, "failed to load offline journal", ex);
    }
  }

  private byte[] encodeRecord(
      final EntryKind kind,
      final byte[] txId,
      final byte[] payload,
      final long timestampMs,
      final byte[] chain)
      throws OfflineJournalException {
    if (payload.length > 0xFFFF_FFFFL) {
      throw new OfflineJournalException(
          OfflineJournalException.Reason.PAYLOAD_TOO_LARGE,
          "offline journal payload exceeds 4 GiB");
    }
    final long payloadLen = payload.length;
    final byte[] record =
        buildRecordWithoutHmac(kind.tag, timestampMs, payloadLen, txId, payload, chain);
    final byte[] hmac = computeHmac(lastChain, record);
    final byte[] encoded = Arrays.copyOf(record, record.length + HMAC_LENGTH);
    System.arraycopy(hmac, 0, encoded, record.length, HMAC_LENGTH);
    return encoded;
  }

  private void writeRecord(final byte[] record) throws OfflineJournalException {
    try {
      channel.position(channel.size());
      channel.write(ByteBuffer.wrap(record));
      channel.force(true);
    } catch (final IOException ex) {
      throw new OfflineJournalException(
          OfflineJournalException.Reason.IO, "failed to persist offline journal record", ex);
    }
  }

  private static byte[] requireTxId(final byte[] txId) throws OfflineJournalException {
    if (txId == null || txId.length != HASH_LENGTH) {
      throw new OfflineJournalException(
          OfflineJournalException.Reason.INVALID_TX_ID_LENGTH,
          "offline journal tx_id must be exactly 32 bytes");
    }
    return txId.clone();
  }

  private static void ensureAvailable(
      final byte[] bytes, final int offset, final int expectedRemaining)
      throws OfflineJournalException {
    if (offset + expectedRemaining > bytes.length) {
      throw new OfflineJournalException(
          OfflineJournalException.Reason.INTEGRITY_VIOLATION, "truncated offline journal record");
    }
  }

  private static byte[] computeChain(final byte[] prevChain, final byte[] txId) {
    final byte[] input = new byte[prevChain.length + txId.length];
    System.arraycopy(prevChain, 0, input, 0, prevChain.length);
    System.arraycopy(txId, 0, input, prevChain.length, txId.length);
    return Blake2b.digest(input, HASH_LENGTH);
  }

  private byte[] computeHmac(final byte[] prevChain, final byte[] record)
      throws OfflineJournalException {
    try {
      final Mac mac = Mac.getInstance("HmacSHA256");
      mac.init(new SecretKeySpec(key, "HmacSHA256"));
      mac.update(prevChain);
      mac.update(record);
      return mac.doFinal();
    } catch (final GeneralSecurityException ex) {
      throw new OfflineJournalException(
          OfflineJournalException.Reason.IO, "failed to compute offline journal HMAC", ex);
    }
  }

  private static byte[] buildRecordWithoutHmac(
      final byte kind,
      final long timestampMs,
      final long payloadLen,
      final byte[] txId,
      final byte[] payload,
      final byte[] chain) {
    final ByteBuffer buffer =
        ByteBuffer.allocate(
                1 + 8 + 4 + HASH_LENGTH + (int) payloadLen + HASH_LENGTH)
            .order(ByteOrder.LITTLE_ENDIAN);
    buffer.put(kind);
    buffer.putLong(timestampMs);
    buffer.putInt((int) payloadLen);
    buffer.put(txId);
    buffer.put(payload);
    buffer.put(chain);
    return buffer.array();
  }

  private static long readUInt32LE(final byte[] bytes, final int offset) {
    final long value = ((long) bytes[offset] & 0xFF)
        | (((long) bytes[offset + 1] & 0xFF) << 8)
        | (((long) bytes[offset + 2] & 0xFF) << 16)
        | (((long) bytes[offset + 3] & 0xFF) << 24);
    return value & 0xFFFF_FFFFL;
  }

  private static long readUInt64LE(final byte[] bytes, final int offset) {
    long value = 0;
    for (int i = 0; i < 8; i++) {
      value |= ((long) bytes[offset + i] & 0xFF) << (i * 8);
    }
    return value;
  }

  private static String toHex(final byte[] bytes) {
    final StringBuilder builder = new StringBuilder(bytes.length * 2);
    for (final byte b : bytes) {
      builder.append(String.format("%02x", b));
    }
    return builder.toString();
  }

  private static final class JournalState {
    final Map<HashKey, OfflineJournalEntry> pending;
    final Set<HashKey> committed;
    final byte[] lastChain;

    JournalState(
        final Map<HashKey, OfflineJournalEntry> pending,
        final Set<HashKey> committed,
        final byte[] lastChain) {
      this.pending = pending;
      this.committed = committed;
      this.lastChain = lastChain;
    }

    static JournalState empty() {
      return new JournalState(
          new HashMap<>(), new HashSet<>(), new byte[HASH_LENGTH]);
    }
  }

  private static final class HashKey {
    private final byte[] value;
    private final int hashCode;

    private HashKey(final byte[] value) {
      this.value = value;
      this.hashCode = Arrays.hashCode(value);
    }

    static HashKey of(final byte[] value) {
      return new HashKey(Arrays.copyOf(value, value.length));
    }

    @Override
    public boolean equals(final Object obj) {
      if (this == obj) {
        return true;
      }
      if (!(obj instanceof HashKey other)) {
        return false;
      }
      return Arrays.equals(value, other.value);
    }

    @Override
    public int hashCode() {
      return hashCode;
    }
  }
}

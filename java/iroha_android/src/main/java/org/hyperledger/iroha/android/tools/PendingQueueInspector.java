package org.hyperledger.iroha.android.tools;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Instant;
import java.time.ZoneOffset;
import java.time.format.DateTimeFormatter;
import java.util.ArrayList;
import java.util.Base64;
import java.util.List;
import java.util.Locale;
import java.util.Optional;
import java.util.OptionalLong;
import java.util.concurrent.atomic.AtomicInteger;
import org.hyperledger.iroha.android.client.queue.FilePendingTransactionQueue;
import org.hyperledger.iroha.android.tx.SignedTransaction;
import org.hyperledger.iroha.android.tx.SignedTransactionHasher;
import org.hyperledger.iroha.android.tx.offline.OfflineSigningEnvelope;
import org.hyperledger.iroha.android.tx.offline.OfflineSigningEnvelopeCodec;

/**
 * Utility that inspects a pending queue file produced by {@link
 * FilePendingTransactionQueue}. Useful during AND7 queue-replay drills where operators must capture
 * hashed payloads and alias metadata for incident artefacts.
 */
public final class PendingQueueInspector {

  private static final Base64.Decoder DECODER = Base64.getDecoder();
  private static final OfflineSigningEnvelopeCodec ENVELOPE_CODEC =
      new OfflineSigningEnvelopeCodec();
  private static final DateTimeFormatter ISO_FORMATTER =
      DateTimeFormatter.ISO_OFFSET_DATE_TIME.withLocale(Locale.ROOT);
  private static final String DEFAULT_ALIAS = "pending.queue";

  private PendingQueueInspector() {}

  public static void main(final String[] args) throws Exception {
    final Arguments arguments = Arguments.parse(args);
    if (arguments.queueFile() == null) {
      printUsage();
      System.exit(1);
    }
    final List<EntrySummary> entries = inspect(arguments.queueFile());
    if (arguments.jsonOutput()) {
      System.out.println(toJson(entries));
    } else {
      printHuman(entries);
    }
  }

  /** Inspect a queue file and return decoded entry summaries (exposed for tests). */
  public static List<EntrySummary> inspect(final Path queueFile) throws IOException {
    if (queueFile == null) {
      throw new IllegalArgumentException("queueFile must not be null");
    }
    if (!Files.exists(queueFile)) {
      throw new IOException("queue file does not exist: " + queueFile);
    }
    final List<String> lines = Files.readAllLines(queueFile, StandardCharsets.UTF_8);
    final List<EntrySummary> summaries = new ArrayList<>();
    final AtomicInteger index = new AtomicInteger();
    for (final String line : lines) {
      if (line == null || line.isBlank()) {
        continue;
      }
      summaries.add(decodeLine(line.trim(), index.getAndIncrement()));
    }
    return summaries;
  }

  private static EntrySummary decodeLine(final String line, final int index) throws IOException {
    final byte[] bytes;
    try {
      bytes = DECODER.decode(line);
    } catch (final IllegalArgumentException ex) {
      throw new IOException("Failed to decode queue entry " + index, ex);
    }
    try {
      final OfflineSigningEnvelope envelope = ENVELOPE_CODEC.decode(bytes);
      final SignedTransaction transaction =
          new SignedTransaction(
              envelope.encodedPayload(),
              envelope.signature(),
              envelope.publicKey(),
              envelope.schemaName(),
              envelope.keyAlias(),
              envelope.exportedKeyBundle().orElse(null));
      final OptionalLong issuedAt =
          envelope.issuedAtMs() >= 0
              ? OptionalLong.of(envelope.issuedAtMs())
              : OptionalLong.empty();
      return new EntrySummary(
          index,
          SignedTransactionHasher.hashHex(transaction),
          transaction.schemaName(),
          transaction.keyAlias().orElse(DEFAULT_ALIAS),
          issuedAt,
          envelope.exportedKeyBundle().isPresent());
    } catch (final Exception ex) {
      throw new IOException("Failed to decode queue entry " + index, ex);
    }
  }

  private static void printUsage() {
    System.err.println(
        """
Usage: PendingQueueInspector --file <path> [--json]

Options:
  --file <path>   Path to the pending queue file (required)
  --json          Emit JSON array instead of human-readable summary

Example:
  java -cp build/classes org.hyperledger.iroha.android.tools.PendingQueueInspector \\
      --file /tmp/pixel8/pending.queue --json
""");
  }

  private static void printHuman(final List<EntrySummary> entries) {
    System.out.printf("Queue entries: %d%n", entries.size());
    for (final EntrySummary entry : entries) {
      System.out.printf(
          Locale.ROOT,
          "[%d] hash=%s schema=%s alias=%s issued_at=%s exported_key_bundle=%s%n",
          entry.index(),
          entry.hashHex(),
          entry.schemaName(),
          entry.keyAlias(),
          entry.issuedAtIso().orElse("unknown"),
          entry.hasExportedKeyBundle());
    }
  }

  private static String toJson(final List<EntrySummary> entries) {
    final StringBuilder builder = new StringBuilder();
    builder.append('[');
    for (int i = 0; i < entries.size(); i++) {
      if (i > 0) {
        builder.append(',');
      }
      builder.append(entries.get(i).toJson());
    }
    builder.append(']');
    return builder.toString();
  }

  private record Arguments(Path queueFile, boolean jsonOutput) {
    static Arguments parse(final String[] args) {
      Path file = null;
      boolean json = false;
      for (int i = 0; i < args.length; i++) {
        final String arg = args[i];
        if ("--file".equals(arg) && i + 1 < args.length) {
          file = Path.of(args[++i]);
        } else if ("--json".equals(arg)) {
          json = true;
        } else {
          System.err.printf("Unknown argument: %s%n", arg);
        }
      }
      return new Arguments(file, json);
    }
  }

  /** Summary metadata for a queued transaction. */
  public static final class EntrySummary {
    private final int index;
    private final String hashHex;
    private final String schemaName;
    private final String keyAlias;
    private final OptionalLong issuedAtMs;
    private final boolean hasExportedKeyBundle;

    EntrySummary(
        final int index,
        final String hashHex,
        final String schemaName,
        final String keyAlias,
        final OptionalLong issuedAtMs,
        final boolean hasExportedKeyBundle) {
      this.index = index;
      this.hashHex = hashHex;
      this.schemaName = schemaName;
      this.keyAlias = keyAlias;
      this.issuedAtMs = issuedAtMs;
      this.hasExportedKeyBundle = hasExportedKeyBundle;
    }

    public int index() {
      return index;
    }

    public String hashHex() {
      return hashHex;
    }

    public String schemaName() {
      return schemaName;
    }

    public String keyAlias() {
      return keyAlias;
    }

    public OptionalLong issuedAtMs() {
      return issuedAtMs;
    }

    public boolean hasExportedKeyBundle() {
      return hasExportedKeyBundle;
    }

    public Optional<String> issuedAtIso() {
      if (issuedAtMs.isPresent()) {
        final Instant instant = Instant.ofEpochMilli(issuedAtMs.getAsLong());
        return Optional.of(ISO_FORMATTER.format(instant.atOffset(ZoneOffset.UTC)));
      }
      return Optional.empty();
    }

    private String toJson() {
      final StringBuilder builder = new StringBuilder();
      builder.append('{');
      builder.append("\"index\":").append(index).append(',');
      builder.append("\"hash_hex\":\"").append(hashHex).append("\",");
      builder.append("\"schema\":\"").append(escape(schemaName)).append("\",");
      builder.append("\"alias\":\"").append(escape(keyAlias)).append("\",");
      builder.append("\"issued_at_ms\":");
      if (issuedAtMs.isPresent()) {
        builder.append(issuedAtMs.getAsLong());
      } else {
        builder.append("null");
      }
      builder.append(',');
      builder.append("\"has_exported_key_bundle\":").append(hasExportedKeyBundle);
      builder.append('}');
      return builder.toString();
    }

    private static String escape(final String value) {
      return value.replace("\\", "\\\\").replace("\"", "\\\"");
    }
  }
}

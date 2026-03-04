package org.hyperledger.iroha.android.offline;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Instant;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.TreeMap;
import org.hyperledger.iroha.android.crypto.Blake2b;

public final class OfflineCounterJournalTest {

  private OfflineCounterJournalTest() {}

  public static void main(final String[] args) throws Exception {
    persistsSummaries();
    rejectsSummaryMismatch();
    enforcesMonotonicCounters();
    rejectsFractionalCounters();
    rejectsFractionalRecordedAt();
    System.out.println("[IrohaAndroid] OfflineCounterJournalTest passed.");
  }

  private static void persistsSummaries() throws IOException {
    final Path tempFile = Files.createTempFile("offline-counter-journal", ".json");
    try {
      final Map<String, Long> apple = new LinkedHashMap<>();
      apple.put("app.attest:k1", 17L);
      apple.put("app.attest:k9", 23L);
      final Map<String, Long> android = new LinkedHashMap<>();
      android.put("pixel-7a", 5L);
      android.put("pixel-8", 9L);
      final String summaryHash = computeSummaryHashHex(apple, android);
      final OfflineSummaryList.OfflineSummaryItem item =
          new OfflineSummaryList.OfflineSummaryItem(
              "deadbeef",
              "alice@wonderland",
              "Alice",
              summaryHash,
              apple,
              android);
      final OfflineCounterJournal journal = new OfflineCounterJournal(tempFile);
      journal.upsert(List.of(item), Instant.ofEpochMilli(1_700_000_000_000L));
      final OfflineCounterJournal.OfflineCounterCheckpoint checkpoint =
          journal.find("deadbeef").orElseThrow(() -> new IllegalStateException("missing checkpoint"));
      assert summaryHash.equals(checkpoint.summaryHashHex()) : "summary hash mismatch";
      assert checkpoint.appleKeyCounters().get("app.attest:k1") == 17L : "apple counter mismatch";
      assert checkpoint.androidSeriesCounters().get("pixel-7a") == 5L : "android counter mismatch";
      assert checkpoint.recordedAtMs() == 1_700_000_000_000L : "recorded_at_ms mismatch";

      final OfflineCounterJournal reloaded = new OfflineCounterJournal(tempFile);
      assert reloaded.find("deadbeef").isPresent() : "checkpoint did not persist";
    } finally {
      Files.deleteIfExists(tempFile);
    }
  }

  private static void rejectsSummaryMismatch() throws IOException {
    final Path tempFile = Files.createTempFile("offline-counter-journal", ".json");
    try {
      final Map<String, Long> apple = Map.of("app.attest:k1", 1L);
      final OfflineSummaryList.OfflineSummaryItem item =
          new OfflineSummaryList.OfflineSummaryItem(
              "deadbeef",
              "alice@wonderland",
              "Alice",
              "00".repeat(32),
              apple,
              Map.of());
      final OfflineCounterJournal journal = new OfflineCounterJournal(tempFile);
      boolean failed = false;
      try {
        journal.upsert(List.of(item), Instant.now());
      } catch (OfflineCounterException error) {
        failed = true;
        assert error.reason() == OfflineCounterException.Reason.SUMMARY_HASH_MISMATCH
            : "expected summary hash mismatch";
      }
      assert failed : "expected summary hash mismatch error";
    } finally {
      Files.deleteIfExists(tempFile);
    }
  }

  private static void enforcesMonotonicCounters() throws IOException {
    final Path tempFile = Files.createTempFile("offline-counter-journal", ".json");
    try {
      final OfflineCounterJournal journal = new OfflineCounterJournal(tempFile);
      journal.advanceAppleCounter(
          "deadbeef",
          "alice@wonderland",
          "Alice",
          "app.attest:k1",
          1L,
          Instant.ofEpochMilli(1_700_000_000_000L));
      boolean failed = false;
      try {
        journal.advanceAppleCounter(
            "deadbeef",
            "alice@wonderland",
            "Alice",
            "app.attest:k1",
            3L,
            Instant.ofEpochMilli(1_700_000_000_100L));
      } catch (OfflineCounterException error) {
        failed = true;
        assert error.reason() == OfflineCounterException.Reason.COUNTER_VIOLATION
            : "expected counter violation";
      }
      assert failed : "expected counter violation error";
    } finally {
      Files.deleteIfExists(tempFile);
    }
  }

  private static void rejectsFractionalCounters() throws IOException {
    final Path tempFile = Files.createTempFile("offline-counter-journal-invalid", ".json");
    final String json =
        """
        {
          "deadbeef": {
            "controller_id": "alice@wonderland",
            "controller_display": "Alice",
            "summary_hash_hex": "00",
            "apple_key_counters": { "app.attest:k1": 1.5 },
            "android_series_counters": {},
            "recorded_at_ms": 1
          }
        }
        """;
    Files.writeString(tempFile, json, StandardCharsets.UTF_8);
    boolean thrown = false;
    try {
      new OfflineCounterJournal(tempFile);
    } catch (Exception ex) {
      thrown = true;
    }
    assert thrown : "expected fractional counters to be rejected";
    Files.deleteIfExists(tempFile);
  }

  private static void rejectsFractionalRecordedAt() throws IOException {
    final Path tempFile = Files.createTempFile("offline-counter-journal-invalid", ".json");
    final String json =
        """
        {
          "deadbeef": {
            "controller_id": "alice@wonderland",
            "controller_display": "Alice",
            "summary_hash_hex": "00",
            "apple_key_counters": { "app.attest:k1": 1 },
            "android_series_counters": {},
            "recorded_at_ms": 1.5
          }
        }
        """;
    Files.writeString(tempFile, json, StandardCharsets.UTF_8);
    boolean thrown = false;
    try {
      new OfflineCounterJournal(tempFile);
    } catch (Exception ex) {
      thrown = true;
    }
    assert thrown : "expected fractional recorded_at_ms to be rejected";
    Files.deleteIfExists(tempFile);
  }

  private static String computeSummaryHashHex(
      final Map<String, Long> apple, final Map<String, Long> android) {
    final byte[] payload = encodeSummaryPayload(apple, android);
    final byte[] digest = Blake2b.digest256(payload);
    digest[digest.length - 1] |= 1;
    return toHex(digest);
  }

  private static byte[] encodeSummaryPayload(
      final Map<String, Long> apple, final Map<String, Long> android) {
    final byte[] applePayload = encodeCounterMap(apple);
    final byte[] androidPayload = encodeCounterMap(android);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeU64(out, applePayload.length);
    out.writeBytes(applePayload);
    writeU64(out, androidPayload.length);
    out.writeBytes(androidPayload);
    return out.toByteArray();
  }

  private static byte[] encodeCounterMap(final Map<String, Long> map) {
    final Map<String, Long> sorted = new TreeMap<>(map);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeU64(out, sorted.size());
    for (final Map.Entry<String, Long> entry : sorted.entrySet()) {
      final byte[] keyPayload = encodeString(entry.getKey());
      writeU64(out, keyPayload.length);
      out.writeBytes(keyPayload);
      final byte[] valuePayload = encodeU64(entry.getValue());
      writeU64(out, valuePayload.length);
      out.writeBytes(valuePayload);
    }
    return out.toByteArray();
  }

  private static byte[] encodeString(final String value) {
    final byte[] bytes = value.getBytes(StandardCharsets.UTF_8);
    final ByteArrayOutputStream out = new ByteArrayOutputStream();
    writeU64(out, bytes.length);
    out.writeBytes(bytes);
    return out.toByteArray();
  }

  private static byte[] encodeU64(final long value) {
    final byte[] out = new byte[8];
    long v = value;
    for (int i = 0; i < 8; i++) {
      out[i] = (byte) (v & 0xFF);
      v >>>= 8;
    }
    return out;
  }

  private static void writeU64(final ByteArrayOutputStream out, final long value) {
    out.writeBytes(encodeU64(value));
  }

  private static String toHex(final byte[] data) {
    final StringBuilder builder = new StringBuilder(data.length * 2);
    for (final byte b : data) {
      builder.append(String.format(Locale.ROOT, "%02x", b));
    }
    return builder.toString();
  }
}

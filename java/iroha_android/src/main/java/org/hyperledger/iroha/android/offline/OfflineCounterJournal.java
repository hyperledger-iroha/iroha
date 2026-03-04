package org.hyperledger.iroha.android.offline;

import java.io.ByteArrayOutputStream;
import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.time.Instant;
import java.util.ArrayList;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.TreeMap;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.crypto.Blake2b;

/** File-backed store for offline platform counters and summary hashes. */
public final class OfflineCounterJournal {

  private static final String PROVISIONED_PREFIX = "provisioned::";

  private final Path journalFile;
  private final Object lock = new Object();
  private final Map<String, OfflineCounterCheckpoint> entries = new LinkedHashMap<>();

  public OfflineCounterJournal(final Path journalFile) throws IOException {
    this.journalFile = Objects.requireNonNull(journalFile, "journalFile");
    final Path parent = journalFile.getParent();
    if (parent != null) {
      Files.createDirectories(parent);
    }
    if (Files.exists(journalFile) && Files.size(journalFile) > 0) {
      loadExisting();
    }
  }

  public Path journalFile() {
    return journalFile;
  }

  public List<OfflineCounterCheckpoint> entries() {
    synchronized (lock) {
      return List.copyOf(entries.values());
    }
  }

  public Optional<OfflineCounterCheckpoint> find(final String certificateIdHex) {
    if (certificateIdHex == null || certificateIdHex.trim().isEmpty()) {
      return Optional.empty();
    }
    synchronized (lock) {
      final OfflineCounterCheckpoint direct = entries.get(certificateIdHex);
      if (direct != null) {
        return Optional.of(direct);
      }
      final String normalized = certificateIdHex.trim();
      for (final Map.Entry<String, OfflineCounterCheckpoint> entry : entries.entrySet()) {
        if (entry.getKey().equalsIgnoreCase(normalized)) {
          return Optional.of(entry.getValue());
        }
      }
      return Optional.empty();
    }
  }

  /** Exports the stored checkpoints as canonical JSON. */
  public byte[] exportJson() {
    synchronized (lock) {
      return JsonEncoder.encode(serializeEntriesLocked()).getBytes(StandardCharsets.UTF_8);
    }
  }

  public List<OfflineCounterCheckpoint> upsert(
      final OfflineSummaryList summaryList, final Instant recordedAt) throws IOException {
    Objects.requireNonNull(summaryList, "summaryList");
    return upsert(summaryList.items(), recordedAt);
  }

  public List<OfflineCounterCheckpoint> upsert(
      final List<OfflineSummaryList.OfflineSummaryItem> summaries, final Instant recordedAt)
      throws IOException {
    Objects.requireNonNull(summaries, "summaries");
    final long recordedAtMs = Objects.requireNonNull(recordedAt, "recordedAt").toEpochMilli();
    synchronized (lock) {
      final List<OfflineCounterCheckpoint> inserted = new ArrayList<>(summaries.size());
      for (final OfflineSummaryList.OfflineSummaryItem summary : summaries) {
        final String certificateId = normalizeHex(summary.certificateIdHex());
        final String computed =
            computeSummaryHashHex(summary.appleKeyCounters(), summary.androidSeriesCounters());
        final String expected = normalizeHex(summary.summaryHashHex());
        if (!computed.equalsIgnoreCase(expected)) {
          throw new OfflineCounterException(
              OfflineCounterException.Reason.SUMMARY_HASH_MISMATCH,
              "summary hash mismatch for " + certificateId + ": expected " + expected + ", got " + computed);
        }
        final OfflineCounterCheckpoint checkpoint =
            new OfflineCounterCheckpoint(
                certificateId,
                summary.controllerId(),
                summary.controllerDisplay(),
                computed,
                summary.appleKeyCounters(),
                summary.androidSeriesCounters(),
                recordedAtMs);
        entries.put(certificateId, checkpoint);
        inserted.add(checkpoint);
      }
      persistLocked();
      return inserted;
    }
  }

  public OfflineCounterCheckpoint advanceAppleCounter(
      final String certificateIdHex,
      final String controllerId,
      final String controllerDisplay,
      final String keyId,
      final long counter,
      final Instant recordedAt) throws IOException {
    return advanceCounter(
        certificateIdHex,
        controllerId,
        controllerDisplay,
        OfflineCounterPlatform.APPLE_KEY,
        keyId,
        counter,
        recordedAt);
  }

  public OfflineCounterCheckpoint advanceAndroidSeriesCounter(
      final String certificateIdHex,
      final String controllerId,
      final String controllerDisplay,
      final String series,
      final long counter,
      final Instant recordedAt) throws IOException {
    return advanceCounter(
        certificateIdHex,
        controllerId,
        controllerDisplay,
        OfflineCounterPlatform.ANDROID_SERIES,
        series,
        counter,
        recordedAt);
  }

  public OfflineCounterCheckpoint advanceProvisionedCounter(
      final String certificateIdHex,
      final String controllerId,
      final String controllerDisplay,
      final AndroidProvisionedProof proof,
      final Instant recordedAt) throws IOException {
    Objects.requireNonNull(proof, "proof");
    final String schema = proof.manifestSchema();
    final String deviceId = proof.deviceId();
    if (schema == null || schema.trim().isEmpty() || deviceId == null || deviceId.trim().isEmpty()) {
      throw new OfflineCounterException(
          OfflineCounterException.Reason.INVALID_SCOPE,
          "provisioned manifest schema/device_id missing");
    }
    final String scope = PROVISIONED_PREFIX + schema.trim() + "::" + deviceId.trim();
    return advanceCounter(
        certificateIdHex,
        controllerId,
        controllerDisplay,
        OfflineCounterPlatform.ANDROID_SERIES,
        scope,
        proof.counter(),
        recordedAt);
  }

  private OfflineCounterCheckpoint advanceCounter(
      final String certificateIdHex,
      final String controllerId,
      final String controllerDisplay,
    final OfflineCounterPlatform platform,
    final String scope,
    final long counter,
    final Instant recordedAt) throws IOException {
    Objects.requireNonNull(platform, "platform");
    final String normalizedCert = normalizeHex(certificateIdHex);
    final String trimmedScope = scope == null ? "" : scope.trim();
    if (normalizedCert.trim().isEmpty()) {
      throw new OfflineCounterException(
          OfflineCounterException.Reason.INVALID_SCOPE,
          "certificate_id_hex must not be empty");
    }
    if (trimmedScope.isEmpty()) {
      throw new OfflineCounterException(
          OfflineCounterException.Reason.INVALID_SCOPE,
          "counter scope must not be empty");
    }
    if (counter < 0) {
      throw new OfflineCounterException(
          OfflineCounterException.Reason.INVALID_SCOPE,
          "counter must be non-negative");
    }
    final long recordedAtMs = Objects.requireNonNull(recordedAt, "recordedAt").toEpochMilli();
    synchronized (lock) {
      final OfflineCounterCheckpoint existing = entries.get(normalizedCert);
      final Map<String, Long> apple =
          existing == null ? new LinkedHashMap<>() : new LinkedHashMap<>(existing.appleKeyCounters());
      final Map<String, Long> android =
          existing == null ? new LinkedHashMap<>() : new LinkedHashMap<>(existing.androidSeriesCounters());
      final Map<String, Long> target =
          platform == OfflineCounterPlatform.APPLE_KEY ? apple : android;
      if (target.containsKey(trimmedScope)) {
        final long previous = target.get(trimmedScope);
        final long expected = previous + 1;
        if (counter != expected) {
          throw new OfflineCounterException(
              OfflineCounterException.Reason.COUNTER_VIOLATION,
              "counter jump for " + trimmedScope + ": expected " + expected + ", got " + counter);
        }
      }
      target.put(trimmedScope, counter);
      final String summaryHash = computeSummaryHashHex(apple, android);
      final String resolvedControllerId =
          existing != null
              && existing.controllerId() != null
              && !existing.controllerId().trim().isEmpty()
              ? existing.controllerId()
              : controllerId;
      final String resolvedControllerDisplay =
          existing != null
              && existing.controllerDisplay() != null
              && !existing.controllerDisplay().trim().isEmpty()
              ? existing.controllerDisplay()
              : controllerDisplay;
      final OfflineCounterCheckpoint checkpoint =
          new OfflineCounterCheckpoint(
              normalizedCert,
              resolvedControllerId,
              resolvedControllerDisplay,
              summaryHash,
              apple,
              android,
              recordedAtMs);
      entries.put(normalizedCert, checkpoint);
      persistLocked();
      return checkpoint;
    }
  }

  private void loadExisting() throws IOException {
    final String json = new String(Files.readAllBytes(journalFile), StandardCharsets.UTF_8).trim();
    if (json.isEmpty()) {
      return;
    }
    final Object root = JsonParser.parse(json);
    if (!(root instanceof Map<?, ?> map)) {
      throw new IOException("Counter journal must be a JSON object");
    }
    entries.clear();
    for (final Map.Entry<?, ?> entry : map.entrySet()) {
      final String certificateId = Objects.toString(entry.getKey(), null);
      if (certificateId == null) {
        continue;
      }
      final OfflineCounterCheckpoint checkpoint =
          OfflineCounterCheckpoint.fromJson(certificateId, entry.getValue());
      entries.put(certificateId, checkpoint);
    }
  }

  private void persistLocked() throws IOException {
    final Map<String, Object> serialized = serializeEntriesLocked();
    Files.write(
        journalFile,
        JsonEncoder.encode(serialized).getBytes(StandardCharsets.UTF_8),
        StandardOpenOption.CREATE,
        StandardOpenOption.TRUNCATE_EXISTING);
  }

  private Map<String, Object> serializeEntriesLocked() {
    final Map<String, Object> serialized = new LinkedHashMap<>(entries.size());
    for (final Map.Entry<String, OfflineCounterCheckpoint> entry : entries.entrySet()) {
      serialized.put(entry.getKey(), entry.getValue().toJson());
    }
    return serialized;
  }

  private static String normalizeHex(final String value) {
    if (value == null) {
      return "";
    }
    String trimmed = value.trim();
    if (trimmed.startsWith("0x") || trimmed.startsWith("0X")) {
      trimmed = trimmed.substring(2);
    }
    return trimmed.toLowerCase(Locale.ROOT);
  }

  private static String computeSummaryHashHex(
      final Map<String, Long> apple, final Map<String, Long> android) {
    final byte[] payload = encodeSummaryPayload(apple, android);
    final byte[] digest = irohaHash(payload);
    return toHex(digest);
  }

  private static byte[] irohaHash(final byte[] payload) {
    final byte[] digest = Blake2b.digest256(payload);
    digest[digest.length - 1] |= 1;
    return digest;
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

  private enum OfflineCounterPlatform {
    APPLE_KEY,
    ANDROID_SERIES
  }

  public static final class OfflineCounterCheckpoint {
    private final String certificateIdHex;
    private final String controllerId;
    private final String controllerDisplay;
    private final String summaryHashHex;
    private final Map<String, Long> appleKeyCounters;
    private final Map<String, Long> androidSeriesCounters;
    private final long recordedAtMs;

    public OfflineCounterCheckpoint(
        final String certificateIdHex,
        final String controllerId,
        final String controllerDisplay,
        final String summaryHashHex,
        final Map<String, Long> appleKeyCounters,
        final Map<String, Long> androidSeriesCounters,
        final long recordedAtMs) {
      this.certificateIdHex = certificateIdHex;
      this.controllerId = controllerId;
      this.controllerDisplay = controllerDisplay;
      this.summaryHashHex = summaryHashHex;
      this.appleKeyCounters = Map.copyOf(new LinkedHashMap<>(appleKeyCounters));
      this.androidSeriesCounters = Map.copyOf(new LinkedHashMap<>(androidSeriesCounters));
      this.recordedAtMs = recordedAtMs;
    }

    public String certificateIdHex() {
      return certificateIdHex;
    }

    public String controllerId() {
      return controllerId;
    }

    public String controllerDisplay() {
      return controllerDisplay;
    }

    public String summaryHashHex() {
      return summaryHashHex;
    }

    public Map<String, Long> appleKeyCounters() {
      return appleKeyCounters;
    }

    public Map<String, Long> androidSeriesCounters() {
      return androidSeriesCounters;
    }

    public long recordedAtMs() {
      return recordedAtMs;
    }

    Map<String, Object> toJson() {
      final Map<String, Object> map = new LinkedHashMap<>();
      map.put("controller_id", controllerId);
      map.put("controller_display", controllerDisplay);
      map.put("summary_hash_hex", summaryHashHex);
      map.put("apple_key_counters", appleKeyCounters);
      map.put("android_series_counters", androidSeriesCounters);
      map.put("recorded_at_ms", recordedAtMs);
      return map;
    }

    @SuppressWarnings("unchecked")
    static OfflineCounterCheckpoint fromJson(final String certificateIdHex, final Object value) {
      if (!(value instanceof Map<?, ?> raw)) {
        throw new IllegalStateException("counter entry for " + certificateIdHex + " is not an object");
      }
      final String controllerId = Objects.toString(raw.get("controller_id"), "");
      final String controllerDisplay = Objects.toString(raw.get("controller_display"), null);
      final String summaryHash = Objects.toString(raw.get("summary_hash_hex"), "");
      final Map<String, Long> apple =
          raw.get("apple_key_counters") instanceof Map<?, ?> map
              ? asCounterMap(map)
              : Map.of();
      final Map<String, Long> android =
          raw.get("android_series_counters") instanceof Map<?, ?> map
              ? asCounterMap(map)
              : Map.of();
      final long recordedAtMs = optionalLong(raw.get("recorded_at_ms"), "recorded_at_ms");
      return new OfflineCounterCheckpoint(
          normalizeHex(certificateIdHex),
          controllerId,
          controllerDisplay,
          summaryHash,
          apple,
          android,
          recordedAtMs);
    }

    private static Map<String, Long> asCounterMap(final Map<?, ?> map) {
      final Map<String, Long> result = new LinkedHashMap<>();
      for (final Map.Entry<?, ?> entry : map.entrySet()) {
        if (entry.getKey() == null || entry.getValue() == null) {
          continue;
        }
        final String key = String.valueOf(entry.getKey());
        final long value = requireLong(entry.getValue(), "counter." + key);
        result.put(key, value);
      }
      return result;
    }

    private static long optionalLong(final Object value, final String path) {
      if (value == null) {
        return 0L;
      }
      return requireLong(value, path);
    }

    private static long requireLong(final Object value, final String path) {
      if (value instanceof Number number) {
        if (number instanceof Float || number instanceof Double) {
          throw new IllegalStateException(path + " must be an integer");
        }
        return number.longValue();
      }
      try {
        return Long.parseLong(String.valueOf(value));
      } catch (final NumberFormatException ex) {
        throw new IllegalStateException(path + " is not numeric", ex);
      }
    }
  }
}

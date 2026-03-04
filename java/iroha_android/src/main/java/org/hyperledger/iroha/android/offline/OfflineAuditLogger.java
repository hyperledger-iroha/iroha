package org.hyperledger.iroha.android.offline;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.nio.file.StandardOpenOption;
import java.util.ArrayList;
import java.util.Collections;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.JsonParser;

/**
 * File-backed audit log that mirrors the iOS SDK toggle. Entries are persisted as a JSON array so
 * operators can export deterministic payloads for regulators.
 */
public final class OfflineAuditLogger {

  private final Path logFile;
  private final Object lock = new Object();
  private volatile boolean enabled;
  private List<OfflineAuditEntry> entries;

  public OfflineAuditLogger(final Path logFile, final boolean enabled) throws IOException {
    this.logFile = Objects.requireNonNull(logFile, "logFile");
    this.enabled = enabled;
    this.entries = new ArrayList<>();
    final Path parent = logFile.getParent();
    if (parent != null) {
      Files.createDirectories(parent);
    }
    if (Files.exists(logFile) && Files.size(logFile) > 0) {
      loadExisting();
    }
  }

  public boolean isEnabled() {
    return enabled;
  }

  public void setEnabled(final boolean enabled) {
    this.enabled = enabled;
  }

  public Path logFile() {
    return logFile;
  }

  public void record(final OfflineAuditEntry entry) throws IOException {
    Objects.requireNonNull(entry, "entry");
    if (!enabled) {
      return;
    }
    synchronized (lock) {
      entries.add(entry);
      persist();
    }
  }

  public List<OfflineAuditEntry> entries() {
    synchronized (lock) {
      return Collections.unmodifiableList(new ArrayList<>(entries));
    }
  }

  public byte[] exportJson() throws IOException {
    synchronized (lock) {
      return encodeEntries(entries).getBytes(StandardCharsets.UTF_8);
    }
  }

  public void clear() throws IOException {
    synchronized (lock) {
      entries.clear();
      persist();
    }
  }

  private void loadExisting() throws IOException {
    final String json = new String(Files.readAllBytes(logFile), StandardCharsets.UTF_8).trim();
    if (json.isEmpty()) {
      entries = new ArrayList<>();
      return;
    }
    final Object parsed = JsonParser.parse(json);
    if (!(parsed instanceof List<?> list)) {
      throw new IOException("Audit log is corrupted (expected JSON array)");
    }
    final List<OfflineAuditEntry> restored = new ArrayList<>(list.size());
    for (final Object element : list) {
      if (!(element instanceof Map<?, ?> map)) {
        throw new IOException("Audit log contains non-object entries");
      }
      @SuppressWarnings("unchecked")
      final Map<String, Object> casted = (Map<String, Object>) map;
      restored.add(OfflineAuditEntry.fromJsonMap(casted));
    }
    entries = restored;
  }

  private void persist() throws IOException {
    final String json = encodeEntries(entries);
    Files.write(
        logFile,
        json.getBytes(StandardCharsets.UTF_8),
        StandardOpenOption.CREATE,
        StandardOpenOption.TRUNCATE_EXISTING);
  }

  private static String encodeEntries(final List<OfflineAuditEntry> entries) {
    final StringBuilder builder = new StringBuilder();
    builder.append('[');
    for (int i = 0; i < entries.size(); i++) {
      if (i > 0) {
        builder.append(',');
      }
      writeObject(builder, entries.get(i).toJson());
    }
    builder.append(']');
    return builder.toString();
  }

  @SuppressWarnings("unchecked")
  private static void writeObject(final StringBuilder builder, final Map<String, Object> map) {
    builder.append('{');
    boolean first = true;
    for (final Map.Entry<String, Object> entry : map.entrySet()) {
      if (!first) {
        builder.append(',');
      } else {
        first = false;
      }
      writeString(builder, entry.getKey());
      builder.append(':');
      writeValue(builder, entry.getValue());
    }
    builder.append('}');
  }

  private static void writeValue(final StringBuilder builder, final Object value) {
    if (value == null) {
      builder.append("null");
      return;
    }
    if (value instanceof String string) {
      writeString(builder, string);
      return;
    }
    if (value instanceof Number number) {
      builder.append(number.toString());
      return;
    }
    if (value instanceof Boolean bool) {
      builder.append(bool.booleanValue() ? "true" : "false");
      return;
    }
    if (value instanceof Map<?, ?> map) {
      @SuppressWarnings("unchecked")
      final Map<String, Object> casted = (Map<String, Object>) map;
      writeObject(builder, casted);
      return;
    }
    throw new IllegalArgumentException("Unsupported JSON value: " + value);
  }

  private static void writeString(final StringBuilder builder, final String value) {
    builder.append('"');
    for (int i = 0; i < value.length(); i++) {
      final char c = value.charAt(i);
      switch (c) {
        case '"' -> builder.append("\\\"");
        case '\\' -> builder.append("\\\\");
        case '\b' -> builder.append("\\b");
        case '\f' -> builder.append("\\f");
        case '\n' -> builder.append("\\n");
        case '\r' -> builder.append("\\r");
        case '\t' -> builder.append("\\t");
        default -> {
          if (c < 0x20) {
            builder.append(String.format("\\u%04x", (int) c));
          } else {
            builder.append(c);
          }
        }
      }
    }
    builder.append('"');
  }
}

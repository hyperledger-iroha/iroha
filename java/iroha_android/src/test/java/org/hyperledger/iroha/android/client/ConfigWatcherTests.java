package org.hyperledger.iroha.android.client;

import java.io.IOException;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.ArrayList;
import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.Objects;
import java.util.concurrent.CopyOnWriteArrayList;
import org.hyperledger.iroha.android.telemetry.TelemetryRecord;
import org.hyperledger.iroha.android.telemetry.TelemetrySink;

public final class ConfigWatcherTests {

  private final Path tempDir;
  private static final String CONFIG_RELOAD_SIGNAL = "android.telemetry.config.reload";

  private ConfigWatcherTests() throws IOException {
    this.tempDir = Files.createTempDirectory("config_watcher_tests");
  }

  public static void main(final String[] args) throws Exception {
    final ConfigWatcherTests tests = new ConfigWatcherTests();
    try {
      tests.reloadsManifestAndEmitsTelemetry();
      tests.retriesTransientErrorsWithBackoff();
      System.out.println("[IrohaAndroid] ConfigWatcherTests passed.");
    } finally {
      tests.cleanup();
    }
  }

  private void cleanup() throws IOException {
    deleteRecursively(tempDir);
  }

  private void reloadsManifestAndEmitsTelemetry() throws Exception {
    final Path manifest = tempDir.resolve("watch_manifest.json");
    writeManifest(manifest, "https://alpha.example");
    final RecordingTelemetrySink sink = new RecordingTelemetrySink();
    final TestListener listener = new TestListener();
    try (ConfigWatcher watcher =
        new ConfigWatcher(
            manifest,
            Duration.ZERO,
            (builder, context) -> builder.setTelemetrySink(sink),
            listener)) {
      assertEquals(1, listener.reloads().size(), "initial load should run once");
      Signal signal = sink.lastWithId(CONFIG_RELOAD_SIGNAL);
      assertEquals(
          "success",
          signal.fields().get("result"),
          "initial telemetry status mismatch");
      assertSignalMetadata(signal, manifest, "initial reload metadata mismatch");
      assertEquals(
          URI.create("https://alpha.example"),
          listener.lastConfig().clientConfig().baseUri(),
          "base URI mismatch after initial load");

      watcher.checkNow();
      assertEquals(1, listener.reloads().size(), "no change should skip reload");

      writeManifest(manifest, "https://beta.example");
      watcher.checkNow();
      assertEquals(2, listener.reloads().size(), "second load missing");
      signal = sink.lastWithId(CONFIG_RELOAD_SIGNAL);
      assertEquals(
          "success", signal.fields().get("result"), "reload telemetry missing");
      assertSignalMetadata(signal, manifest, "reload metadata mismatch");
      assertEquals(
          URI.create("https://beta.example"),
          listener.lastConfig().clientConfig().baseUri(),
          "base URI mismatch after reload");

      Files.writeString(manifest, "{ invalid json", StandardCharsets.UTF_8);
      watcher.checkNow();
      if (listener.errors().isEmpty()) {
        throw new AssertionError("reload error not reported");
      }
      signal = sink.lastWithId(CONFIG_RELOAD_SIGNAL);
      assertEquals("error", signal.fields().get("result"), "failure telemetry missing");
      assertEquals(
          "IllegalStateException",
          signal.fields().get("error"),
          "failure telemetry error class mismatch");
      assertEquals(
          manifestDigest(manifest),
          signal.fields().get("digest"),
          "failure telemetry digest mismatch");
      assertEquals(
          manifest.toString(),
          signal.fields().get("source"),
          "failure telemetry source mismatch");
      final long failureDuration = durationMs(signal);
      if (failureDuration != 0L) {
        throw new AssertionError(
            "failure duration must be zero (duration_ms=" + failureDuration + ")");
      }
    }
  }

  private void retriesTransientErrorsWithBackoff() throws Exception {
    final Path manifest = tempDir.resolve("retry_manifest.json");
    writeManifest(manifest, "https://gamma.example");
    final RecordingTelemetrySink sink = new RecordingTelemetrySink();
    final TestListener listener = new TestListener();
    try (ConfigWatcher watcher =
        new ConfigWatcher(
            manifest,
            Duration.ZERO,
            (builder, context) -> builder.setTelemetrySink(sink),
            listener)) {
      assertEquals(1, listener.reloads().size(), "initial load should run once");

      Files.writeString(manifest, "{ invalid json ", StandardCharsets.UTF_8);
      final Thread writer =
          new Thread(
              () -> {
                try {
                  Thread.sleep(75);
                  writeManifest(manifest, "https://delta.example");
                } catch (final Exception ex) {
                  throw new RuntimeException(ex);
                }
              });
      writer.start();
      watcher.checkNow();
      writer.join();

      assertEquals(2, listener.reloads().size(), "retry should eventually succeed");
      if (listener.errors().isEmpty()) {
        throw new AssertionError("transient failure should be reported via listener");
      }
      assertEquals(
          URI.create("https://delta.example"),
          listener.lastConfig().clientConfig().baseUri(),
          "base URI mismatch after successful retry");
      assertEquals(
          "success",
          sink.lastWithId(CONFIG_RELOAD_SIGNAL).fields().get("result"),
          "final telemetry signal should report success");
    }
  }

  private void writeManifest(final Path path, final String baseUri) throws IOException {
    Files.writeString(path, manifestJson(baseUri), StandardCharsets.UTF_8);
  }

  private static String manifestDigest(final Path manifest) throws IOException {
    return ClientConfigManifestLoader.sha256Hex(Files.readAllBytes(manifest));
  }

  private static void assertSignalMetadata(
      final Signal signal, final Path manifest, final String message) throws IOException {
    assertEquals(manifest.toString(), signal.fields().get("source"), message + " (source mismatch)");
    assertEquals(
        manifestDigest(manifest), signal.fields().get("digest"), message + " (digest mismatch)");
    final long duration = durationMs(signal);
    if (duration < 0) {
      throw new AssertionError(
          message + " (duration_ms must be non-negative, value=" + duration + ")");
    }
  }

  private static long durationMs(final Signal signal) {
    final Object value = signal.fields().get("duration_ms");
    if (!(value instanceof Number)) {
      throw new AssertionError("duration_ms field missing or not numeric: " + value);
    }
    return ((Number) value).longValue();
  }

  private static String manifestJson(final String baseUri) {
    final String salt =
        Base64.getEncoder().encodeToString("watcher-salt-2026".getBytes(StandardCharsets.UTF_8));
    return """
        {
          "torii": {
            "base_uri": "%s",
            "timeout_ms": 6000,
            "default_headers": {
              "User-Agent": "WatcherTests/1.0"
            }
          },
          "retry": {
            "max_attempts": 2,
            "base_delay_ms": 200
          },
          "telemetry": {
            "enabled": true,
            "exporter_name": "watcher-tests",
            "redaction": {
              "salt_b64": "%s",
              "salt_version": "2026-03-05T00:00Z",
              "rotation_id": "watcher-rotation"
            }
          }
        }
        """
        .formatted(baseUri, salt);
  }

  private static final class RecordingTelemetrySink implements TelemetrySink {
    private final List<Signal> signals = new CopyOnWriteArrayList<>();

    List<Signal> signals() {
      return signals;
    }

    Signal lastWithId(final String id) {
      for (int i = signals.size() - 1; i >= 0; i--) {
        final Signal signal = signals.get(i);
        if (signal.id().equals(id)) {
          return signal;
        }
      }
      throw new IllegalStateException("No signal recorded for id=" + id);
    }

    @Override
    public void onRequest(final TelemetryRecord record) {}

    @Override
    public void onResponse(final TelemetryRecord record, final ClientResponse response) {}

    @Override
    public void onFailure(final TelemetryRecord record, final Throwable error) {}

    @Override
    public void emitSignal(final String signalId, final Map<String, Object> fields) {
      signals.add(new Signal(signalId, new LinkedHashMap<>(fields)));
    }
  }

  private static final class Signal {
    private final String id;
    private final Map<String, Object> fields;

    Signal(final String id, final Map<String, Object> fields) {
      this.id = id;
      this.fields = Collections.unmodifiableMap(fields);
    }

    public String id() {
      return id;
    }

    public Map<String, Object> fields() {
      return fields;
    }
  }

  private static final class TestListener implements ConfigWatcher.Listener {
    private final List<ClientConfigManifestLoader.LoadedClientConfig> reloads =
        new ArrayList<>();
    private final List<Throwable> errors = new ArrayList<>();

    @Override
    public void onReload(final ClientConfigManifestLoader.LoadedClientConfig config) {
      reloads.add(config);
    }

    @Override
    public void onReloadError(final Throwable error) {
      errors.add(error);
    }

    List<ClientConfigManifestLoader.LoadedClientConfig> reloads() {
      return reloads;
    }

    ClientConfigManifestLoader.LoadedClientConfig lastConfig() {
      return reloads.get(reloads.size() - 1);
    }

    List<Throwable> errors() {
      return errors;
    }
  }

  private static void deleteRecursively(final Path path) throws IOException {
    if (!Files.exists(path)) {
      return;
    }
    if (Files.isDirectory(path)) {
      try (var stream = Files.list(path)) {
        for (final Path child : stream.toList()) {
          deleteRecursively(child);
        }
      }
    }
    Files.deleteIfExists(path);
  }

  private static void assertEquals(final Object expected, final Object actual, final String message) {
    if (!Objects.equals(expected, actual)) {
      throw new AssertionError(message + " (expected=" + expected + ", actual=" + actual + ")");
    }
  }

  private static void assertEquals(final long expected, final long actual, final String message) {
    if (expected != actual) {
      throw new AssertionError(message + " (expected=" + expected + ", actual=" + actual + ")");
    }
  }
}

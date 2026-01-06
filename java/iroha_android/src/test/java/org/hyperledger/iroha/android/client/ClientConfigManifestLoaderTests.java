package org.hyperledger.iroha.android.client;

import java.io.IOException;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.time.Duration;
import java.util.Base64;
import java.util.Objects;
import org.hyperledger.iroha.android.telemetry.TelemetryOptions;

public final class ClientConfigManifestLoaderTests {

  private final Path tempDir;

  private ClientConfigManifestLoaderTests() throws IOException {
    this.tempDir = Files.createTempDirectory("client_config_manifest_tests");
  }

  public static void main(final String[] args) throws Exception {
    final ClientConfigManifestLoaderTests tests = new ClientConfigManifestLoaderTests();
    try {
      tests.loadsToriiAndRetryConfiguration();
      tests.supportsOfflineJournalQueueWithSeed();
      tests.supportsFilePendingQueueWithRelativePath();
      tests.parsesTelemetryRedaction();
      tests.customizerCanMutateBuilder();
      tests.rejectsFractionalTimeoutMs();
      tests.rejectsFractionalRetryAttempts();
      System.out.println("[IrohaAndroid] ClientConfigManifestLoaderTests passed.");
    } finally {
      tests.cleanup();
    }
  }

  private void cleanup() throws IOException {
    deleteRecursively(tempDir);
  }

  private void loadsToriiAndRetryConfiguration() throws Exception {
    final Path manifest = tempDir.resolve("client_manifest.json");
    Files.writeString(manifest, baseManifestJson("https://torii.example", false));

    final ClientConfigManifestLoader.LoadedClientConfig loaded =
        ClientConfigManifestLoader.load(manifest);
    final ClientConfig config = loaded.clientConfig();

    assertEquals(URI.create("https://torii.example"), config.baseUri(), "base URI mismatch");
    assertEquals(Duration.ofMillis(7_000), config.requestTimeout(), "timeout mismatch");
    assertEquals("IrohaAndroidTests/1.0", config.defaultHeaders().get("User-Agent"), "header missing");
    assertTrue(config.retryPolicy().allowsRetry(2), "retry should allow attempt 2");
    assertFalse(config.retryPolicy().allowsRetry(3), "retry should stop after max attempts");
    assertEquals(
        Duration.ofMillis(1_000),
        config.retryPolicy().delayForAttempt(8),
        "retry delay should cap at max_delay_ms");
    final String expectedDigest =
        ClientConfigManifestLoader.sha256Hex(Files.readAllBytes(manifest));
    assertEquals(expectedDigest, loaded.context().digest(), "digest mismatch");
  }

  private void supportsOfflineJournalQueueWithSeed() throws Exception {
    final Path manifest = tempDir.resolve("offline_manifest.json");
    final Path queueDir = tempDir.resolve("queue");
    Files.createDirectories(queueDir);
    final String queuePath = queueDir.resolve("pending.queue").toString();
    final String seed =
        Base64.getEncoder().encodeToString("journal-seed-1234".getBytes(StandardCharsets.UTF_8));
    final String json =
        """
        {
          "torii": {
            "base_uri": "https://offline.example",
            "timeout_ms": 5000
          },
          "pending_queue": {
            "kind": "offline_journal",
            "path": "%s",
            "key_seed_b64": "%s"
          },
          "telemetry": { "enabled": false }
        }
        """
            .formatted(queuePath.replace("\\", "\\\\"), seed);
    Files.writeString(manifest, json, StandardCharsets.UTF_8);

    final ClientConfig config = ClientConfigManifestLoader.load(manifest).clientConfig();
    assertNotNull(config.pendingQueue(), "pending queue should be configured");
    assertTrue(
        config.pendingQueue().telemetryQueueName().toLowerCase().contains("offline"),
        "pending queue should be offline journal");
  }

  private void supportsFilePendingQueueWithRelativePath() throws Exception {
    final Path manifest = tempDir.resolve("file_queue_manifest.json");
    final String json =
        """
        {
          "torii": { "base_uri": "https://file-queue.example", "timeout_ms": 4000 },
          "pending_queue": {
            "kind": "file",
            "path": "queues/pending.queue"
          },
          "telemetry": { "enabled": false }
        }
        """;
    Files.writeString(manifest, json, StandardCharsets.UTF_8);

    final ClientConfig config = ClientConfigManifestLoader.load(manifest).clientConfig();
    assertNotNull(config.pendingQueue(), "pending queue should be configured");
    assertTrue(
        config.pendingQueue().telemetryQueueName().toLowerCase().contains("file"),
        "pending queue should be file-backed");
    final Path expectedPath =
        manifest.toAbsolutePath().getParent().resolve("queues").resolve("pending.queue");
    assertTrue(Files.exists(expectedPath), "queue file should be created relative to manifest");
  }

  private void parsesTelemetryRedaction() throws Exception {
    final Path manifest = tempDir.resolve("telemetry_manifest.json");
    Files.writeString(manifest, baseManifestJson("https://telemetry.example", true));

    final ClientConfig config = ClientConfigManifestLoader.load(manifest).clientConfig();
    final TelemetryOptions options = config.telemetryOptions();
    assertTrue(options.enabled(), "telemetry should be enabled");
    assertEquals("android-main", config.telemetryExporterName(), "exporter name mismatch");
    assertEquals("2026-03-05T00:00Z", options.redaction().saltVersion(), "salt version mismatch");
    assertEquals("telemetry-salt-q1", options.redaction().rotationId(), "rotation mismatch");
    final byte[] expectedSalt =
        Base64.getDecoder().decode("YW5kcm9pZC1zYWx0LTIwMjY=");
    assertByteArrayEquals(expectedSalt, options.redaction().salt(), "salt mismatch");
  }

  private void customizerCanMutateBuilder() throws Exception {
    final Path manifest = tempDir.resolve("customizer.json");
    Files.writeString(manifest, baseManifestJson("https://custom.example", false));

    final ClientConfigManifestLoader.LoadedClientConfig loaded =
        ClientConfigManifestLoader.load(
            manifest,
            (builder, context) -> builder.putDefaultHeader("X-Test-Header", context.digest()));
    final ClientConfig config = loaded.clientConfig();

    assertEquals(
        loaded.context().digest(),
        config.defaultHeaders().get("X-Test-Header"),
        "customizer header mismatch");
  }

  private void rejectsFractionalTimeoutMs() throws Exception {
    final Path manifest = tempDir.resolve("fractional_timeout.json");
    final String json =
        """
        {
          "torii": {
            "base_uri": "https://fractional.example",
            "timeout_ms": 12.5
          },
          "telemetry": { "enabled": false }
        }
        """;
    Files.writeString(manifest, json, StandardCharsets.UTF_8);

    try {
      ClientConfigManifestLoader.load(manifest);
      throw new AssertionError("expected fractional timeout to be rejected");
    } catch (final IllegalStateException ex) {
      assertTrue(
          ex.getMessage() == null || ex.getMessage().contains("Fractional"),
          "error should mention fractional value");
    }
  }

  private void rejectsFractionalRetryAttempts() throws Exception {
    final Path manifest = tempDir.resolve("fractional_retry.json");
    final String json =
        """
        {
          "torii": { "base_uri": "https://fractional.example" },
          "retry": {
            "max_attempts": 2.5
          },
          "telemetry": { "enabled": false }
        }
        """;
    Files.writeString(manifest, json, StandardCharsets.UTF_8);

    try {
      ClientConfigManifestLoader.load(manifest);
      throw new AssertionError("expected fractional retry attempt to be rejected");
    } catch (final IllegalStateException ex) {
      assertTrue(
          ex.getMessage() == null || ex.getMessage().contains("Fractional"),
          "error should mention fractional value");
    }
  }

  private static String baseManifestJson(final String baseUri, final boolean includeTelemetry) {
    final StringBuilder builder =
        new StringBuilder()
            .append("{\n")
            .append("  \"torii\": {\n")
            .append("    \"base_uri\": \"")
            .append(baseUri)
            .append("\",\n")
            .append("    \"timeout_ms\": 7000,\n")
            .append("    \"default_headers\": {\n")
            .append("      \"User-Agent\": \"IrohaAndroidTests/1.0\"\n")
            .append("    }\n")
            .append("  },\n")
            .append("  \"retry\": {\n")
            .append("    \"max_attempts\": 3,\n")
            .append("    \"base_delay_ms\": 250,\n")
            .append("    \"max_delay_ms\": 1000,\n")
            .append("    \"retry_status_codes\": [429]\n")
            .append("  },\n");
    if (includeTelemetry) {
      builder
          .append("  \"telemetry\": {\n")
          .append("    \"enabled\": true,\n")
          .append("    \"exporter_name\": \"android-main\",\n")
          .append("    \"redaction\": {\n")
          .append("      \"salt_b64\": \"YW5kcm9pZC1zYWx0LTIwMjY=\",\n")
          .append("      \"salt_version\": \"2026-03-05T00:00Z\",\n")
          .append("      \"rotation_id\": \"telemetry-salt-q1\"\n")
          .append("    }\n")
          .append("  }\n");
    } else {
      builder.append("  \"telemetry\": { \"enabled\": false }\n");
    }
    builder.append("}\n");
    return builder.toString();
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

  private static void assertNotNull(final Object value, final String message) {
    if (value == null) {
      throw new AssertionError(message);
    }
  }

  private static void assertTrue(final boolean condition, final String message) {
    if (!condition) {
      throw new AssertionError(message);
    }
  }

  private static void assertFalse(final boolean condition, final String message) {
    if (condition) {
      throw new AssertionError(message);
    }
  }

  private static void assertByteArrayEquals(
      final byte[] expected, final byte[] actual, final String message) {
    if (expected.length != actual.length) {
      throw new AssertionError(message);
    }
    for (int i = 0; i < expected.length; i++) {
      if (expected[i] != actual[i]) {
        throw new AssertionError(message);
      }
    }
  }
}

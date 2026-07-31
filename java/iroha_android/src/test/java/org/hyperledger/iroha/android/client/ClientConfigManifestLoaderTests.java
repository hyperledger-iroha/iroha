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
      tests.supportsFilePendingQueueWithRelativePath();
      tests.parsesTelemetryRedaction();
      tests.customizerCanMutateBuilder();
      tests.rejectsFractionalTimeoutMs();
      tests.rejectsFractionalRetryAttempts();
      tests.rejectsOutOfRangeRetryAttempts();
      tests.rejectsNonStringScalarFields();
      tests.rejectsMalformedPresentNumericAndBooleanFields();
      tests.rejectsOutOfDomainNumericValuesInsteadOfUsingDefaults();
      tests.preservesIntegerAndBooleanStringCompatibility();
      tests.rejectsGenesisPrivacyFingerprintsAsClientProofPolicyAtAnyDepth();
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
    assertEquals(
        "test-chain",
        config.localSigningContext().get().chainId(),
        "local signing chain mismatch");
    assertEquals(
        "test-chain",
        config.toBuilder().build().localSigningContext().get().chainId(),
        "toBuilder must preserve the local signing context");
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
          ex.getMessage() == null || ex.getMessage().contains("integer"),
          "error should explain the integer requirement");
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
          ex.getMessage() == null || ex.getMessage().contains("integer"),
          "error should explain the integer requirement");
    }
  }

  private void rejectsOutOfRangeRetryAttempts() throws Exception {
    final Path manifest = tempDir.resolve("out_of_range_retry.json");
    final String json =
        """
        {
          "torii": { "base_uri": "https://range.example" },
          "retry": {
            "max_attempts": 3000000000
          },
          "telemetry": { "enabled": false }
        }
        """;
    Files.writeString(manifest, json, StandardCharsets.UTF_8);

    try {
      ClientConfigManifestLoader.load(manifest);
      throw new AssertionError("expected out-of-range retry attempt to be rejected");
    } catch (final IllegalStateException ex) {
      assertTrue(
          ex.getMessage() == null || ex.getMessage().contains("out of range"),
          "error should mention out-of-range value");
    }
  }

  private void rejectsNonStringScalarFields() throws Exception {
    final String[] malformed = {
      manifest("{\"base_uri\":123}", "{\"enabled\":false}", ""),
      manifest(
          "{\"base_uri\":\"https://torii.example\",\"sorafs_gateway_uri\":false}",
          "{\"enabled\":false}",
          ""),
      manifest(
          "{\"base_uri\":\"https://torii.example\",\"sorafs_gateway_uri\":\"\"}",
          "{\"enabled\":false}",
          ""),
      manifest(
          "{\"base_uri\":\"https://torii.example\",\"default_headers\":{\"X-Test\":7}}",
          "{\"enabled\":false}",
          ""),
      manifest(
          "{\"base_uri\":\"https://torii.example\",\"default_headers\":{\"X-Test\":null}}",
          "{\"enabled\":false}",
          ""),
      manifest(
          "{\"base_uri\":\"https://torii.example\"}",
          "{\"enabled\":false}",
          ",\"pending_queue\":{\"kind\":true}"),
      manifest(
          "{\"base_uri\":\"https://torii.example\"}",
          "{\"enabled\":false,\"exporter_name\":[]}",
          "")
    };

    for (int i = 0; i < malformed.length; i++) {
      assertRejected("non_string_" + i + ".json", malformed[i]);
    }
  }

  private void rejectsMalformedPresentNumericAndBooleanFields() throws Exception {
    final String[] malformed = {
      manifest(
          "{\"base_uri\":\"https://torii.example\",\"timeout_ms\":true}",
          "{\"enabled\":false}",
          ""),
      manifest(
          "{\"base_uri\":\"https://torii.example\",\"timeout_ms\":\"\"}",
          "{\"enabled\":false}",
          ""),
      manifest(
          "{\"base_uri\":\"https://torii.example\"}",
          "{\"enabled\":false}",
          ",\"retry\":{\"max_attempts\":false}"),
      manifest(
          "{\"base_uri\":\"https://torii.example\"}",
          "{\"enabled\":false}",
          ",\"retry\":{\"base_delay_ms\":{}}"),
      manifest(
          "{\"base_uri\":\"https://torii.example\"}",
          "{\"enabled\":false}",
          ",\"retry\":{\"retry_on_network_error\":1}"),
      manifest(
          "{\"base_uri\":\"https://torii.example\"}",
          "{\"enabled\":false}",
          ",\"retry\":{\"retry_status_codes\":[null]}")
    };

    for (int i = 0; i < malformed.length; i++) {
      assertRejected("malformed_scalar_" + i + ".json", malformed[i]);
    }
  }

  private void rejectsOutOfDomainNumericValuesInsteadOfUsingDefaults() throws Exception {
    final String[] malformed = {
      manifest(
          "{\"base_uri\":\"https://torii.example\",\"timeout_ms\":-1}",
          "{\"enabled\":false}",
          ""),
      manifest(
          "{\"base_uri\":\"https://torii.example\"}",
          "{\"enabled\":false}",
          ",\"retry\":{\"max_attempts\":0}"),
      manifest(
          "{\"base_uri\":\"https://torii.example\"}",
          "{\"enabled\":false}",
          ",\"retry\":{\"base_delay_ms\":-1}"),
      manifest(
          "{\"base_uri\":\"https://torii.example\"}",
          "{\"enabled\":false}",
          ",\"retry\":{\"max_delay_ms\":-1}")
    };

    for (int i = 0; i < malformed.length; i++) {
      assertRejected("negative_scalar_" + i + ".json", malformed[i]);
    }
  }

  private void preservesIntegerAndBooleanStringCompatibility() throws Exception {
    final String json =
        manifest(
            "{\"base_uri\":\"https://torii.example\",\"timeout_ms\":\"7000\"}",
            "{\"enabled\":false}",
            ",\"retry\":{\"max_attempts\":\"3\",\"base_delay_ms\":\"250\","
                + "\"retry_on_network_error\":\"no\"}");
    final Path path = tempDir.resolve("string_compatibility.json");
    Files.writeString(path, json, StandardCharsets.UTF_8);

    final ClientConfig config = ClientConfigManifestLoader.load(path).clientConfig();

    assertEquals(Duration.ofMillis(7_000), config.requestTimeout(), "timeout mismatch");
    assertTrue(config.retryPolicy().allowsRetry(2), "retry should allow attempt 2");
    assertFalse(config.retryPolicy().allowsRetry(3), "retry should stop after max attempts");
    assertFalse(config.retryPolicy().shouldRetryError(1), "network retry should be disabled");
  }

  private void rejectsGenesisPrivacyFingerprintsAsClientProofPolicyAtAnyDepth()
      throws Exception {
    final String[] nonAuthoritative = {
      manifest(
          "{\"base_uri\":\"https://torii.example\"}",
          "{\"enabled\":false}",
          ",\"confidential_features\":{\"zk_policy_hash\":\"00\"}"),
      manifest(
          "{\"base_uri\":\"https://torii.example\"}",
          "{\"enabled\":false}",
          ",\"genesis\":{\"zk_policy_hash\":\"00\"}"),
      manifest(
          "{\"base_uri\":\"https://torii.example\"}",
          "{\"enabled\":false}",
          ",\"genesis\":{\"confidentialFeatures\":{}}"),
      manifest(
          "{\"base_uri\":\"https://torii.example\"}",
          "{\"enabled\":false}",
          ",\"client\":{\"privacy\":{\"zkPolicyHash\":\"00\"}}")
    };

    for (int i = 0; i < nonAuthoritative.length; i++) {
      final Path manifest = tempDir.resolve("non_authoritative_privacy_policy_" + i + ".json");
      Files.writeString(manifest, nonAuthoritative[i], StandardCharsets.UTF_8);
      try {
        ClientConfigManifestLoader.load(manifest);
        throw new AssertionError("expected genesis privacy fingerprint to be rejected");
      } catch (final IllegalStateException expected) {
        assertTrue(
            expected.getMessage() != null
                && expected.getMessage().contains("/v1/privacy/capabilities"),
            "error should direct callers to committed privacy capabilities");
      }
    }
  }

  private void assertRejected(final String fileName, final String json) throws Exception {
    final Path manifest = tempDir.resolve(fileName);
    Files.writeString(manifest, json, StandardCharsets.UTF_8);
    try {
      ClientConfigManifestLoader.load(manifest);
      throw new AssertionError("expected malformed manifest to be rejected: " + fileName);
    } catch (final IllegalStateException expected) {
      // Expected.
    }
  }

  private static String manifest(
      final String torii, final String telemetry, final String extra) {
    return "{\"torii\":" + torii + ",\"telemetry\":" + telemetry + extra + "}";
  }

  private static String baseManifestJson(final String baseUri, final boolean includeTelemetry) {
    final StringBuilder builder =
        new StringBuilder()
            .append("{\n")
            .append("  \"chain_id\": \"test-chain\",\n")
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

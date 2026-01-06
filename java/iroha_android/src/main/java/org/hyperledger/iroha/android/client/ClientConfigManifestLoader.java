package org.hyperledger.iroha.android.client;

import java.io.IOException;
import java.net.URI;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.security.MessageDigest;
import java.security.NoSuchAlgorithmException;
import java.time.Duration;
import java.time.Instant;
import java.util.ArrayList;
import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import java.util.Optional;
import java.util.function.BiConsumer;
import org.hyperledger.iroha.android.offline.OfflineJournalKey;
import org.hyperledger.iroha.android.telemetry.TelemetryOptions;

/**
 * Loads {@link ClientConfig} instances from JSON manifests derived from {@code iroha_config}.
 *
 * <p>The loader keeps the manifest structure available via {@link ManifestContext} so callers can
 * inspect the inputs alongside the built {@link ClientConfig}.
 */
public final class ClientConfigManifestLoader {

  private ClientConfigManifestLoader() {}

  /** Loads the manifest at {@code manifestPath} using the default configuration. */
  public static LoadedClientConfig load(final Path manifestPath) throws IOException {
    return load(manifestPath, null);
  }

  /**
    * Loads the manifest at {@code manifestPath} and allows callers to customise the builder before
    * it is materialised.
    *
    * @param customizer invoked after the manifest is applied to the builder
    */
  public static LoadedClientConfig load(
      final Path manifestPath, final Customizer customizer) throws IOException {
    Objects.requireNonNull(manifestPath, "manifestPath");
    final byte[] payload =
        Files.readAllBytes(manifestPath);
    final String digest = sha256Hex(payload);
    return parse(manifestPath, payload, digest, customizer);
  }

  static LoadedClientConfig parse(
      final Path manifestPath,
      final byte[] payload,
      final String digest,
      final Customizer customizer) {
    final Map<String, Object> root = parseRoot(payload);
    final ClientConfig.Builder builder = ClientConfig.builder();
    applyTorii(manifestPath, builder, root);
    applyRetry(builder, root);
    applyPendingQueue(manifestPath, builder, root);
    applyTelemetry(builder, root);
    final ManifestContext context =
        new ManifestContext(manifestPath, digest, immutableCopy(root));
    if (customizer != null) {
      customizer.accept(builder, context);
    }
    final ClientConfig config = builder.build();
    return new LoadedClientConfig(config, context, Instant.now());
  }

  private static Map<String, Object> parseRoot(final byte[] payload) {
    final String json = new String(payload, StandardCharsets.UTF_8).trim();
    if (json.isEmpty()) {
      throw new IllegalStateException("Client config manifest is empty");
    }
    final Object root = JsonParser.parse(json);
    return expectObject(root, "manifest");
  }

  private static void applyTorii(
      final Path manifestPath,
      final ClientConfig.Builder builder,
      final Map<String, Object> root) {
    final Map<String, Object> torii = expectObject(root.get("torii"), "torii");
    final String baseUri = requireString(torii, "base_uri");
    builder.setBaseUri(parseUri(baseUri, "torii.base_uri"));
    final String gateway = optionalString(torii.get("sorafs_gateway_uri"));
    if (gateway != null && !gateway.isEmpty()) {
      builder.setSorafsGatewayUri(parseUri(gateway, "torii.sorafs_gateway_uri"));
    }
    final Long timeoutMs = optionalLong(torii.get("timeout_ms"));
    if (timeoutMs != null && timeoutMs >= 0) {
      builder.setRequestTimeout(Duration.ofMillis(timeoutMs));
    }
    final Map<String, Object> headers =
        optionalObject(torii.get("default_headers"), "torii.default_headers");
    if (headers != null) {
      for (final Map.Entry<String, Object> entry : headers.entrySet()) {
        final String name = Objects.requireNonNull(entry.getKey(), "header name");
        final String value = optionalString(entry.getValue());
        if (value != null) {
          builder.putDefaultHeader(name, value);
        }
      }
    }
  }

  private static void applyRetry(final ClientConfig.Builder builder, final Map<String, Object> root) {
    final Map<String, Object> retry = optionalObject(root.get("retry"), "retry");
    if (retry == null) {
      return;
    }
    final RetryPolicy.Builder policy = RetryPolicy.builder();
    final Integer attempts = optionalInt(retry.get("max_attempts"));
    if (attempts != null && attempts >= 1) {
      policy.setMaxAttempts(attempts);
    }
    final Long baseDelayMs = optionalLong(retry.get("base_delay_ms"));
    if (baseDelayMs != null && baseDelayMs >= 0) {
      policy.setBaseDelay(Duration.ofMillis(baseDelayMs));
    }
    final Long maxDelayMs = optionalLong(retry.get("max_delay_ms"));
    if (maxDelayMs != null && maxDelayMs >= 0) {
      policy.setMaxDelay(Duration.ofMillis(maxDelayMs));
    }
    final Boolean retryServer = optionalBoolean(retry.get("retry_on_server_error"));
    if (retryServer != null) {
      policy.setRetryOnServerError(retryServer);
    }
    final Boolean retryTooMany = optionalBoolean(retry.get("retry_on_too_many_requests"));
    if (retryTooMany != null) {
      policy.setRetryOnTooManyRequests(retryTooMany);
    }
    final Boolean retryNetwork = optionalBoolean(retry.get("retry_on_network_error"));
    if (retryNetwork != null) {
      policy.setRetryOnNetworkError(retryNetwork);
    }
    final List<Object> codes = optionalArray(retry.get("retry_status_codes"), "retry.retry_status_codes");
    if (codes != null) {
      for (final Object value : codes) {
        final Integer status = optionalInt(value);
        if (status != null) {
          policy.addRetryStatusCode(status);
        }
      }
    }
    builder.setRetryPolicy(policy.build());
  }

  private static void applyPendingQueue(
      final Path manifestPath,
      final ClientConfig.Builder builder,
      final Map<String, Object> root) {
    final Map<String, Object> pending =
        optionalObject(root.get("pending_queue"), "pending_queue");
    if (pending == null) {
      return;
    }
    final String kind = optionalString(pending.get("kind"));
    if (kind == null || "memory".equalsIgnoreCase(kind)) {
      builder.setPendingQueue(null);
      return;
    }
    if ("offline_journal".equalsIgnoreCase(kind)) {
      final String pathRaw = requireString(pending, "path");
      final Path resolved =
          resolveRelative(manifestPath, pathRaw);
      final Optional<OfflineJournalKey> key = deriveJournalKey(pending);
      if (key.isEmpty()) {
        throw new IllegalStateException(
            "pending_queue.kind=offline_journal requires either key_seed_b64 or passphrase");
      }
      builder.enableOfflineJournalQueue(resolved, key.get());
      return;
    }
    if ("file".equalsIgnoreCase(kind)) {
      final String pathRaw = requireString(pending, "path");
      final Path resolved = resolveRelative(manifestPath, pathRaw);
      builder.enableFilePendingQueue(resolved);
      return;
    }
    throw new IllegalStateException("Unsupported pending queue kind: " + kind);
  }

  private static Optional<OfflineJournalKey> deriveJournalKey(final Map<String, Object> pending) {
    final String seedB64 = optionalString(pending.get("key_seed_b64"));
    if (seedB64 != null && !seedB64.isEmpty()) {
      final byte[] seed = Base64.getDecoder().decode(seedB64);
      if (seed.length == 0) {
        throw new IllegalStateException("pending_queue.key_seed_b64 cannot decode to an empty seed");
      }
      return Optional.of(OfflineJournalKey.derive(seed));
    }
    final String seedHex = optionalString(pending.get("key_seed_hex"));
    if (seedHex != null && !seedHex.trim().isEmpty()) {
      final byte[] seed = hexToBytes(seedHex.trim());
      return Optional.of(OfflineJournalKey.derive(seed));
    }
    final String passphrase = optionalString(pending.get("passphrase"));
    if (passphrase != null && !passphrase.isEmpty()) {
      return Optional.of(OfflineJournalKey.deriveFromPassphrase(passphrase.toCharArray()));
    }
    return Optional.empty();
  }

  private static void applyTelemetry(
      final ClientConfig.Builder builder, final Map<String, Object> root) {
    final Map<String, Object> telemetry =
        optionalObject(root.get("telemetry"), "telemetry");
    if (telemetry == null) {
      builder.setTelemetryOptions(TelemetryOptions.disabled());
      return;
    }
    final Boolean enabledFlag = optionalBoolean(telemetry.get("enabled"));
    final boolean enabled = enabledFlag == null || enabledFlag;
    final TelemetryOptions.Builder telemetryBuilder = TelemetryOptions.builder().setEnabled(enabled);
    if (enabled) {
      final Map<String, Object> redaction =
          optionalObject(telemetry.get("redaction"), "telemetry.redaction");
      if (redaction == null) {
        throw new IllegalStateException(
            "telemetry.redaction block required when telemetry.enabled is true");
      }
      telemetryBuilder.setTelemetryRedaction(parseRedaction(redaction));
    } else {
      telemetryBuilder.setTelemetryRedaction(TelemetryOptions.Redaction.disabled());
    }
    builder.setTelemetryOptions(telemetryBuilder.build());
    final String exporterName = optionalString(telemetry.get("exporter_name"));
    if (exporterName != null) {
      builder.setTelemetryExporterName(exporterName);
    }
  }

  private static TelemetryOptions.Redaction parseRedaction(final Map<String, Object> redaction) {
    final TelemetryOptions.Redaction.Builder builder = TelemetryOptions.Redaction.builder();
    final String saltB64 = optionalString(redaction.get("salt_b64"));
    final String saltHex = optionalString(redaction.get("salt_hex"));
    if (saltB64 != null && !saltB64.isEmpty()) {
      builder.setSalt(Base64.getDecoder().decode(saltB64));
    } else if (saltHex != null && !saltHex.trim().isEmpty()) {
      builder.setSaltHex(saltHex.trim());
    }
    final String saltVersion = requireString(redaction, "salt_version");
    builder.setSaltVersion(saltVersion);
    final String rotation = optionalString(redaction.get("rotation_id"));
    if (rotation != null && !rotation.isEmpty()) {
      builder.setRotationId(rotation);
    } else {
      builder.setRotationId(saltVersion);
    }
    final String algorithm = optionalString(redaction.get("algorithm"));
    if (algorithm != null && !algorithm.isEmpty()) {
      builder.setAlgorithm(algorithm);
    }
    return builder.build();
  }

  private static Map<String, Object> immutableCopy(final Map<String, Object> input) {
    final Map<String, Object> copy = new LinkedHashMap<>();
    for (final Map.Entry<String, Object> entry : input.entrySet()) {
      copy.put(entry.getKey(), immutableValue(entry.getValue()));
    }
    return Collections.unmodifiableMap(copy);
  }

  private static Object immutableValue(final Object value) {
    if (value instanceof Map<?, ?> map) {
      final Map<String, Object> copy = new LinkedHashMap<>();
      for (final Map.Entry<?, ?> entry : map.entrySet()) {
        final Object key = entry.getKey();
        if (!(key instanceof String)) {
          continue;
        }
        copy.put((String) key, immutableValue(entry.getValue()));
      }
      return Collections.unmodifiableMap(copy);
    }
    if (value instanceof List<?> list) {
      final List<Object> copy = new ArrayList<>(list.size());
      for (final Object element : list) {
        copy.add(immutableValue(element));
      }
      return Collections.unmodifiableList(copy);
    }
    return value;
  }

  private static Map<String, Object> expectObject(final Object value, final String path) {
    if (value == null) {
      throw new IllegalStateException(path + " block is required");
    }
    if (!(value instanceof Map<?, ?> map)) {
      throw new IllegalStateException(path + " is not a JSON object");
    }
    final Map<String, Object> copy = new LinkedHashMap<>();
    for (final Map.Entry<?, ?> entry : map.entrySet()) {
      final Object key = entry.getKey();
      if (!(key instanceof String)) {
        continue;
      }
      copy.put((String) key, entry.getValue());
    }
    return copy;
  }

  private static Map<String, Object> optionalObject(final Object value, final String path) {
    if (value == null) {
      return null;
    }
    if (!(value instanceof Map<?, ?> map)) {
      throw new IllegalStateException(path + " is not a JSON object");
    }
    final Map<String, Object> copy = new LinkedHashMap<>();
    for (final Map.Entry<?, ?> entry : map.entrySet()) {
      final Object key = entry.getKey();
      if (key instanceof String str) {
        copy.put(str, entry.getValue());
      }
    }
    return copy;
  }

  private static List<Object> optionalArray(final Object value, final String path) {
    if (value == null) {
      return null;
    }
    if (!(value instanceof List<?> list)) {
      throw new IllegalStateException(path + " is not an array");
    }
    return new ArrayList<>(list);
  }

  private static String requireString(final Map<String, Object> object, final String key) {
    final Object value = object.get(key);
    if (value == null) {
      throw new IllegalStateException("Missing required field: " + key);
    }
    final String normalized = optionalString(value);
    if (normalized == null || normalized.isEmpty()) {
      throw new IllegalStateException("Field " + key + " must be a non-empty string");
    }
    return normalized;
  }

  private static String optionalString(final Object value) {
    if (value == null) {
      return null;
    }
    if (value instanceof String string) {
      return string;
    }
    return String.valueOf(value);
  }

  private static Integer optionalInt(final Object value) {
    if (value == null) {
      return null;
    }
    if (value instanceof Number number) {
      if (number instanceof Float || number instanceof Double) {
        throw new IllegalStateException("Fractional numbers are not supported: " + value);
      }
      return number.intValue();
    }
    final String normalized = optionalString(value);
    if (normalized == null) {
      return null;
    }
    return Integer.parseInt(normalized);
  }

  private static Long optionalLong(final Object value) {
    if (value == null) {
      return null;
    }
    if (value instanceof Number number) {
      if (number instanceof Float || number instanceof Double) {
        throw new IllegalStateException("Fractional numbers are not supported: " + value);
      }
      return number.longValue();
    }
    final String normalized = optionalString(value);
    if (normalized == null || normalized.isEmpty()) {
      return null;
    }
    return Long.parseLong(normalized);
  }

  private static Boolean optionalBoolean(final Object value) {
    if (value == null) {
      return null;
    }
    if (value instanceof Boolean bool) {
      return bool;
    }
    final String normalized = optionalString(value);
    if (normalized == null) {
      return null;
    }
    final String lower = normalized.trim().toLowerCase(Locale.ROOT);
    if ("true".equals(lower) || "1".equals(lower) || "yes".equals(lower)) {
      return true;
    }
    if ("false".equals(lower) || "0".equals(lower) || "no".equals(lower)) {
      return false;
    }
    throw new IllegalStateException("Unsupported boolean value: " + value);
  }

  private static URI parseUri(final String value, final String field) {
    try {
      return URI.create(value);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalStateException("Invalid URI for " + field + ": " + value, ex);
    }
  }

  private static Path resolveRelative(final Path manifestPath, final String target) {
    final Path baseDir = manifestPath.toAbsolutePath().getParent();
    if (baseDir == null) {
      return Path.of(target).toAbsolutePath();
    }
    return baseDir.resolve(target).normalize();
  }

  private static byte[] hexToBytes(final String hex) {
    final String normalized = hex.trim();
    if ((normalized.length() & 1) == 1) {
      throw new IllegalStateException("Hex value must have an even number of characters");
    }
    final byte[] result = new byte[normalized.length() / 2];
    for (int i = 0; i < normalized.length(); i += 2) {
      final int hi = Character.digit(normalized.charAt(i), 16);
      final int lo = Character.digit(normalized.charAt(i + 1), 16);
      if (hi < 0 || lo < 0) {
        throw new IllegalStateException("Hex value contains invalid characters");
      }
      result[i / 2] = (byte) ((hi << 4) | lo);
    }
    return result;
  }

  static String sha256Hex(final byte[] payload) {
    final MessageDigest digest;
    try {
      digest = MessageDigest.getInstance("SHA-256");
    } catch (final NoSuchAlgorithmException ex) {
      throw new IllegalStateException("SHA-256 digest not available", ex);
    }
    final byte[] hash = digest.digest(payload);
    final StringBuilder builder = new StringBuilder(hash.length * 2);
    for (final byte value : hash) {
      builder.append(String.format(Locale.ROOT, "%02x", value));
    }
    return builder.toString();
  }

  /** Allows callers to customise the builder after the manifest has been applied. */
  @FunctionalInterface
  public interface Customizer extends BiConsumer<ClientConfig.Builder, ManifestContext> {
    @Override
    void accept(ClientConfig.Builder builder, ManifestContext context);
  }

  /** Immutable manifest metadata passed to customisers and returned in {@link LoadedClientConfig}. */
  public static final class ManifestContext {
    private final Path manifestPath;
    private final String digest;
    private final Map<String, Object> manifest;

    ManifestContext(
        final Path manifestPath, final String digest, final Map<String, Object> manifest) {
      this.manifestPath = manifestPath;
      this.digest = digest;
      this.manifest = manifest;
    }

    public Path manifestPath() {
      return manifestPath;
    }

    public String digest() {
      return digest;
    }

    public Map<String, Object> manifest() {
      return manifest;
    }
  }

  /** Wraps the built {@link ClientConfig} together with provenance metadata. */
  public static final class LoadedClientConfig {
    private final ClientConfig clientConfig;
    private final ManifestContext context;
    private final Instant loadedAt;

    LoadedClientConfig(
        final ClientConfig clientConfig,
        final ManifestContext context,
        final Instant loadedAt) {
      this.clientConfig = Objects.requireNonNull(clientConfig, "clientConfig");
      this.context = Objects.requireNonNull(context, "context");
      this.loadedAt = Objects.requireNonNull(loadedAt, "loadedAt");
    }

    public ClientConfig clientConfig() {
      return clientConfig;
    }

    public ManifestContext context() {
      return context;
    }

    public Instant loadedAt() {
      return loadedAt;
    }
  }
}

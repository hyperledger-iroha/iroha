package org.hyperledger.iroha.android.offline;

import java.io.IOException;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;
import org.hyperledger.iroha.android.util.HashLiteral;

/** Immutable representation of an `AndroidProvisionedProof` payload. */
public final class AndroidProvisionedProof {

  private final String manifestSchema;
  private final Integer manifestVersion;
  private final long manifestIssuedAtMs;
  private final String challengeHashLiteral;
  private final long counter;
  private final Map<String, Object> deviceManifest;
  private final String inspectorSignatureHex;

  private AndroidProvisionedProof(
      final String manifestSchema,
      final Integer manifestVersion,
      final long manifestIssuedAtMs,
      final String challengeHashLiteral,
      final long counter,
      final Map<String, Object> deviceManifest,
      final String inspectorSignatureHex) {
    this.manifestSchema = manifestSchema;
    this.manifestVersion = manifestVersion;
    this.manifestIssuedAtMs = manifestIssuedAtMs;
    this.challengeHashLiteral = challengeHashLiteral;
    this.counter = counter;
    this.deviceManifest = deviceManifest;
    this.inspectorSignatureHex = inspectorSignatureHex;
  }

  public static AndroidProvisionedProof fromJson(final byte[] payload) {
    final Object root = parse(payload);
    return fromObject(root);
  }

  public static AndroidProvisionedProof fromJson(final String json) {
    final Object root = JsonParser.parse(json.trim());
    return fromObject(root);
  }

  public static AndroidProvisionedProof fromPath(final Path path) throws IOException {
    return fromJson(Files.readAllBytes(path));
  }

  private static AndroidProvisionedProof fromObject(final Object root) {
    final Map<String, Object> object = expectObject(root, "root");
    final String schema = requireString(object.get("manifest_schema"), "manifest_schema");
    final Integer manifestVersion = asOptionalInt(object.get("manifest_version"));
    final long issuedAt = requireLong(object.get("manifest_issued_at_ms"), "manifest_issued_at_ms");
    final String hashLiteral = requireString(object.get("challenge_hash"), "challenge_hash");
    final long counter = requireLong(object.get("counter"), "counter");
    final Map<String, Object> manifest = expectObject(object.get("device_manifest"), "device_manifest");
    final String signature = requireString(object.get("inspector_signature"), "inspector_signature");
    final String canonicalSchema = schema.trim();
    if (canonicalSchema.isEmpty()) {
      throw new IllegalStateException("manifest_schema must not be empty");
    }
    final Map<String, Object> manifestCopy = Collections.unmodifiableMap(copyManifest(manifest));
    final String deviceId = asString(manifestCopy.get("android.provisioned.device_id"));
    if (deviceId == null || deviceId.isBlank()) {
      throw new IllegalStateException("device_manifest must include android.provisioned.device_id");
    }
    final String canonicalHash = HashLiteral.canonicalize(hashLiteral);
    final String canonicalSignature = normalizeSignature(signature);
    return new AndroidProvisionedProof(
        canonicalSchema,
        manifestVersion,
        issuedAt,
        canonicalHash,
        counter,
        manifestCopy,
        canonicalSignature);
  }

  public String manifestSchema() {
    return manifestSchema;
  }

  public Integer manifestVersion() {
    return manifestVersion;
  }

  public long manifestIssuedAtMs() {
    return manifestIssuedAtMs;
  }

  public String challengeHashLiteral() {
    return challengeHashLiteral;
  }

  public long counter() {
    return counter;
  }

  public Map<String, Object> deviceManifest() {
    return deviceManifest;
  }

  public String inspectorSignatureHex() {
    return inspectorSignatureHex;
  }

  public String deviceId() {
    return asString(deviceManifest.get("android.provisioned.device_id"));
  }

  public byte[] inspectorSignature() {
    return hexToBytes(inspectorSignatureHex);
  }

  public byte[] challengeHashBytes() {
    return HashLiteral.decode(challengeHashLiteral);
  }

  /** Returns a canonical JSON representation (keys sorted lexicographically). */
  public String toCanonicalJson() {
    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("manifest_schema", manifestSchema);
    if (manifestVersion != null) {
      payload.put("manifest_version", manifestVersion);
    }
    payload.put("manifest_issued_at_ms", manifestIssuedAtMs);
    payload.put("challenge_hash", challengeHashLiteral);
    payload.put("counter", counter);
    payload.put("device_manifest", deviceManifest);
    payload.put("inspector_signature", inspectorSignatureHex);
    return JsonEncoder.encode(payload);
  }

  private static Object parse(final byte[] payload) {
    final String json = new String(payload, StandardCharsets.UTF_8).trim();
    if (json.isEmpty()) {
      throw new IllegalStateException("Empty JSON payload");
    }
    return JsonParser.parse(json);
  }

  @SuppressWarnings("unchecked")
  private static Map<String, Object> expectObject(final Object value, final String path) {
    if (!(value instanceof Map)) {
      throw new IllegalStateException(path + " is not a JSON object");
    }
    return (Map<String, Object>) value;
  }

  private static String requireString(final Object value, final String path) {
    final String normalized = asString(value);
    if (normalized == null || normalized.isBlank()) {
      throw new IllegalStateException(path + " is required");
    }
    return normalized;
  }

  private static String asString(final Object value) {
    if (value == null) {
      return null;
    }
    if (value instanceof String string) {
      return string;
    }
    return String.valueOf(value);
  }

  private static long requireLong(final Object value, final String path) {
    if (!(value instanceof Number number)) {
      throw new IllegalStateException(path + " is not a number");
    }
    if (number instanceof Float || number instanceof Double) {
      throw new IllegalStateException(path + " must be an integer");
    }
    return number.longValue();
  }

  private static Integer asOptionalInt(final Object value) {
    if (value == null) {
      return null;
    }
    if (value instanceof Number number) {
      if (number instanceof Float || number instanceof Double) {
        return null;
      }
      return number.intValue();
    }
    final String normalized = asString(value);
    if (normalized == null || normalized.isBlank()) {
      return null;
    }
    return Integer.parseInt(normalized, 10);
  }

  private static Map<String, Object> copyManifest(final Map<String, Object> manifest) {
    final Map<String, Object> copy = new LinkedHashMap<>();
    manifest.forEach((key, value) -> copy.put(Objects.requireNonNull(key, "manifest key"), copyValue(value)));
    return copy;
  }

  @SuppressWarnings("unchecked")
  private static Object copyValue(final Object value) {
    if (value instanceof Map<?, ?> map) {
      final Map<String, Object> nested = new LinkedHashMap<>();
      map.forEach((key, entry) -> nested.put(String.valueOf(key), copyValue(entry)));
      return Collections.unmodifiableMap(nested);
    }
    if (value instanceof Iterable<?> iterable) {
      final java.util.List<Object> list = new java.util.ArrayList<>();
      for (Object entry : iterable) {
        list.add(copyValue(entry));
      }
      return Collections.unmodifiableList(list);
    }
    if (value == null || value instanceof String || value instanceof Number || value instanceof Boolean) {
      return value;
    }
    return String.valueOf(value);
  }

  private static String normalizeSignature(final String value) {
    final String trimmed = Objects.requireNonNull(value, "inspector_signature").trim();
    if (!trimmed.matches("^[0-9A-Fa-f]{128}$")) {
      throw new IllegalStateException("inspector_signature must be 64-byte hex");
    }
    return trimmed.toUpperCase(Locale.ROOT);
  }

  private static byte[] hexToBytes(final String hex) {
    final byte[] bytes = new byte[hex.length() / 2];
    for (int i = 0; i < bytes.length; i++) {
      final int offset = i * 2;
      bytes[i] = (byte) Integer.parseInt(hex.substring(offset, offset + 2), 16);
    }
    return bytes;
  }
}

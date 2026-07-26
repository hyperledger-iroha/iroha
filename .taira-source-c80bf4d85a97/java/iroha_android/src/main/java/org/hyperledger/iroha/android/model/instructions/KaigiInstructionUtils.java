package org.hyperledger.iroha.android.model.instructions;

import java.util.ArrayList;
import java.util.Base64;
import java.util.Collections;
import java.util.LinkedHashMap;
import java.util.List;
import java.util.Locale;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.util.HashLiteral;

/** Shared helpers for flattening Kaigi instruction payloads to argument maps. */
final class KaigiInstructionUtils {

  private static final String DOMAIN_KEY = "domain_id";
  private static final String CALL_NAME_KEY = "call_name";

  private KaigiInstructionUtils() {}

  static CallId parseCallId(final Map<String, String> arguments, final String prefix) {
    final String domain = require(arguments, prefixKey(prefix, DOMAIN_KEY));
    final String callName = require(arguments, prefixKey(prefix, CALL_NAME_KEY));
    return new CallId(domain, callName);
  }

  static void appendCallId(final CallId callId, final Map<String, String> target, final String prefix) {
    target.put(prefixKey(prefix, DOMAIN_KEY), callId.domainId());
    target.put(prefixKey(prefix, CALL_NAME_KEY), callId.callName());
  }

  static Map<String, String> extractMetadata(final Map<String, String> arguments, final String prefix) {
    final Map<String, String> metadata = new LinkedHashMap<>();
    final String effectivePrefix = prefix.endsWith(".") ? prefix : prefix + ".";
    arguments.forEach(
        (key, value) -> {
          if (key.startsWith(effectivePrefix)) {
            final String metadataKey = key.substring(effectivePrefix.length());
            if (!metadataKey.isEmpty()) {
              metadata.put(metadataKey, value);
            }
          }
        });
    return metadata;
  }

  static void appendMetadata(
      final Map<String, String> metadata, final Map<String, String> target, final String prefix) {
    if (metadata.isEmpty()) {
      return;
    }
    final String effectivePrefix = prefix.endsWith(".") ? prefix : prefix + ".";
    metadata.forEach((key, value) -> target.put(effectivePrefix + key, value));
  }

  static String canonicalizeHash(final byte[] bytes) {
    return HashLiteral.canonicalize(bytes);
  }

  static String canonicalizeHash(final String value) {
    return HashLiteral.canonicalize(value);
  }

  static String canonicalizeOptionalHash(final String value) {
    if (value == null || value.trim().isEmpty()) {
      return null;
    }
    return HashLiteral.canonicalizeOptional(value);
  }

  static String canonicalizeOptionalHash(final byte[] value) {
    if (value == null) {
      return null;
    }
    return canonicalizeHash(value);
  }

  static long parseUnsignedLong(final String value, final String fieldName) {
    Objects.requireNonNull(value, fieldName);
    try {
      final long parsed = Long.parseUnsignedLong(value);
      if (parsed < 0) {
        throw new IllegalArgumentException(fieldName + " must be unsigned");
      }
      return parsed;
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException(fieldName + " must be an unsigned integer", ex);
    }
  }

  static Long parseOptionalUnsignedLong(final String value, final String fieldName) {
    if (value == null || value.isBlank()) {
      return null;
    }
    return parseUnsignedLong(value, fieldName);
  }

  static int parsePositiveInt(final String value, final String fieldName) {
    Objects.requireNonNull(value, fieldName);
    try {
      final int parsed = Integer.parseUnsignedInt(value);
      if (parsed <= 0) {
        throw new IllegalArgumentException(fieldName + " must be greater than zero");
      }
      return parsed;
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException(fieldName + " must be a positive integer", ex);
    }
  }

  static Integer parseOptionalPositiveInt(final String value, final String fieldName) {
    if (value == null || value.isBlank()) {
      return null;
    }
    return parsePositiveInt(value, fieldName);
  }

  static int parseNonNegativeInt(final String value, final String fieldName) {
    Objects.requireNonNull(value, fieldName);
    try {
      final int parsed = Integer.parseUnsignedInt(value);
      if (parsed < 0) {
        throw new IllegalArgumentException(fieldName + " must be non-negative");
      }
      return parsed;
    } catch (final NumberFormatException ex) {
      throw new IllegalArgumentException(fieldName + " must be a non-negative integer", ex);
    }
  }

  static String toBase64(final byte[] bytes) {
    Objects.requireNonNull(bytes, "bytes");
    return Base64.getEncoder().encodeToString(bytes);
  }

  static String requireBase64(final String value, final String fieldName) {
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException(fieldName + " must not be blank");
    }
    final String trimmed = value.trim();
    final byte[] decoded;
    try {
      decoded = Base64.getDecoder().decode(trimmed);
    } catch (final IllegalArgumentException ex) {
      throw new IllegalArgumentException(fieldName + " must be base64", ex);
    }
    if (decoded.length == 0) {
      throw new IllegalArgumentException(fieldName + " must decode to non-empty bytes");
    }
    return trimmed;
  }

  static PrivacyMode parsePrivacyMode(final Map<String, String> arguments, final String prefix) {
    final String modeKey = prefixKey(prefix, "mode");
    final String mode = arguments.getOrDefault(modeKey, "Transparent");
    final String state = arguments.get(prefixKey(prefix, "state"));
    return new PrivacyMode(mode, state);
  }

  static void appendPrivacyMode(
      final PrivacyMode privacyMode, final Map<String, String> target, final String prefix) {
    target.put(prefixKey(prefix, "mode"), privacyMode.mode());
    if (privacyMode.state() != null) {
      target.put(prefixKey(prefix, "state"), privacyMode.state());
    }
  }

  static RelayManifest parseRelayManifest(final Map<String, String> arguments, final String prefix) {
    final String expiresKey = prefixKey(prefix, "expiry_ms");
    final Long expiryMs = parseOptionalUnsignedLong(arguments.get(expiresKey), expiresKey);

    final List<RelayManifestHop> hops = new ArrayList<>();
    final String hopPrefix = prefixKey(prefix, "hop.");
    arguments.forEach(
        (key, value) -> {
          if (!key.startsWith(hopPrefix)) {
            return;
          }
          final String tail = key.substring(hopPrefix.length());
          final int separator = tail.indexOf('.');
          if (separator <= 0) {
            throw new IllegalArgumentException("Malformed relay manifest key: " + key);
          }
          final int index = Integer.parseInt(tail.substring(0, separator));
          while (hops.size() <= index) {
            hops.add(new RelayManifestHop());
          }
          final RelayManifestHop hop = hops.get(index);
          final String attribute = tail.substring(separator + 1);
          switch (attribute) {
            case "relay_id":
              hop.relayId = value;
              break;
            case "hpke_public_key":
              hop.hpkePublicKey = requireBase64(value, key);
              break;
            case "weight":
              final int parsed = parseNonNegativeInt(value, "relay hop weight");
              if (parsed > 0xFF) {
                throw new IllegalArgumentException("relay hop weight must fit in a byte");
              }
              hop.weight = parsed;
              break;
            default:
              throw new IllegalArgumentException("Unknown relay manifest attribute: " + key);
          }
        });
    if (hops.isEmpty() && expiryMs == null) {
      return null;
    }
    for (int index = 0; index < hops.size(); index++) {
      final RelayManifestHop hop = hops.get(index);
      if (hop.relayId == null || hop.relayId.isBlank()) {
        throw new IllegalArgumentException("relay_manifest.hop." + index + ".relay_id is required");
      }
      if (hop.hpkePublicKey == null || hop.hpkePublicKey.isBlank()) {
        throw new IllegalArgumentException(
            "relay_manifest.hop." + index + ".hpke_public_key is required");
      }
      if (hop.weight == null) {
        throw new IllegalArgumentException("relay_manifest.hop." + index + ".weight is required");
      }
    }
    return new RelayManifest(expiryMs, Collections.unmodifiableList(hops));
  }

  static void appendRelayManifest(
      final RelayManifest manifest, final Map<String, String> target, final String prefix) {
    if (manifest == null) {
      return;
    }
    if (manifest.expiryMs != null) {
      target.put(prefixKey(prefix, "expiry_ms"), Long.toUnsignedString(manifest.expiryMs));
    }
    for (int index = 0; index < manifest.hops.size(); index++) {
      final RelayManifestHop hop = manifest.hops.get(index);
      final String baseKey = prefixKey(prefix, "hop." + index);
      target.put(baseKey + ".relay_id", hop.relayId);
      target.put(baseKey + ".hpke_public_key", hop.hpkePublicKey);
      target.put(baseKey + ".weight", Integer.toUnsignedString(hop.weight));
    }
  }

  static String prefixKey(final String prefix, final String key) {
    if (prefix == null || prefix.isEmpty()) {
      return key;
    }
    if (prefix.endsWith(".")) {
      return prefix + key;
    }
    return prefix + "." + key;
  }

  static String require(final Map<String, String> arguments, final String key) {
    final String value = arguments.get(key);
    if (value == null || value.isBlank()) {
      throw new IllegalArgumentException("Instruction argument '" + key + "' is required");
    }
    return value;
  }

  private static boolean startsWithIgnoreCase(final String value, final String prefix) {
    return value.regionMatches(true, 0, prefix, 0, prefix.length());
  }

  /** Immutable representation of a Kaigi call identifier. */
  static final class CallId {
    private final String domainId;
    private final String callName;

    CallId(final String domainId, final String callName) {
      this.domainId = Objects.requireNonNull(domainId, "domainId");
      this.callName = Objects.requireNonNull(callName, "callName");
    }

    String domainId() {
      return domainId;
    }

    String callName() {
      return callName;
    }
  }

  /** Immutable privacy configuration descriptor. */
  static final class PrivacyMode {
    private final String mode;
    private final String state;

    PrivacyMode(final String mode, final String state) {
      if (mode == null || mode.isBlank()) {
        throw new IllegalArgumentException("privacy mode must not be blank");
      }
      final String normalized = mode.trim();
      if (!"Transparent".equals(normalized) && !"ZkRosterV1".equals(normalized)) {
        throw new IllegalArgumentException("privacy mode must be Transparent or ZkRosterV1");
      }
      this.mode = normalized;
      this.state = state == null || state.isBlank() ? null : state;
    }

    String mode() {
      return mode;
    }

    String state() {
      return state;
    }
  }

  /** Immutable relay manifest snapshot. */
  static final class RelayManifest {
    private final Long expiryMs;
    private final List<RelayManifestHop> hops;

    RelayManifest(final Long expiryMs, final List<RelayManifestHop> hops) {
      this.expiryMs = expiryMs;
      this.hops = hops == null ? List.of() : hops;
    }

    Long expiryMs() {
      return expiryMs;
    }

    List<RelayManifestHop> hops() {
      return hops;
    }
  }

  /** Single relay hop entry. */
  static final class RelayManifestHop {
    private String relayId;
    private String hpkePublicKey;
    private Integer weight;

    RelayManifestHop() {}

    RelayManifestHop(final String relayId, final String hpkePublicKey, final Integer weight) {
      this.relayId = relayId;
      this.hpkePublicKey = hpkePublicKey;
      this.weight = weight;
    }

    RelayManifestHop withRelayId(final String value) {
      this.relayId = Objects.requireNonNull(value, "relayId");
      return this;
    }

    RelayManifestHop withHpkePublicKey(final String value) {
      this.hpkePublicKey = Objects.requireNonNull(value, "hpkePublicKey");
      return this;
    }

    RelayManifestHop withWeight(final int value) {
      if (value < 0 || value > 0xFF) {
        throw new IllegalArgumentException("relay hop weight must fit in an unsigned byte");
      }
      this.weight = value;
      return this;
    }

    String relayId() {
      return relayId;
    }

    String hpkePublicKey() {
      return hpkePublicKey;
    }

    Integer weight() {
      return weight;
    }
  }
}

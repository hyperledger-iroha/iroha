package org.hyperledger.iroha.android.client;

import java.util.Arrays;
import java.util.Collections;
import java.util.HashSet;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Optional;
import java.util.Set;

/** Helpers for validating Torii's metadata-only public pipeline status. */
final class PipelineStatusExtractor {
  private static final Set<String> STATUS_KINDS =
      immutableSet("Queued", "Approved", "Committed", "Applied", "Rejected", "Expired");
  private static final Set<String> ROOT_FIELDS =
      immutableSet("hash", "status", "scope", "resolved_from");
  private static final Set<String> STATUS_FIELDS = immutableSet("kind", "block_height");
  private static final Set<String> SCOPES = immutableSet("local", "global");
  private static final Set<String> SOURCES = immutableSet("queue", "cache", "state");

  private PipelineStatusExtractor() {}

  /** Return the canonical top-level status kind, if present. */
  static Optional<String> extractStatusKind(final Object payload) {
    if (!(payload instanceof Map)) {
      return Optional.empty();
    }
    final Object status = ((Map<?, ?>) payload).get("status");
    if (!(status instanceof Map)) {
      return Optional.empty();
    }
    final Object kind = ((Map<?, ?>) status).get("kind");
    return kind instanceof String && STATUS_KINDS.contains(kind)
        ? Optional.of((String) kind)
        : Optional.empty();
  }

  /** Reject retired detail fields and return a fresh metadata-only map. */
  static Map<String, Object> normalizePublicStatus(final Map<String, Object> payload) {
    if (payload == null) {
      throw new IllegalStateException("Pipeline status response must not be empty");
    }
    if (!ROOT_FIELDS.equals(payload.keySet())) {
      final Set<String> extras = new HashSet<>(payload.keySet());
      extras.removeAll(ROOT_FIELDS);
      if (!extras.isEmpty()) {
        throw new IllegalStateException(
            "Pipeline status contains retired or unsupported fields: " + extras);
      }
      final Set<String> missing = new HashSet<>(ROOT_FIELDS);
      missing.removeAll(payload.keySet());
      throw new IllegalStateException("Pipeline status is missing required fields: " + missing);
    }
    final Object hashValue = payload.get("hash");
    if (!(hashValue instanceof String)
        || !((String) hashValue).matches("[0-9a-f]{63}[13579bdf]")) {
      throw new IllegalStateException(
          "Pipeline status hash must be an exact lowercase marked 32-byte hash");
    }
    final Object rawStatusValue = payload.get("status");
    if (!(rawStatusValue instanceof Map)) {
      throw new IllegalStateException("Pipeline status kind is missing or unsupported");
    }
    final Map<?, ?> rawStatus = (Map<?, ?>) rawStatusValue;
    if (!rawStatus.containsKey("kind") || !STATUS_FIELDS.containsAll(rawStatus.keySet())) {
      throw new IllegalStateException(
          "Pipeline status contains retired, unsupported, or missing status fields");
    }
    final Object kindValue = rawStatus.get("kind");
    if (!(kindValue instanceof String) || !STATUS_KINDS.contains(kindValue)) {
      throw new IllegalStateException("Pipeline status kind is missing or unsupported");
    }
    final LinkedHashMap<String, Object> status = new LinkedHashMap<>();
    status.put("kind", kindValue);
    if (rawStatus.containsKey("block_height")) {
      final Object blockHeight = rawStatus.get("block_height");
      if (!hasPositiveBlockHeight(blockHeight)) {
        throw new IllegalStateException("Pipeline status block height must be a positive integer");
      }
      status.put("block_height", blockHeight);
    }
    final Object scope = payload.get("scope");
    if (!(scope instanceof String) || !SCOPES.contains(scope)) {
      throw new IllegalStateException("Pipeline status has an unsupported scope");
    }
    final Object resolvedFrom = payload.get("resolved_from");
    if (!(resolvedFrom instanceof String) || !SOURCES.contains(resolvedFrom)) {
      throw new IllegalStateException("Pipeline status has an unsupported resolution source");
    }
    final LinkedHashMap<String, Object> normalized = new LinkedHashMap<>();
    normalized.put("hash", hashValue);
    normalized.put("status", Collections.unmodifiableMap(status));
    normalized.put("scope", scope);
    normalized.put("resolved_from", resolvedFrom);
    return Collections.unmodifiableMap(normalized);
  }

  /** Validate one exact global status observation and return its canonical kind. */
  static String requireAuthoritativeStatus(
      final Map<String, Object> payload, final String expectedHash) {
    if (expectedHash == null || !expectedHash.matches("[0-9a-f]{63}[13579bdf]")) {
      throw new IllegalStateException(
          "Requested transaction hash must be an exact lowercase marked 32-byte hash");
    }
    final Map<String, Object> normalized = normalizePublicStatus(payload);
    if (!expectedHash.equals(normalized.get("hash"))) {
      throw new IllegalStateException(
          "Pipeline status hash does not match the requested transaction hash");
    }
    if (!"global".equals(normalized.get("scope"))) {
      throw new IllegalStateException("Pipeline status must use global scope");
    }
    final Map<?, ?> status = (Map<?, ?>) normalized.get("status");
    final String kind = (String) status.get("kind");
    if ("Applied".equals(kind)) {
      if (!hasPositiveBlockHeight(status.get("block_height"))) {
        throw new IllegalStateException(
            "Applied pipeline status must have a positive block height");
      }
    }
    return kind;
  }

  static Optional<String> normalizeStatus(final String statusLiteral) {
    return statusLiteral != null && STATUS_KINDS.contains(statusLiteral)
        ? Optional.of(statusLiteral)
        : Optional.empty();
  }

  private static boolean hasPositiveBlockHeight(final Object blockHeight) {
    if (!(blockHeight instanceof Number)) {
      return false;
    }
    final double value = ((Number) blockHeight).doubleValue();
    return Double.isFinite(value) && value > 0 && value == Math.rint(value);
  }

  private static Set<String> immutableSet(final String... values) {
    return Collections.unmodifiableSet(new HashSet<>(Arrays.asList(values)));
  }
}

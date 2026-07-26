package org.hyperledger.iroha.android.client;

import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.Set;

/**
 * Helpers for parsing Torii pipeline status payloads.
 */
final class PipelineStatusExtractor {
  private static final Set<String> STATUS_KINDS =
      Set.of("Queued", "Approved", "Committed", "Applied", "Rejected", "Expired");

  private PipelineStatusExtractor() {}

  static Optional<String> extractStatusKind(final Object payload) {
    if (!(payload instanceof Map)) {
      return Optional.empty();
    }
    final Map<?, ?> payloadMap = (Map<?, ?>) payload;
    return coerceStatus(payloadMap.get("status"));
  }

  static Optional<String> extractRejectionReason(final Object payload) {
    if (!(payload instanceof Map)) {
      return Optional.empty();
    }
    final Map<?, ?> payloadMap = (Map<?, ?>) payload;
    final Object diagnostics = payloadMap.get("diagnostics");
    if (diagnostics instanceof List<?>) {
      for (final Object diagnostic : (List<?>) diagnostics) {
        if (diagnostic instanceof Map<?, ?>) {
          final Map<?, ?> record = (Map<?, ?>) diagnostic;
          final Optional<String> decoded = coerceReason(record.get("decoded_reason"));
          if (decoded.isPresent()) {
            return decoded;
          }
          final Optional<String> message = coerceReason(record.get("message"));
          if (message.isPresent()) {
            return message;
          }
        }
      }
    }

    final Object status = payloadMap.get("status");
    if (status instanceof Map<?, ?>) {
      final Optional<String> rejection =
          coerceReason(((Map<?, ?>) status).get("rejection_reason"));
      if (rejection.isPresent()) {
        return rejection;
      }
    }

    final Optional<String> summary = coerceReason(payloadMap.get("summary"));
    if (summary.isPresent() && !summary.equals(extractStatusKind(payload))) {
      return summary;
    }
    return Optional.empty();
  }

  static String requireAuthoritativeStatus(
      final Map<String, Object> payload, final String expectedHash) {
    if (payload == null) {
      throw new IllegalStateException("Pipeline status response must not be empty");
    }
    if (!expectedHash.equals(payload.get("hash"))) {
      throw new IllegalStateException(
          "Pipeline status hash does not match the requested transaction hash");
    }
    if (!"global".equals(payload.get("scope"))) {
      throw new IllegalStateException("Pipeline status must use global scope");
    }
    if (!(payload.get("summary") instanceof String)) {
      throw new IllegalStateException("Pipeline status summary is missing or malformed");
    }

    final String kind =
        extractStatusKind(payload)
            .orElseThrow(
                () -> new IllegalStateException("Pipeline status kind is missing or unsupported"));
    final Object resolvedFrom = payload.get("resolved_from");
    if (!(resolvedFrom instanceof String)) {
      throw new IllegalStateException("Pipeline status resolution source is missing");
    }

    if ("Applied".equals(kind)) {
      if (!"state".equals(resolvedFrom) || !hasPositiveBlockHeight(payload)) {
        throw new IllegalStateException(
            "Applied pipeline status must be state-resolved with a positive block height");
      }
    } else if ("Rejected".equals(kind) || "Expired".equals(kind)) {
      if (!"state".equals(resolvedFrom)) {
        throw new IllegalStateException(
            "Terminal pipeline failure must be resolved from state");
      }
    } else if (!Set.of("queue", "cache", "state").contains(resolvedFrom)) {
      throw new IllegalStateException("Pipeline status has an unsupported resolution source");
    }
    return kind;
  }

  private static Optional<String> coerceStatus(final Object status) {
    if (status instanceof Map<?, ?>) {
      final Object kind = ((Map<?, ?>) status).get("kind");
      if (kind instanceof String) {
        return normalizeStatus((String) kind);
      }
    }
    return Optional.empty();
  }

  private static Optional<String> coerceReason(final Object reason) {
    if (reason == null) {
      return Optional.empty();
    }
    final String text = reason.toString().trim();
    if (text.isEmpty()) {
      return Optional.empty();
    }
    return Optional.of(text);
  }

  static Optional<String> normalizeStatus(final String statusLiteral) {
    return statusLiteral != null && STATUS_KINDS.contains(statusLiteral)
        ? Optional.of(statusLiteral)
        : Optional.empty();
  }

  private static boolean hasPositiveBlockHeight(final Map<String, Object> payload) {
    final Object status = payload.get("status");
    if (!(status instanceof Map<?, ?>)) {
      return false;
    }
    final Object blockHeight = ((Map<?, ?>) status).get("block_height");
    if (!(blockHeight instanceof Number)) {
      return false;
    }
    final double value = ((Number) blockHeight).doubleValue();
    return Double.isFinite(value) && value > 0 && value == Math.rint(value);
  }
}

package org.hyperledger.iroha.android.client;

import java.util.Map;
import java.util.Optional;

/**
 * Helpers for parsing Torii pipeline status payloads.
 */
final class PipelineStatusExtractor {
  private static final String[] REJECTION_REASON_KEYS =
      new String[] {"rejection_reason", "rejectionReason", "reason", "reject_code", "rejectCode"};

  private PipelineStatusExtractor() {}

  static Optional<String> extractStatusKind(final Object payload) {
    if (!(payload instanceof Map)) {
      return Optional.empty();
    }
    final Map<?, ?> payloadMap = (Map<?, ?>) payload;
    final Optional<String> direct = coerceStatus(payloadMap.get("status"));
    if (direct.isPresent()) {
      return direct;
    }
    final Object content = payloadMap.get("content");
    if (content instanceof Map) {
      return coerceStatus(((Map<?, ?>) content).get("status"));
    }
    return Optional.empty();
  }

  static Optional<String> extractRejectionReason(final Object payload) {
    if (!(payload instanceof Map)) {
      return Optional.empty();
    }
    final Map<?, ?> payloadMap = (Map<?, ?>) payload;
    final Optional<String> direct = coerceReasonFromRecord(payloadMap);
    if (direct.isPresent()) {
      return direct;
    }
    final Object content = payloadMap.get("content");
    if (content instanceof Map) {
      final Map<?, ?> contentMap = (Map<?, ?>) content;
      final Optional<String> contentReason = coerceReasonFromRecord(contentMap);
      if (contentReason.isPresent()) {
        return contentReason;
      }
      final Object status = contentMap.get("status");
      if (status instanceof Map) {
        final Map<?, ?> statusMap = (Map<?, ?>) status;
        final Optional<String> statusReason = coerceReasonFromRecord(statusMap);
        if (statusReason.isPresent()) {
          return statusReason;
        }
        if ("Rejected".equalsIgnoreCase(String.valueOf(statusMap.get("kind")))) {
          return coerceReason(statusMap.get("content"));
        }
      }
    }
    return Optional.empty();
  }

  private static Optional<String> coerceStatus(final Object status) {
    if (status instanceof Map) {
      final Object kind = ((Map<?, ?>) status).get("kind");
      if (kind != null) {
        return normalizeStatus(kind.toString());
      }
    } else if (status != null) {
      return normalizeStatus(status.toString());
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

  private static Optional<String> coerceReasonFromRecord(final Map<?, ?> record) {
    for (final String key : REJECTION_REASON_KEYS) {
      final Optional<String> reason = coerceReason(record.get(key));
      if (reason.isPresent()) {
        return reason;
      }
    }
    final Optional<String> parsedFromStatus = parseReasonFromStatus(record.get("status"));
    if (parsedFromStatus.isPresent()) {
      return parsedFromStatus;
    }
    final Optional<String> parsedFromKind = parseReasonFromStatus(record.get("kind"));
    if (parsedFromKind.isPresent()) {
      return parsedFromKind;
    }
    return Optional.empty();
  }

  static Optional<String> normalizeStatus(final String statusLiteral) {
    if (statusLiteral == null) {
      return Optional.empty();
    }
    final String trimmed = statusLiteral.trim();
    if (trimmed.isEmpty()) {
      return Optional.empty();
    }
    if (trimmed.startsWith("Queued")) {
      return Optional.of("Queued");
    }
    if (trimmed.startsWith("Approved")) {
      return Optional.of("Approved");
    }
    if (trimmed.startsWith("Committed")) {
      return Optional.of("Committed");
    }
    if (trimmed.startsWith("Applied")) {
      return Optional.of("Applied");
    }
    if (trimmed.startsWith("Rejected")) {
      return Optional.of("Rejected");
    }
    if (trimmed.startsWith("Expired")) {
      return Optional.of("Expired");
    }
    return Optional.of(trimmed);
  }

  private static Optional<String> parseReasonFromStatus(final Object statusValue) {
    if (!(statusValue instanceof String)) {
      return Optional.empty();
    }
    final String trimmed = ((String) statusValue).trim();
    if (!trimmed.startsWith("Rejected(") || !trimmed.endsWith(")")) {
      return Optional.empty();
    }
    final String reason = trimmed.substring("Rejected(".length(), trimmed.length() - 1).trim();
    return reason.isEmpty() ? Optional.empty() : Optional.of(reason);
  }
}

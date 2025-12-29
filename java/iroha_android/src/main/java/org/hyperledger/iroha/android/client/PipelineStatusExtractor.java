package org.hyperledger.iroha.android.client;

import java.util.Map;
import java.util.Optional;

/**
 * Helpers for parsing Torii pipeline status payloads.
 */
final class PipelineStatusExtractor {

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

  private static Optional<String> coerceStatus(final Object status) {
    if (status instanceof Map) {
      final Object kind = ((Map<?, ?>) status).get("kind");
      if (kind != null) {
        return Optional.of(kind.toString());
      }
    } else if (status != null) {
      return Optional.of(status.toString());
    }
    return Optional.empty();
  }
}

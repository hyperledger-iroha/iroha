package org.hyperledger.iroha.android.client;

import java.util.HashMap;
import java.util.Map;
import java.util.Optional;

public final class PipelineStatusExtractorTests {

  private PipelineStatusExtractorTests() {}

  public static void main(final String[] args) {
    extractStatusKindFromNestedContent();
    extractStatusKindFromDirectString();
    extractStatusKindMissingStatus();
    System.out.println("[IrohaAndroid] Pipeline status extractor tests passed.");
  }

  private static void extractStatusKindFromNestedContent() {
    final Map<String, Object> nestedStatus = new HashMap<>();
    nestedStatus.put("kind", "Committed");

    final Map<String, Object> content = new HashMap<>();
    content.put("status", nestedStatus);

    final Map<String, Object> payload = new HashMap<>();
    payload.put("kind", "Transaction");
    payload.put("content", content);

    final Optional<String> status = PipelineStatusExtractor.extractStatusKind(payload);
    assert status.isPresent() : "Expected status to be present";
    assert "Committed".equals(status.get()) : "Expected nested status kind";
  }

  private static void extractStatusKindFromDirectString() {
    final Map<String, Object> payload = new HashMap<>();
    payload.put("status", "Rejected");

    final Optional<String> status = PipelineStatusExtractor.extractStatusKind(payload);
    assert status.isPresent() : "Expected status to be present";
    assert "Rejected".equals(status.get()) : "Expected direct status string";
  }

  private static void extractStatusKindMissingStatus() {
    final Optional<String> status = PipelineStatusExtractor.extractStatusKind(new HashMap<>());
    assert status.isEmpty() : "Expected empty optional when status is missing";
    assert PipelineStatusExtractor.extractStatusKind(null).isEmpty()
        : "Expected empty optional when payload is null";
  }
}

package org.hyperledger.iroha.android.client;

import java.util.Arrays;
import java.util.Collections;
import java.util.HashMap;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Optional;
import org.hyperledger.iroha.norito.NoritoAdapters;
import org.hyperledger.iroha.norito.NoritoCodec;
import org.hyperledger.iroha.norito.TypeAdapter;

public final class PipelineStatusExtractorTests {

  private PipelineStatusExtractorTests() {}

  public static void main(final String[] args) {
    extractStatusKindFromCurrentPayload();
    rejectNestedStatusEnvelope();
    rejectDirectStatusString();
    rejectNonCanonicalStatusKinds();
    extractStatusKindMissingStatus();
    extractRejectionReasonFromCurrentDiagnostics();
    extractRejectCodeFromErrorEnvelopeBody();
    extractRejectCodeFromNoritoErrorEnvelopeBody();
    System.out.println("[IrohaAndroid] Pipeline status extractor tests passed.");
  }

  private static void extractStatusKindFromCurrentPayload() {
    final Map<String, Object> statusRecord = new HashMap<>();
    statusRecord.put("kind", "Applied");
    final Map<String, Object> payload = new HashMap<>();
    payload.put("status", statusRecord);

    final Optional<String> status = PipelineStatusExtractor.extractStatusKind(payload);
    assert status.isPresent() : "Expected status to be present";
    assert "Applied".equals(status.get()) : "Expected current flat status kind";
  }

  private static void rejectNestedStatusEnvelope() {
    final Map<String, Object> nestedStatus = new HashMap<>();
    nestedStatus.put("kind", "Applied");
    final Map<String, Object> content = new HashMap<>();
    content.put("status", nestedStatus);
    final Map<String, Object> payload = new HashMap<>();
    payload.put("content", content);
    assert !PipelineStatusExtractor.extractStatusKind(payload).isPresent()
        : "Retired nested status envelopes must be rejected";
  }

  private static void rejectDirectStatusString() {
    final Map<String, Object> payload = new HashMap<>();
    payload.put("status", "Applied");
    assert !PipelineStatusExtractor.extractStatusKind(payload).isPresent()
        : "Status must use the current typed object";
  }

  private static void rejectNonCanonicalStatusKinds() {
    for (final String kind : Arrays.asList(" Applied", "Applied ", "Applied(extra)", "Unknown")) {
      final Map<String, Object> statusRecord = new HashMap<>();
      statusRecord.put("kind", kind);
      final Map<String, Object> payload = new HashMap<>();
      payload.put("status", statusRecord);
      assert !PipelineStatusExtractor.extractStatusKind(payload).isPresent()
          : "Non-canonical status kind must be rejected: " + kind;
    }
  }

  private static void extractStatusKindMissingStatus() {
    final Optional<String> status = PipelineStatusExtractor.extractStatusKind(new HashMap<>());
    assert !status.isPresent() : "Expected empty optional when status is missing";
    assert !PipelineStatusExtractor.extractStatusKind(null).isPresent()
        : "Expected empty optional when payload is null";
  }

  private static void extractRejectionReasonFromCurrentDiagnostics() {
    final Map<String, Object> diagnostic = new HashMap<>();
    diagnostic.put("decoded_reason", "missing exact permission");
    diagnostic.put("message", "fallback message");
    final Map<String, Object> statusRecord = new HashMap<>();
    statusRecord.put("kind", "Rejected");
    final Map<String, Object> payload = new HashMap<>();
    payload.put("status", statusRecord);
    payload.put("diagnostics", Collections.singletonList(diagnostic));

    final Optional<String> reason = PipelineStatusExtractor.extractRejectionReason(payload);
    assert reason.isPresent() : "Expected current diagnostic reason to be present";
    assert "missing exact permission".equals(reason.get()) : "Expected decoded reason";
  }

  private static void extractRejectCodeFromErrorEnvelopeBody() {
    final byte[] body =
        """
        {
          "code": "queue_full",
          "message": "transaction queue is at capacity",
          "details": {
            "reject_code": "TX_QUEUE_FULL",
            "retry_after_seconds": 1,
            "queue": {
              "state": "saturated",
              "queued": 128,
              "capacity": 128,
              "saturated": true
            }
          }
        }
        """
            .getBytes(java.nio.charset.StandardCharsets.UTF_8);

    final String rejectCode =
        HttpErrorMessageExtractor.extractRejectCode(
            Collections.emptyMap(), "x-iroha-reject-code", body);
    assert "TX_QUEUE_FULL".equals(rejectCode) : "Expected error envelope details reject code";
    assert "transaction queue is at capacity".equals(HttpErrorMessageExtractor.extractMessage(body))
        : "Expected envelope message";
  }

  private static void extractRejectCodeFromNoritoErrorEnvelopeBody() {
    final byte[] body =
        encodeErrorEnvelope("queue_full", "transaction queue is at capacity", "TX_QUEUE_FULL");
    final String rejectCode =
        HttpErrorMessageExtractor.extractRejectCode(
            Collections.emptyMap(), "x-iroha-reject-code", body);
    assert "TX_QUEUE_FULL".equals(rejectCode) : "Expected Norito error envelope reject code";
    assert "transaction queue is at capacity".equals(HttpErrorMessageExtractor.extractMessage(body))
        : "Expected Norito envelope message";
  }

  private static byte[] encodeErrorEnvelope(
      final String code, final String message, final String rejectCode) {
    final TypeAdapter<Optional<String>> optionalString =
        NoritoAdapters.option(NoritoAdapters.stringAdapter());
    final TypeAdapter<Object> detailsAdapter =
        NoritoAdapters.struct(
            Arrays.asList(
                NoritoAdapters.field("reject_code", optionalString),
                NoritoAdapters.field("queue", optionalString),
                NoritoAdapters.field(
                    "retry_after_seconds", NoritoAdapters.option(NoritoAdapters.uint(64))),
                NoritoAdapters.field("endpoint", optionalString),
                NoritoAdapters.field("axt", optionalString)));
    final TypeAdapter<Object> envelopeAdapter =
        NoritoAdapters.struct(
            Arrays.asList(
                NoritoAdapters.field("code", NoritoAdapters.stringAdapter()),
                NoritoAdapters.field("message", NoritoAdapters.stringAdapter()),
                NoritoAdapters.field("details", NoritoAdapters.option(detailsAdapter))));
    final Map<String, Object> details = new LinkedHashMap<>();
    details.put("reject_code", Optional.of(rejectCode));
    details.put("queue", Optional.empty());
    details.put("retry_after_seconds", Optional.empty());
    details.put("endpoint", Optional.empty());
    details.put("axt", Optional.empty());
    final Map<String, Object> envelope = new LinkedHashMap<>();
    envelope.put("code", code);
    envelope.put("message", message);
    envelope.put("details", Optional.of(details));
    return NoritoCodec.encode(
        (Object) envelope, "iroha_torii_shared::ErrorEnvelope", envelopeAdapter);
  }
}

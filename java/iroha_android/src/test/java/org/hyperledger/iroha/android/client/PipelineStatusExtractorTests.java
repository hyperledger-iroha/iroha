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
    normalizeMetadataOnlyStatus();
    keepNonStateTerminalHintsNonFinal();
    acceptOnlyExactLocalOrGlobalScopes();
    rejectNonCanonicalHashRepresentations();
    rejectRetiredStatusDetails();
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

  private static void normalizeMetadataOnlyStatus() {
    final String hash = String.join("", Collections.nCopies(32, "ab"));
    final Map<String, Object> statusRecord = new HashMap<>();
    statusRecord.put("kind", "Applied");
    statusRecord.put("block_height", 7L);
    final Map<String, Object> payload = new HashMap<>();
    payload.put("hash", hash);
    payload.put("status", statusRecord);
    payload.put("scope", "global");
    payload.put("resolved_from", "state");

    final Map<String, Object> normalized = PipelineStatusExtractor.normalizePublicStatus(payload);
    assert normalized.keySet().equals(
        new java.util.HashSet<>(
            Arrays.asList("hash", "status", "scope", "resolved_from")))
        : "Public status must expose only metadata";
    assert PipelineStatusExtractor.requireAuthoritativeStatus(normalized, hash)
        .equals("Applied") : "Expected canonical Applied status";
  }

  private static void keepNonStateTerminalHintsNonFinal() {
    final String hash = String.join("", Collections.nCopies(32, "ab"));
    for (final String kind : Arrays.asList("Applied", "Rejected", "Expired")) {
      for (final String source : Arrays.asList("cache", "queue")) {
        final Map<String, Object> status = new LinkedHashMap<>();
        status.put("kind", kind);
        if ("Applied".equals(kind)) {
          status.put("block_height", 7L);
        }
        final Map<String, Object> payload = new LinkedHashMap<>();
        payload.put("hash", hash);
        payload.put("status", status);
        payload.put("scope", "global");
        payload.put("resolved_from", source);
        assert kind.equals(PipelineStatusExtractor.requireAuthoritativeStatus(payload, hash))
            : "Structurally valid non-state status must remain observable";
        if ("Applied".equals(kind)) {
          try {
            TransactionFinality.requireApplied(payload, hash);
            throw new AssertionError("Cached or queued Applied must not prove finality");
          } catch (final IllegalStateException expected) {
            assert expected.getMessage().contains("exact Applied")
                : "Expected exact finality rejection";
          }
        }
      }
    }
  }

  private static void acceptOnlyExactLocalOrGlobalScopes() {
    final String hash = String.join("", Collections.nCopies(32, "ab"));
    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("hash", hash);
    payload.put("status", Collections.singletonMap("kind", "Queued"));
    payload.put("scope", "local");
    payload.put("resolved_from", "queue");
    assert "local".equals(PipelineStatusExtractor.normalizePublicStatus(payload).get("scope"))
        : "Exact local status scope must remain valid";

    payload.put("scope", "auto");
    try {
      PipelineStatusExtractor.normalizePublicStatus(payload);
      throw new AssertionError("Retired auto scope must be rejected");
    } catch (final IllegalStateException expected) {
      assert expected.getMessage().contains("unsupported scope")
          : "Expected exact status-scope rejection";
    }
  }

  private static void rejectNonCanonicalHashRepresentations() {
    final String hash = String.join("", Collections.nCopies(32, "ab"));
    final Map<String, Object> payload = new LinkedHashMap<>();
    payload.put("hash", hash);
    payload.put("status", Map.of("kind", "Applied", "block_height", 7L));
    payload.put("scope", "global");
    payload.put("resolved_from", "state");
    final Object[] invalidHashes = {
      hash.toUpperCase(java.util.Locale.ROOT),
      " " + hash,
      hash + " ",
      "0x" + hash,
      hash.substring(0, hash.length() - 2),
      String.join("", Collections.nCopies(32, "aa")),
      new byte[32],
      Collections.nCopies(32, Integer.valueOf(0xab))
    };
    for (final Object invalid : invalidHashes) {
      payload.put("hash", invalid);
      try {
        PipelineStatusExtractor.normalizePublicStatus(payload);
        throw new AssertionError("Expected non-canonical response hash rejection: " + invalid);
      } catch (final IllegalStateException expected) {
        assert expected.getMessage().contains("exact lowercase")
            : "Expected exact response hash failure";
      }
    }

    payload.put("hash", hash);
    for (final String invalid :
        Arrays.asList(
            hash.toUpperCase(java.util.Locale.ROOT),
            " " + hash,
            hash + " ",
            "0x" + hash,
            hash.substring(0, hash.length() - 2),
            String.join("", Collections.nCopies(32, "aa")))) {
      try {
        PipelineStatusExtractor.requireAuthoritativeStatus(payload, invalid);
        throw new AssertionError("Expected non-canonical requested hash rejection: " + invalid);
      } catch (final IllegalStateException expected) {
        assert expected.getMessage().contains("exact lowercase")
            : "Expected exact requested hash failure";
      }
    }
  }

  private static void rejectRetiredStatusDetails() {
    final String hash = String.join("", Collections.nCopies(32, "cd"));
    final Map<String, Object> statusRecord = new HashMap<>();
    statusRecord.put("kind", "Rejected");
    statusRecord.put("rejection_reason", "secret");
    final Map<String, Object> payload = new HashMap<>();
    payload.put("hash", hash);
    payload.put("status", statusRecord);
    payload.put("scope", "global");
    payload.put("resolved_from", "state");
    payload.put("diagnostics", Collections.singletonList("secret"));

    try {
      PipelineStatusExtractor.normalizePublicStatus(payload);
      throw new AssertionError("Retired status details must be rejected");
    } catch (final IllegalStateException expected) {
      assert expected.getMessage().contains("retired or unsupported fields")
          : "Expected closed public status fields";
    }
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

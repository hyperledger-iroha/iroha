package org.hyperledger.iroha.android.offline;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.Locale;

public final class OfflineJsonParserTest {

  private OfflineJsonParserTest() {}

  public static void main(final String[] args) {
    parsesOfflineReadiness();
    ignoresUnknownOfflineReadinessMembers();
    rejectsMalformedUtf8WithoutReplacement();
    rejectsNonCanonicalOfflineReadiness();
    parsesOfflineTransfers();
    rejectsOfflineTransferMalformedIntegerFields();
    canonicalizesJson();
    System.out.println("[IrohaAndroid] OfflineJsonParserTest passed.");
  }

  private static void parsesOfflineReadiness() {
    final String json =
        """
        {
          "asset_definition_id": "xor#wonderland",
          "evaluated_block_height": 18446744073709551615,
          "evaluated_block_hash": "abababababababababababababababababababababababababababababababab",
          "ready": false,
          "blockers": [
            {"code": "offline_disabled", "message": "Offline transfers are disabled"}
          ]
        }
        """;
    final OfflineReadiness readiness =
        OfflineJsonParser.parseOfflineReadiness(json.getBytes(StandardCharsets.UTF_8));
    assert "xor#wonderland".equals(readiness.assetDefinitionId());
    assert new BigInteger("18446744073709551615").equals(readiness.evaluatedBlockHeight());
    assert "abababababababababababababababababababababababababababababababab"
        .equals(readiness.evaluatedBlockHash());
    assert !readiness.ready();
    assert readiness.blockers().size() == 1;
    assert "offline_disabled".equals(readiness.blockers().get(0).code());
    assert "Offline transfers are disabled".equals(readiness.blockers().get(0).message());
  }

  private static void ignoresUnknownOfflineReadinessMembers() {
    final OfflineReadiness readiness =
        parseOfflineReadiness(
            readinessBody(
                "\"future_top_level\": {\"ignored\": true},",
                "\"xor#wonderland\"",
                "7",
                "false",
                "[{\"code\":\"2fa_required\",\"message\":\"no\",\"future_detail\":7}]"));
    assert readiness.blockers().size() == 1;
    assert "2fa_required".equals(readiness.blockers().get(0).code());
  }

  private static void rejectsMalformedUtf8WithoutReplacement() {
    final byte[] payload =
        readinessBody("", "\"xor#wonderland\"", "7", "true", "[]")
            .getBytes(StandardCharsets.UTF_8);
    final byte[] marker = "xor#wonderland".getBytes(StandardCharsets.US_ASCII);
    final int offset = findSubsequence(payload, marker);
    if (offset < 0) {
      throw new AssertionError("readiness asset marker missing from test payload");
    }
    payload[offset] = (byte) 0xc3;
    expectIllegalState(
        () -> OfflineJsonParser.parseOfflineReadiness(payload),
        "Offline JSON payload must be valid UTF-8");
  }

  private static void rejectsNonCanonicalOfflineReadiness() {
    for (final MalformedReadinessCase item : malformedReadinessCases()) {
      expectIllegalState(() -> parseOfflineReadiness(item.body), item.message);
    }
  }


  private static void parsesOfflineTransfers() {
    final String json =
        """
        {
          "items": [
            {
              "bundle_id_hex": "0xabc123",
              "controller_id": "payer",
              "controller_display": "Payer",
              "receiver_id": "payee",
              "receiver_display": "Payee",
              "deposit_account_id": "deposit",
              "deposit_account_display": "Deposit",
              "asset_id": "pkr#paynet",
              "total_amount": "500",
              "claimed_delta": "500",
              "status": "PENDING",
              "receipt_count": 1,
              "recorded_at_ms": 1767648180000,
              "recorded_at_height": 12345,
              "receipt_summaries": [
                {
                  "sender_id": "payer",
                  "receiver_id": "payee",
                  "amount": "500",
                  "asset_id": "pkr#paynet",
                  "status": "PENDING"
                }
              ]
            }
          ],
          "total": 1
        }
        """;
    final OfflineTransferList transfers =
        OfflineJsonParser.parseTransfers(json.getBytes(StandardCharsets.UTF_8));
    assert transfers.total() == 1L;
    assert transfers.items().size() == 1;
    final OfflineTransferList.OfflineTransferItem item = transfers.items().get(0);
    assert "0xabc123".equals(item.bundleIdHex());
    assert "Payee".equals(item.receiverDisplay());
    assert "Deposit".equals(item.depositAccountDisplay());
    assert item.receiptCount() == 1L;
    assert item.firstReceiptSummary().isPresent();
    assert "payer".equals(item.firstReceiptSummary().get().senderId());
    assert "pkr#paynet".equals(item.toJsonMap().get("asset_id"));
  }

  private static void rejectsOfflineTransferMalformedIntegerFields() {
    for (final MalformedTransferIntegerCase item : malformedTransferIntegerCases()) {
      expectIllegalState(() -> parseOfflineTransfers(item.body), item.message);
    }
  }

  private static void canonicalizesJson() {
    final String encoded =
        OfflineJsonParser.canonicalJson("{\"b\":2,\"a\":1}".getBytes(StandardCharsets.UTF_8));
    assert "{\"a\":1,\"b\":2}".equals(encoded) : "canonical JSON mismatch: " + encoded;
  }

  private static OfflineReadiness parseOfflineReadiness(final String json) {
    return OfflineJsonParser.parseOfflineReadiness(json.getBytes(StandardCharsets.UTF_8));
  }


  private static OfflineTransferList parseOfflineTransfers(final String json) {
    return OfflineJsonParser.parseTransfers(json.getBytes(StandardCharsets.UTF_8));
  }

  private record MalformedReadinessCase(String body, String message) {}

  private record MalformedTransferIntegerCase(String body, String message) {}

  private static MalformedReadinessCase[] malformedReadinessCases() {
    return new MalformedReadinessCase[] {
      new MalformedReadinessCase(
          readinessBody("", "\"xor#wonderland\"", "\"7\"", "true", "[]"),
          "evaluated_block_height must be a JSON integer number"),
      new MalformedReadinessCase(
          readinessBody("", "\"xor#wonderland\"", "-1", "true", "[]"),
          "evaluated_block_height must fit in an unsigned 64-bit integer"),
      new MalformedReadinessCase(
          readinessBody("", "\"xor#wonderland\"", "18446744073709551616", "true", "[]"),
          "evaluated_block_height must fit in an unsigned 64-bit integer"),
      new MalformedReadinessCase(
          readinessBody("", "\"xor#wonderland\"", "7", "1", "[]"),
          "ready must be a boolean"),
      new MalformedReadinessCase(
          readinessBody("", "\" xor#wonderland\"", "7", "true", "[]"),
          "asset_definition_id must be an exact non-empty string"),
      new MalformedReadinessCase(
          readinessBody("", "\"xor#wonderland\"", "7", "true",
              "[{\"code\":\"blocked\",\"message\":1}]"),
          "blockers[0].message must be a string"),
      new MalformedReadinessCase(
          readinessBody("", "\"xor#wonderland\"", "7", "false",
              "[{\"code\":\"Bad-Code\",\"message\":\"no\"}]"),
          "code must be a 1-64 character lowercase stable identifier"),
      new MalformedReadinessCase(
          readinessBody("", "\"xor#wonderland\"", "7", "false", "[]"),
          "ready must be true exactly when blockers is empty"),
      new MalformedReadinessCase(
          readinessBody("", "\"xor#wonderland\"", "7", "true",
              "[{\"code\":\"blocked\",\"message\":\"no\"}]"),
          "ready must be true exactly when blockers is empty")
    };
  }

  private static MalformedTransferIntegerCase[] malformedTransferIntegerCases() {
    return new MalformedTransferIntegerCase[] {
      new MalformedTransferIntegerCase(
          transferListBody("\"1\"", "1", "1", "1"),
          "total must be a JSON integer number"),
      new MalformedTransferIntegerCase(
          transferListBody("\"\"", "1", "1", "1"),
          "total must be a JSON integer number"),
      new MalformedTransferIntegerCase(
          transferListBody("1.5", "1", "1", "1"),
          "total must be an integer"),
      new MalformedTransferIntegerCase(
          transferListBody("9223372036854775808", "1", "1", "1"),
          "total must fit in signed 64-bit range"),
      new MalformedTransferIntegerCase(
          transferListBody("1", "\"1\"", "1", "1"),
          "items[0].receipt_count must be a JSON integer number"),
      new MalformedTransferIntegerCase(
          transferListBody("1", "1", "1.5", "1"),
          "items[0].recorded_at_ms must be an integer"),
      new MalformedTransferIntegerCase(
          transferListBody("1", "1", "1", "\"\""),
          "items[0].recorded_at_height must be a JSON integer number")
    };
  }

  private static String transferListBody(
      final String total,
      final String receiptCount,
      final String recordedAtMs,
      final String recordedAtHeight) {
    return String.format(
        Locale.ROOT,
        """
        {
          "items": [
            {
              "receipt_count": %s,
              "recorded_at_ms": %s,
              "recorded_at_height": %s
            }
          ],
          "total": %s
        }
        """,
        receiptCount,
        recordedAtMs,
        recordedAtHeight,
        total);
  }

  private static String readinessBody(
      final String extra,
      final String assetDefinitionId,
      final String evaluatedBlockHeight,
      final String ready,
      final String blockers) {
    return String.format(
        Locale.ROOT,
        """
        {
          %s
          "asset_definition_id": %s,
          "evaluated_block_height": %s,
          "evaluated_block_hash": "abababababababababababababababababababababababababababababababab",
          "ready": %s,
          "blockers": %s
        }
        """,
        extra,
        assetDefinitionId,
        evaluatedBlockHeight,
        ready,
        blockers);
  }

  private static int findSubsequence(final byte[] haystack, final byte[] needle) {
    outer:
    for (int offset = 0; offset <= haystack.length - needle.length; offset++) {
      for (int index = 0; index < needle.length; index++) {
        if (haystack[offset + index] != needle[index]) {
          continue outer;
        }
      }
      return offset;
    }
    return -1;
  }

  private static void expectIllegalState(
      final Runnable action, final String expectedMessageFragment) {
    try {
      action.run();
    } catch (final IllegalStateException | IllegalArgumentException ex) {
      assert ex.getMessage().contains(expectedMessageFragment)
          : "unexpected message: " + ex.getMessage();
      return;
    }
    throw new AssertionError("Expected IllegalStateException");
  }
}

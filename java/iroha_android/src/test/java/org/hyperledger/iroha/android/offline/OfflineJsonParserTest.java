package org.hyperledger.iroha.android.offline;

import java.math.BigInteger;
import java.nio.charset.StandardCharsets;
import java.util.Locale;

public final class OfflineJsonParserTest {

  private OfflineJsonParserTest() {}

  public static void main(final String[] args) {
    parsesOfflineReadiness();
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
    assert !readiness.ready();
    assert readiness.blockers().size() == 1;
    assert "offline_disabled".equals(readiness.blockers().get(0).code());
    assert "Offline transfers are disabled".equals(readiness.blockers().get(0).message());
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
          readinessBody("\"offline_telemetry\": true,", "\"xor#wonderland\"", "7", "true", "[]"),
          "root.offline_telemetry is not a supported field"),
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
              "[{\"code\":\"blocked\",\"message\":\"no\",\"extra\":1}]"),
          "blockers[0].extra is not a supported field")
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

  private static void expectIllegalState(
      final Runnable action, final String expectedMessageFragment) {
    try {
      action.run();
    } catch (final IllegalStateException ex) {
      assert ex.getMessage().contains(expectedMessageFragment)
          : "unexpected message: " + ex.getMessage();
      return;
    }
    throw new AssertionError("Expected IllegalStateException");
  }
}

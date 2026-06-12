package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;

public final class OfflineJsonParserTest {

  private OfflineJsonParserTest() {}

  public static void main(final String[] args) {
    parsesOfflineReadiness();
    parsesOfflineReadinessShortAbi7Aliases();
    parsesOfflineReadinessPrefersShortAbi7Aliases();
    parsesOfflineV2ReadinessShortAbi7Aliases();
    parsesOfflineTransfers();
    canonicalizesJson();
    System.out.println("[IrohaAndroid] OfflineJsonParserTest passed.");
  }

  private static void parsesOfflineReadiness() {
    final String json =
        """
        {
          "offline_telemetry": true,
          "offline_kagemusha_recursive_compact_available": true,
          "offline_kagemusha_recursive_compact_mode": "recursive_compact_v1",
          "offline_kagemusha_recursive_compact_required_native_bridge_abi_version": 7,
          "offline_kagemusha_recursive_compact_circuit_id": "kagemusha-recursive-compact-v1",
          "offline_kagemusha_recursive_compact_artifacts_available": true
        }
        """;
    final OfflineReadiness readiness =
        OfflineJsonParser.parseOfflineReadiness(json.getBytes(StandardCharsets.UTF_8));
    assert !readiness.offlineNote();
    assert !readiness.offlineOneUseKeys();
    assert !readiness.offlineRecursiveNoteProof();
    assert !readiness.offlineFountainQr();
    assert !readiness.offlineSyncOptional();
    assert readiness.offlineTelemetry();
    assert readiness.offlineKagemushaRecursiveCompactAvailable();
    assert "recursive_compact_v1".equals(readiness.offlineKagemushaRecursiveCompactMode());
    assert Integer.valueOf(7).equals(readiness.offlineKagemushaRecursiveCompactRequiredNativeBridgeAbiVersion());
    assert "kagemusha-recursive-compact-v1".equals(readiness.offlineKagemushaRecursiveCompactCircuitId());
    assert readiness.offlineKagemushaRecursiveCompactArtifactsAvailable();
  }

  private static void parsesOfflineReadinessShortAbi7Aliases() {
    final String json =
        """
        {
          "offline_telemetry": true,
          "offline_kagemusha_abi7": true,
          "offline_kagemusha_abi7_mode": "recursive_compact_v1",
          "offline_kagemusha_abi7_bridge_abi_version": "7",
          "offline_kagemusha_abi7_circuit_id": "kagemusha-recursive-compact-v1",
          "offline_kagemusha_abi7_artifacts": true
        }
        """;
    final OfflineReadiness readiness =
        OfflineJsonParser.parseOfflineReadiness(json.getBytes(StandardCharsets.UTF_8));
    assert readiness.offlineKagemushaRecursiveCompactAvailable();
    assert "recursive_compact_v1".equals(readiness.offlineKagemushaRecursiveCompactMode());
    assert Integer.valueOf(7).equals(
        readiness.offlineKagemushaRecursiveCompactRequiredNativeBridgeAbiVersion());
    assert "kagemusha-recursive-compact-v1".equals(
        readiness.offlineKagemushaRecursiveCompactCircuitId());
    assert readiness.offlineKagemushaRecursiveCompactArtifactsAvailable();
  }

  private static void parsesOfflineReadinessPrefersShortAbi7Aliases() {
    final String json =
        """
        {
          "offline_telemetry": true,
          "offline_kagemusha_abi7": false,
          "offline_kagemusha_recursive_compact_available": true,
          "offline_kagemusha_abi7_mode": "short-mode",
          "offline_kagemusha_recursive_compact_mode": "verbose-mode",
          "offline_kagemusha_abi7_bridge_abi_version": 7,
          "offline_kagemusha_recursive_compact_required_native_bridge_abi_version": 99,
          "offline_kagemusha_abi7_circuit_id": "short-circuit",
          "offline_kagemusha_recursive_compact_circuit_id": "verbose-circuit",
          "offline_kagemusha_abi7_artifacts": false,
          "offline_kagemusha_recursive_compact_artifacts_available": true
        }
        """;
    final OfflineReadiness readiness =
        OfflineJsonParser.parseOfflineReadiness(json.getBytes(StandardCharsets.UTF_8));
    assert !readiness.offlineKagemushaRecursiveCompactAvailable();
    assert "short-mode".equals(readiness.offlineKagemushaRecursiveCompactMode());
    assert Integer.valueOf(7).equals(
        readiness.offlineKagemushaRecursiveCompactRequiredNativeBridgeAbiVersion());
    assert "short-circuit".equals(readiness.offlineKagemushaRecursiveCompactCircuitId());
    assert !readiness.offlineKagemushaRecursiveCompactArtifactsAvailable();
  }

  private static void parsesOfflineV2ReadinessShortAbi7Aliases() {
    final String json =
        """
        {
          "offline_telemetry": true,
          "offline_kagemusha_abi7": true,
          "offline_kagemusha_abi7_mode": "recursive_compact_v1",
          "offline_kagemusha_abi7_bridge_abi_version": 7,
          "offline_kagemusha_abi7_circuit_id": "kagemusha-recursive-compact-v1",
          "offline_kagemusha_abi7_artifacts": true
        }
        """;
    final OfflineV2Readiness readiness =
        OfflineJsonParser.parseOfflineV2Readiness(json.getBytes(StandardCharsets.UTF_8));
    assert readiness.offlineKagemushaRecursiveCompactAvailable();
    assert "recursive_compact_v1".equals(readiness.offlineKagemushaRecursiveCompactMode());
    assert Integer.valueOf(7).equals(
        readiness.offlineKagemushaRecursiveCompactRequiredNativeBridgeAbiVersion());
    assert "kagemusha-recursive-compact-v1".equals(
        readiness.offlineKagemushaRecursiveCompactCircuitId());
    assert readiness.offlineKagemushaRecursiveCompactArtifactsAvailable();
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

  private static void canonicalizesJson() {
    final String encoded =
        OfflineJsonParser.canonicalJson("{\"b\":2,\"a\":1}".getBytes(StandardCharsets.UTF_8));
    assert "{\"a\":1,\"b\":2}".equals(encoded) : "canonical JSON mismatch: " + encoded;
  }
}

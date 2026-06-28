package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.Locale;

public final class OfflineJsonParserTest {

  private OfflineJsonParserTest() {}

  public static void main(final String[] args) {
    parsesOfflineReadiness();
    rejectsOfflineReadinessRemovedAbi7Aliases();
    rejectsOfflineReadinessMalformedCanonicalValues();
    parsesOfflineV2Readiness();
    rejectsOfflineV2ReadinessRemovedAbi7Aliases();
    rejectsOfflineV2ReadinessMalformedCanonicalValues();
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
          "offline_kagemusha_recursive_compact_artifacts_available": false
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
    assert !readiness.offlineKagemushaRecursiveCompactArtifactsAvailable();
  }

  private static void rejectsOfflineReadinessRemovedAbi7Aliases() {
    for (final RemovedAbi7ReadinessCase item : REMOVED_ABI7_READINESS_CASES) {
      expectIllegalState(
          () -> parseOfflineReadiness(canonicalReadinessBody("\"" + item.field + "\": true,")),
          item.message);
    }
  }

  private static void rejectsOfflineReadinessMalformedCanonicalValues() {
    for (final MalformedReadinessCase item : malformedCanonicalReadinessCases()) {
      expectIllegalState(() -> parseOfflineReadiness(item.body), item.message);
    }
  }

  private static void parsesOfflineV2Readiness() {
    final String json =
        """
        {
          "offline_telemetry": true,
          "offline_kagemusha_recursive_compact_available": true,
          "offline_kagemusha_recursive_compact_mode": "recursive_compact_v1",
          "offline_kagemusha_recursive_compact_required_native_bridge_abi_version": 7,
          "offline_kagemusha_recursive_compact_circuit_id": "kagemusha-recursive-compact-v1",
          "offline_kagemusha_recursive_compact_artifacts_available": false
        }
        """;
    final OfflineV2Readiness readiness =
        OfflineJsonParser.parseOfflineV2Readiness(json.getBytes(StandardCharsets.UTF_8));
    assert readiness.offlineTelemetry();
    assert readiness.offlineKagemushaRecursiveCompactAvailable();
    assert "recursive_compact_v1".equals(readiness.offlineKagemushaRecursiveCompactMode());
    assert Integer.valueOf(7).equals(
        readiness.offlineKagemushaRecursiveCompactRequiredNativeBridgeAbiVersion());
    assert "kagemusha-recursive-compact-v1".equals(
        readiness.offlineKagemushaRecursiveCompactCircuitId());
    assert !readiness.offlineKagemushaRecursiveCompactArtifactsAvailable();
  }

  private static void rejectsOfflineV2ReadinessRemovedAbi7Aliases() {
    for (final RemovedAbi7ReadinessCase item : REMOVED_ABI7_READINESS_CASES) {
      expectIllegalState(
          () -> parseOfflineV2Readiness(canonicalReadinessBody("\"" + item.field + "\": true,")),
          item.message);
    }
  }

  private static void rejectsOfflineV2ReadinessMalformedCanonicalValues() {
    for (final MalformedReadinessCase item : malformedCanonicalReadinessCases()) {
      expectIllegalState(() -> parseOfflineV2Readiness(item.body), item.message);
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

  private static void canonicalizesJson() {
    final String encoded =
        OfflineJsonParser.canonicalJson("{\"b\":2,\"a\":1}".getBytes(StandardCharsets.UTF_8));
    assert "{\"a\":1,\"b\":2}".equals(encoded) : "canonical JSON mismatch: " + encoded;
  }

  private static OfflineReadiness parseOfflineReadiness(final String json) {
    return OfflineJsonParser.parseOfflineReadiness(json.getBytes(StandardCharsets.UTF_8));
  }

  private static OfflineV2Readiness parseOfflineV2Readiness(final String json) {
    return OfflineJsonParser.parseOfflineV2Readiness(json.getBytes(StandardCharsets.UTF_8));
  }

  private record RemovedAbi7ReadinessCase(String field, String message) {}

  private static final RemovedAbi7ReadinessCase[] REMOVED_ABI7_READINESS_CASES = {
    new RemovedAbi7ReadinessCase(
        "offline_kagemusha_abi7",
        "offline_kagemusha_abi7 is not supported; use offline_kagemusha_recursive_compact_*"),
    new RemovedAbi7ReadinessCase(
        "offline_kagemusha_abi7_mode",
        "offline_kagemusha_abi7_mode is not supported; use offline_kagemusha_recursive_compact_*"),
    new RemovedAbi7ReadinessCase(
        "offline_kagemusha_abi7_bridge_abi_version",
        "offline_kagemusha_abi7_bridge_abi_version is not supported; use offline_kagemusha_recursive_compact_*"),
    new RemovedAbi7ReadinessCase(
        "offline_kagemusha_abi7_circuit_id",
        "offline_kagemusha_abi7_circuit_id is not supported; use offline_kagemusha_recursive_compact_*"),
    new RemovedAbi7ReadinessCase(
        "offline_kagemusha_abi7_artifacts",
        "offline_kagemusha_abi7_artifacts is not supported; use offline_kagemusha_recursive_compact_*")
  };

  private record MalformedReadinessCase(String body, String message) {}

  private static MalformedReadinessCase[] malformedCanonicalReadinessCases() {
    return new MalformedReadinessCase[] {
      new MalformedReadinessCase(
          canonicalReadinessBody("", "\"true\"", "\"recursive_compact_v1\"", "7",
              "\"kagemusha-recursive-compact-v1\"", "false"),
          "offline_kagemusha_recursive_compact_available must be a boolean"),
      new MalformedReadinessCase(
          canonicalReadinessBody("", "true", "\" recursive_compact_v1\"", "7",
              "\"kagemusha-recursive-compact-v1\"", "false"),
          "offline_kagemusha_recursive_compact_mode must be an exact non-empty string"),
      new MalformedReadinessCase(
          canonicalReadinessBody("", "true", "\"recursive_compact_v1\"", "\"007\"",
              "\"kagemusha-recursive-compact-v1\"", "false"),
          "offline_kagemusha_recursive_compact_required_native_bridge_abi_version must be an exact integer string"),
      new MalformedReadinessCase(
          canonicalReadinessBody("", "true", "\"recursive_compact_v1\"", "-1",
              "\"kagemusha-recursive-compact-v1\"", "false"),
          "offline_kagemusha_recursive_compact_required_native_bridge_abi_version must be a positive integer"),
      new MalformedReadinessCase(
          canonicalReadinessBody("", "true", "\"recursive_compact_v1\"", "7.5",
              "\"kagemusha-recursive-compact-v1\"", "false"),
          "offline_kagemusha_recursive_compact_required_native_bridge_abi_version must be an integer"),
      new MalformedReadinessCase(
          canonicalReadinessBody("", "true", "\"recursive_compact_v1\"", "2147483648",
              "\"kagemusha-recursive-compact-v1\"", "false"),
          "offline_kagemusha_recursive_compact_required_native_bridge_abi_version must fit in signed 32-bit range"),
      new MalformedReadinessCase(
          canonicalReadinessBody("", "true", "\"recursive_compact_v1\"", "7",
              "\"\"", "false"),
          "offline_kagemusha_recursive_compact_circuit_id must be an exact non-empty string"),
      new MalformedReadinessCase(
          canonicalReadinessBody("", "true", "\"recursive_compact_v1\"", "7",
              "\"kagemusha-recursive-compact-v1\"", "\"true\""),
          "offline_kagemusha_recursive_compact_artifacts_available must be a boolean")
    };
  }

  private static String canonicalReadinessBody(final String extra) {
    return canonicalReadinessBody(
        extra,
        "true",
        "\"recursive_compact_v1\"",
        "7",
        "\"kagemusha-recursive-compact-v1\"",
        "false");
  }

  private static String canonicalReadinessBody(
      final String extra,
      final String compactAvailable,
      final String compactMode,
      final String compactBridge,
      final String compactCircuit,
      final String compactArtifacts) {
    return String.format(
        Locale.ROOT,
        """
        {
          "offline_telemetry": true,
          %s
          "offline_kagemusha_recursive_compact_available": %s,
          "offline_kagemusha_recursive_compact_mode": %s,
          "offline_kagemusha_recursive_compact_required_native_bridge_abi_version": %s,
          "offline_kagemusha_recursive_compact_circuit_id": %s,
          "offline_kagemusha_recursive_compact_artifacts_available": %s
        }
        """,
            extra,
            compactAvailable,
            compactMode,
            compactBridge,
            compactCircuit,
            compactArtifacts);
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

package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.Locale;

public final class OfflineJsonParserTest {

  private OfflineJsonParserTest() {}

  public static void main(final String[] args) {
    parsesOfflineReadiness();
    parsesOfflineReadinessShortAbi7Aliases();
    parsesOfflineReadinessMatchingAbi7Aliases();
    rejectsOfflineReadinessConflictingAbi7Aliases();
    rejectsOfflineReadinessMalformedAbi7Aliases();
    parsesOfflineV2ReadinessShortAbi7Aliases();
    rejectsOfflineV2ReadinessConflictingAbi7Aliases();
    rejectsOfflineV2ReadinessMalformedAbi7Aliases();
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

  private static void parsesOfflineReadinessMatchingAbi7Aliases() {
    final String json =
        """
        {
          "offline_telemetry": true,
          "offline_kagemusha_abi7": true,
          "offline_kagemusha_recursive_compact_available": true,
          "offline_kagemusha_abi7_mode": "recursive_compact_v1",
          "offline_kagemusha_recursive_compact_mode": "recursive_compact_v1",
          "offline_kagemusha_abi7_bridge_abi_version": 7,
          "offline_kagemusha_recursive_compact_required_native_bridge_abi_version": 7,
          "offline_kagemusha_abi7_circuit_id": "kagemusha-recursive-compact-v1",
          "offline_kagemusha_recursive_compact_circuit_id": "kagemusha-recursive-compact-v1",
          "offline_kagemusha_abi7_artifacts": true,
          "offline_kagemusha_recursive_compact_artifacts_available": true
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

  private static void rejectsOfflineReadinessConflictingAbi7Aliases() {
    expectIllegalState(
        () -> parseOfflineReadiness(aliasReadinessBody("false")),
        "offline_kagemusha_abi7 and offline_kagemusha_recursive_compact_available must match");
    expectIllegalState(
        () -> parseOfflineReadiness(aliasReadinessBody(
            "true",
            "true",
            "\"legacy-mode\"")),
        "offline_kagemusha_abi7_mode and offline_kagemusha_recursive_compact_mode must match");
    expectIllegalState(
        () -> parseOfflineReadiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "8")),
        "offline_kagemusha_abi7_bridge_abi_version and offline_kagemusha_recursive_compact_required_native_bridge_abi_version must match");
    expectIllegalState(
        () -> parseOfflineReadiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "7",
            "7",
            "\"legacy-circuit\"")),
        "offline_kagemusha_abi7_circuit_id and offline_kagemusha_recursive_compact_circuit_id must match");
    expectIllegalState(
        () -> parseOfflineReadiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "7",
            "7",
            "\"kagemusha-recursive-compact-v1\"",
            "\"kagemusha-recursive-compact-v1\"",
            "false")),
        "offline_kagemusha_abi7_artifacts and offline_kagemusha_recursive_compact_artifacts_available must match");
  }

  private static void rejectsOfflineReadinessMalformedAbi7Aliases() {
    expectIllegalState(
        () -> parseOfflineReadiness(aliasReadinessBody("\"true\"")),
        "offline_kagemusha_abi7 must be a boolean");
    expectIllegalState(
        () -> parseOfflineReadiness(aliasReadinessBody("true", "true", "{}")),
        "offline_kagemusha_abi7_mode must be a string");
    expectIllegalState(
        () -> parseOfflineReadiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\" recursive_compact_v1\"")),
        "offline_kagemusha_recursive_compact_mode must be an exact non-empty string");
    expectIllegalState(
        () -> parseOfflineReadiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "\" 7\"")),
        "offline_kagemusha_abi7_bridge_abi_version must be an exact integer string");
    expectIllegalState(
        () -> parseOfflineReadiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "\"+7\"")),
        "offline_kagemusha_abi7_bridge_abi_version must be an exact integer string");
    expectIllegalState(
        () -> parseOfflineReadiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "7",
            "\"007\"")),
        "offline_kagemusha_recursive_compact_required_native_bridge_abi_version must be an exact integer string");
    expectIllegalState(
        () -> parseOfflineReadiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "-1",
            "-1")),
        "offline_kagemusha_abi7_bridge_abi_version must be a positive integer");
    expectIllegalState(
        () -> parseOfflineReadiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "\"2147483648\"",
            "\"2147483648\"")),
        "offline_kagemusha_abi7_bridge_abi_version must fit in signed 32-bit range");
    expectIllegalState(
        () -> parseOfflineReadiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "7",
            "7.5")),
        "offline_kagemusha_recursive_compact_required_native_bridge_abi_version must be an integer");
    expectIllegalState(
        () -> parseOfflineReadiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "7",
            "0")),
        "offline_kagemusha_recursive_compact_required_native_bridge_abi_version must be a positive integer");
    expectIllegalState(
        () -> parseOfflineReadiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "7",
            "2147483648")),
        "offline_kagemusha_recursive_compact_required_native_bridge_abi_version must fit in signed 32-bit range");
    expectIllegalState(
        () -> parseOfflineReadiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "7",
            "7",
            "\"kagemusha-recursive-compact-v1\"",
            "\"\"")),
        "offline_kagemusha_recursive_compact_circuit_id must be an exact non-empty string");
    expectIllegalState(
        () -> parseOfflineReadiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "7",
            "7",
            "7",
            "7")),
        "offline_kagemusha_abi7_circuit_id must be a string");
    expectIllegalState(
        () -> parseOfflineReadiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "7",
            "7",
            "\"kagemusha-recursive-compact-v1\"",
            "\"kagemusha-recursive-compact-v1\"",
            "\"true\"")),
        "offline_kagemusha_abi7_artifacts must be a boolean");
    expectIllegalState(
        () -> parseOfflineReadiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "7",
            "7",
            "\"kagemusha-recursive-compact-v1\"",
            "\"kagemusha-recursive-compact-v1\"",
            "true",
            "\"true\"")),
        "offline_kagemusha_recursive_compact_artifacts_available must be a boolean");
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

  private static void rejectsOfflineV2ReadinessConflictingAbi7Aliases() {
    expectIllegalState(
        () -> parseOfflineV2Readiness(aliasReadinessBody("false")),
        "offline_kagemusha_abi7 and offline_kagemusha_recursive_compact_available must match");
    expectIllegalState(
        () -> parseOfflineV2Readiness(aliasReadinessBody(
            "true",
            "true",
            "\"legacy-mode\"")),
        "offline_kagemusha_abi7_mode and offline_kagemusha_recursive_compact_mode must match");
    expectIllegalState(
        () -> parseOfflineV2Readiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "8")),
        "offline_kagemusha_abi7_bridge_abi_version and offline_kagemusha_recursive_compact_required_native_bridge_abi_version must match");
    expectIllegalState(
        () -> parseOfflineV2Readiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "7",
            "7",
            "\"legacy-circuit\"")),
        "offline_kagemusha_abi7_circuit_id and offline_kagemusha_recursive_compact_circuit_id must match");
    expectIllegalState(
        () -> parseOfflineV2Readiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "7",
            "7",
            "\"kagemusha-recursive-compact-v1\"",
            "\"kagemusha-recursive-compact-v1\"",
            "false")),
        "offline_kagemusha_abi7_artifacts and offline_kagemusha_recursive_compact_artifacts_available must match");
  }

  private static void rejectsOfflineV2ReadinessMalformedAbi7Aliases() {
    expectIllegalState(
        () -> parseOfflineV2Readiness(aliasReadinessBody("\"true\"")),
        "offline_kagemusha_abi7 must be a boolean");
    expectIllegalState(
        () -> parseOfflineV2Readiness(aliasReadinessBody("true", "true", "{}")),
        "offline_kagemusha_abi7_mode must be a string");
    expectIllegalState(
        () -> parseOfflineV2Readiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\" recursive_compact_v1\"")),
        "offline_kagemusha_recursive_compact_mode must be an exact non-empty string");
    expectIllegalState(
        () -> parseOfflineV2Readiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "\" 7\"")),
        "offline_kagemusha_abi7_bridge_abi_version must be an exact integer string");
    expectIllegalState(
        () -> parseOfflineV2Readiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "\"+7\"")),
        "offline_kagemusha_abi7_bridge_abi_version must be an exact integer string");
    expectIllegalState(
        () -> parseOfflineV2Readiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "7",
            "\"007\"")),
        "offline_kagemusha_recursive_compact_required_native_bridge_abi_version must be an exact integer string");
    expectIllegalState(
        () -> parseOfflineV2Readiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "-1",
            "-1")),
        "offline_kagemusha_abi7_bridge_abi_version must be a positive integer");
    expectIllegalState(
        () -> parseOfflineV2Readiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "\"2147483648\"",
            "\"2147483648\"")),
        "offline_kagemusha_abi7_bridge_abi_version must fit in signed 32-bit range");
    expectIllegalState(
        () -> parseOfflineV2Readiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "7",
            "7.5")),
        "offline_kagemusha_recursive_compact_required_native_bridge_abi_version must be an integer");
    expectIllegalState(
        () -> parseOfflineV2Readiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "7",
            "0")),
        "offline_kagemusha_recursive_compact_required_native_bridge_abi_version must be a positive integer");
    expectIllegalState(
        () -> parseOfflineV2Readiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "7",
            "2147483648")),
        "offline_kagemusha_recursive_compact_required_native_bridge_abi_version must fit in signed 32-bit range");
    expectIllegalState(
        () -> parseOfflineV2Readiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "7",
            "7",
            "\"kagemusha-recursive-compact-v1\"",
            "\"\"")),
        "offline_kagemusha_recursive_compact_circuit_id must be an exact non-empty string");
    expectIllegalState(
        () -> parseOfflineV2Readiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "7",
            "7",
            "7",
            "7")),
        "offline_kagemusha_abi7_circuit_id must be a string");
    expectIllegalState(
        () -> parseOfflineV2Readiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "7",
            "7",
            "\"kagemusha-recursive-compact-v1\"",
            "\"kagemusha-recursive-compact-v1\"",
            "\"true\"")),
        "offline_kagemusha_abi7_artifacts must be a boolean");
    expectIllegalState(
        () -> parseOfflineV2Readiness(aliasReadinessBody(
            "true",
            "true",
            "\"recursive_compact_v1\"",
            "\"recursive_compact_v1\"",
            "7",
            "7",
            "\"kagemusha-recursive-compact-v1\"",
            "\"kagemusha-recursive-compact-v1\"",
            "true",
            "\"true\"")),
        "offline_kagemusha_recursive_compact_artifacts_available must be a boolean");
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

  private static String aliasReadinessBody(final String abi7) {
    return aliasReadinessBody(
        abi7,
        "true",
        "\"recursive_compact_v1\"",
        "\"recursive_compact_v1\"",
        "7",
        "7",
        "\"kagemusha-recursive-compact-v1\"",
        "\"kagemusha-recursive-compact-v1\"",
        "true",
        "true");
  }

  private static String aliasReadinessBody(
      final String abi7, final String compactAvailable, final String abi7Mode) {
    return aliasReadinessBody(
        abi7,
        compactAvailable,
        abi7Mode,
        "\"recursive_compact_v1\"",
        "7",
        "7",
        "\"kagemusha-recursive-compact-v1\"",
        "\"kagemusha-recursive-compact-v1\"",
        "true",
        "true");
  }

  private static String aliasReadinessBody(
      final String abi7,
      final String compactAvailable,
      final String abi7Mode,
      final String compactMode) {
    return aliasReadinessBody(
        abi7,
        compactAvailable,
        abi7Mode,
        compactMode,
        "7",
        "7",
        "\"kagemusha-recursive-compact-v1\"",
        "\"kagemusha-recursive-compact-v1\"",
        "true",
        "true");
  }

  private static String aliasReadinessBody(
      final String abi7,
      final String compactAvailable,
      final String abi7Mode,
      final String compactMode,
      final String abi7Bridge) {
    return aliasReadinessBody(
        abi7,
        compactAvailable,
        abi7Mode,
        compactMode,
        abi7Bridge,
        "7",
        "\"kagemusha-recursive-compact-v1\"",
        "\"kagemusha-recursive-compact-v1\"",
        "true",
        "true");
  }

  private static String aliasReadinessBody(
      final String abi7,
      final String compactAvailable,
      final String abi7Mode,
      final String compactMode,
      final String abi7Bridge,
      final String compactBridge) {
    return aliasReadinessBody(
        abi7,
        compactAvailable,
        abi7Mode,
        compactMode,
        abi7Bridge,
        compactBridge,
        "\"kagemusha-recursive-compact-v1\"",
        "\"kagemusha-recursive-compact-v1\"",
        "true",
        "true");
  }

  private static String aliasReadinessBody(
      final String abi7,
      final String compactAvailable,
      final String abi7Mode,
      final String compactMode,
      final String abi7Bridge,
      final String compactBridge,
      final String abi7Circuit) {
    return aliasReadinessBody(
        abi7,
        compactAvailable,
        abi7Mode,
        compactMode,
        abi7Bridge,
        compactBridge,
        abi7Circuit,
        "\"kagemusha-recursive-compact-v1\"",
        "true",
        "true");
  }

  private static String aliasReadinessBody(
      final String abi7,
      final String compactAvailable,
      final String abi7Mode,
      final String compactMode,
      final String abi7Bridge,
      final String compactBridge,
      final String abi7Circuit,
      final String compactCircuit) {
    return aliasReadinessBody(
        abi7,
        compactAvailable,
        abi7Mode,
        compactMode,
        abi7Bridge,
        compactBridge,
        abi7Circuit,
        compactCircuit,
        "true",
        "true");
  }

  private static String aliasReadinessBody(
      final String abi7,
      final String compactAvailable,
      final String abi7Mode,
      final String compactMode,
      final String abi7Bridge,
      final String compactBridge,
      final String abi7Circuit,
      final String compactCircuit,
      final String abi7Artifacts) {
    return aliasReadinessBody(
        abi7,
        compactAvailable,
        abi7Mode,
        compactMode,
        abi7Bridge,
        compactBridge,
        abi7Circuit,
        compactCircuit,
        abi7Artifacts,
        "true");
  }

  private static String aliasReadinessBody(
      final String abi7,
      final String compactAvailable,
      final String abi7Mode,
      final String compactMode,
      final String abi7Bridge,
      final String compactBridge,
      final String abi7Circuit,
      final String compactCircuit,
      final String abi7Artifacts,
      final String compactArtifacts) {
    return String.format(
        Locale.ROOT,
        """
        {
          "offline_telemetry": true,
          "offline_kagemusha_abi7": %s,
          "offline_kagemusha_recursive_compact_available": %s,
          "offline_kagemusha_abi7_mode": %s,
          "offline_kagemusha_recursive_compact_mode": %s,
          "offline_kagemusha_abi7_bridge_abi_version": %s,
          "offline_kagemusha_recursive_compact_required_native_bridge_abi_version": %s,
          "offline_kagemusha_abi7_circuit_id": %s,
          "offline_kagemusha_recursive_compact_circuit_id": %s,
          "offline_kagemusha_abi7_artifacts": %s,
          "offline_kagemusha_recursive_compact_artifacts_available": %s
        }
        """,
            abi7,
            compactAvailable,
            abi7Mode,
            compactMode,
            abi7Bridge,
            compactBridge,
            abi7Circuit,
            compactCircuit,
            abi7Artifacts,
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

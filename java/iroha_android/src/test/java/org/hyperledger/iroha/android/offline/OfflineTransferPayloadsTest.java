package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.LinkedHashMap;
import java.util.Map;
import org.hyperledger.iroha.android.client.JsonParser;

/** Unit tests for {@link OfflineTransferPayloads}. */
public final class OfflineTransferPayloadsTest {

  private OfflineTransferPayloadsTest() {}

  public static void main(final String[] args) {
    attachesSnapshotToMap();
    injectsSnapshotIntoJson();
    System.out.println("[IrohaAndroid] OfflineTransferPayloadsTest passed.");
  }

  private static void attachesSnapshotToMap() {
    final Map<String, Object> transfer = new LinkedHashMap<>();
    transfer.put("bundle_id", "cafebabe");
    final OfflineWallet.PlatformTokenSnapshot snapshot =
        new OfflineWallet.PlatformTokenSnapshot("hms_safety_detect", "deadbeef");
    final Map<String, Object> mutated =
        OfflineTransferPayloads.attachPlatformSnapshot(transfer, snapshot);
    assert !transfer.containsKey("platform_snapshot") : "original transfer map should remain untouched";
    assert mutated.containsKey("platform_snapshot") : "snapshot should be attached";
    final Object node = mutated.get("platform_snapshot");
    assert node instanceof Map<?, ?> : "snapshot node should be a map";
    final Map<?, ?> snapshotMap = (Map<?, ?>) node;
    assert "hms_safety_detect".equals(snapshotMap.get("policy")) : "policy mismatch";
    assert "deadbeef".equals(snapshotMap.get("attestation_jws_b64")) : "attestation mismatch";
  }

  private static void injectsSnapshotIntoJson() {
    final String json =
        """
        {"transfer":{"bundle_id":"feedface"}}
        """;
    final OfflineWallet.PlatformTokenSnapshot snapshot =
        new OfflineWallet.PlatformTokenSnapshot("play_integrity", "payload");
    final byte[] encoded =
        OfflineTransferPayloads.attachPlatformSnapshot(json.getBytes(StandardCharsets.UTF_8), snapshot);
    final Object parsed = JsonParser.parse(new String(encoded, StandardCharsets.UTF_8));
    assert parsed instanceof Map<?, ?> : "root should be a JSON object";
    final Map<?, ?> root = (Map<?, ?>) parsed;
    final Object transferNode = root.get("transfer");
    assert transferNode instanceof Map<?, ?> : "transfer node missing";
    final Map<?, ?> transfer = (Map<?, ?>) transferNode;
    final Object snapshotNode = transfer.get("platform_snapshot");
    assert snapshotNode instanceof Map<?, ?> : "snapshot node missing";
    final Map<?, ?> snapshotMap = (Map<?, ?>) snapshotNode;
    assert "play_integrity".equals(snapshotMap.get("policy")) : "policy mismatch";
    assert "payload".equals(snapshotMap.get("attestation_jws_b64")) : "attestation mismatch";
  }
}

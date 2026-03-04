package org.hyperledger.iroha.android.offline;

import java.nio.charset.StandardCharsets;
import java.util.LinkedHashMap;
import java.util.Map;
import java.util.Objects;
import org.hyperledger.iroha.android.client.JsonEncoder;
import org.hyperledger.iroha.android.client.JsonParser;

/** Utility helpers for editing {@code OfflineToOnlineTransfer} payloads prior to submission. */
public final class OfflineTransferPayloads {

  private static final String TRANSFER_FIELD = "transfer";
  private static final String PLATFORM_SNAPSHOT_FIELD = "platform_snapshot";

  private OfflineTransferPayloads() {}

  /**
   * Returns a shallow copy of {@code transferPayload} with {@code platform_snapshot} populated.
   *
   * <p>The helper does not mutate the original map so callers can keep their cached payloads and
   * only attach the snapshot when required.
   */
  public static Map<String, Object> attachPlatformSnapshot(
      final Map<String, Object> transferPayload,
      final OfflineWallet.PlatformTokenSnapshot snapshot) {
    Objects.requireNonNull(transferPayload, "transferPayload");
    Objects.requireNonNull(snapshot, "snapshot");
    final Map<String, Object> copy = new LinkedHashMap<>(transferPayload);
    copy.put(PLATFORM_SNAPSHOT_FIELD, snapshot.toJsonMap());
    return copy;
  }

  /**
   * Injects the snapshot under {@code transfer.platform_snapshot} within a JSON instruction payload.
   *
   * @param submitTransferJson canonical JSON for a {@code SubmitOfflineToOnlineTransfer} instruction
   * @param snapshot snapshot to attach
   * @return UTF-8 encoded JSON with the snapshot injected
   * @throws IllegalArgumentException if the JSON payload is not an object or lacks a transfer node
   */
  @SuppressWarnings("unchecked")
  public static byte[] attachPlatformSnapshot(
      final byte[] submitTransferJson,
      final OfflineWallet.PlatformTokenSnapshot snapshot) {
    Objects.requireNonNull(submitTransferJson, "submitTransferJson");
    Objects.requireNonNull(snapshot, "snapshot");

    final String json = new String(submitTransferJson, StandardCharsets.UTF_8);
    final Object parsed = JsonParser.parse(json);
    if (!(parsed instanceof Map<?, ?> rootNode)) {
      throw new IllegalArgumentException("SubmitOfflineToOnlineTransfer payload must be a JSON object");
    }
    final Map<String, Object> root = new LinkedHashMap<>((Map<String, Object>) rootNode);
    final Object transferNode = root.get(TRANSFER_FIELD);
    if (!(transferNode instanceof Map<?, ?> existingTransfer)) {
      throw new IllegalArgumentException("SubmitOfflineToOnlineTransfer payload missing 'transfer'");
    }
    final Map<String, Object> transfer =
        attachPlatformSnapshot(new LinkedHashMap<>((Map<String, Object>) existingTransfer), snapshot);
    root.put(TRANSFER_FIELD, transfer);
    final String encoded = JsonEncoder.encode(root);
    return encoded.getBytes(StandardCharsets.UTF_8);
  }
}

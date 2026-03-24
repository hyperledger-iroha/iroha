package org.hyperledger.iroha.sdk.offline

import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.client.JsonParser

/** Utility helpers for editing `OfflineToOnlineTransfer` payloads prior to submission. */
object OfflineTransferPayloads {
    private const val TRANSFER_FIELD = "transfer"
    private const val PLATFORM_SNAPSHOT_FIELD = "platform_snapshot"

    /**
     * Returns a shallow copy of [transferPayload] with `platform_snapshot` populated.
     *
     * The helper does not mutate the original map so callers can keep their cached payloads and
     * only attach the snapshot when required.
     */
    @JvmStatic
    fun attachPlatformSnapshot(
        transferPayload: Map<String, Any>,
        snapshot: OfflineWallet.PlatformTokenSnapshot,
    ): Map<String, Any> {
        val copy = LinkedHashMap(transferPayload)
        copy[PLATFORM_SNAPSHOT_FIELD] = snapshot.toJsonMap()
        return copy
    }

    /**
     * Injects the snapshot under `transfer.platform_snapshot` within a JSON instruction payload.
     *
     * @param submitTransferJson canonical JSON for a `SubmitOfflineToOnlineTransfer` instruction
     * @param snapshot snapshot to attach
     * @return UTF-8 encoded JSON with the snapshot injected
     * @throws IllegalArgumentException if the JSON payload is not an object or lacks a transfer node
     */
    @JvmStatic
    @Suppress("UNCHECKED_CAST")
    fun attachPlatformSnapshot(
        submitTransferJson: ByteArray,
        snapshot: OfflineWallet.PlatformTokenSnapshot,
    ): ByteArray {
        val json = String(submitTransferJson, Charsets.UTF_8)
        val parsed = JsonParser.parse(json)
        require(parsed is Map<*, *>) { "SubmitOfflineToOnlineTransfer payload must be a JSON object" }
        val root = LinkedHashMap(parsed as Map<String, Any>)
        val transferNode = root[TRANSFER_FIELD]
        require(transferNode is Map<*, *>) { "SubmitOfflineToOnlineTransfer payload missing 'transfer'" }
        val transfer = attachPlatformSnapshot(
            LinkedHashMap(transferNode as Map<String, Any>),
            snapshot,
        )
        root[TRANSFER_FIELD] = transfer
        return JsonEncoder.encode(root).toByteArray(Charsets.UTF_8)
    }
}

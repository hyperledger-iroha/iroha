package org.hyperledger.iroha.sdk.sorafs

/**
 * Aggregates the manifest metadata, provider descriptors, and fetch options for a SoraFS gateway
 * request.
 *
 * Call `toJson()` to obtain the structure expected by the Rust orchestrator (the same
 * layout produced by the CLI `sorafs_cli fetch` command).
 */
class GatewayFetchRequest(
    manifestIdHex: String,
    val chunkerHandle: String? = null,
    val options: GatewayFetchOptions = GatewayFetchOptions(),
    providers: List<GatewayProvider>,
) {
    val manifestIdHex: String = SorafsInputValidator.normalizeHexBytes(manifestIdHex, "manifestIdHex", 32)
    val providers: List<GatewayProvider> = providers.toList()

    init {
        check(this.providers.isNotEmpty()) { "at least one provider must be configured" }
    }

    fun toJson(): Map<String, Any> = buildMap {
        put("manifest_id_hex", manifestIdHex)
        chunkerHandle?.let { put("chunker_handle", it) }
        put("options", options.toJson())
        put("providers", providers.map { it.toJson() })
    }

    /** Returns the JSON representation of this request as a UTF-8 encoded string. */
    fun toJsonString(): String = JsonWriter.encode(toJson())

    /** Returns the JSON representation of this request as UTF-8 bytes. */
    fun toJsonBytes(): ByteArray = SorafsGatewayClient.encodeRequestPayload(this)
}

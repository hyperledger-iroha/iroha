package org.hyperledger.iroha.sdk.sorafs

/**
 * Aggregates the manifest metadata, provider descriptors, and fetch options for a SoraFS gateway
 * request.
 *
 * Call `toJson()` to obtain the structure expected by the Rust orchestrator (the same
 * layout produced by the CLI `sorafs_cli fetch` command). Manifest identifiers must be exact
 * lowercase 32-byte hex and optional chunker handles use `namespace.name@major.minor.patch`.
 */
class GatewayFetchRequest(
    manifestIdHex: String,
    chunkerHandle: String? = null,
    val options: GatewayFetchOptions = GatewayFetchOptions(),
    providers: List<GatewayProvider>,
) {
    val manifestIdHex: String =
        SorafsInputValidator.requireCanonicalHexBytes(manifestIdHex, "manifestIdHex", 32)
    val chunkerHandle: String? = chunkerHandle?.let {
        SorafsInputValidator.requireCanonicalChunkerHandle(it, "chunkerHandle")
    }
    val providers: List<GatewayProvider> = providers.toList()

    init {
        check(this.providers.isNotEmpty()) { "at least one provider must be configured" }
        check(this.providers.size <= 256) { "at most 256 providers may be configured" }
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

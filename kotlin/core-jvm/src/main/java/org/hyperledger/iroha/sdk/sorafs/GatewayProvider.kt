package org.hyperledger.iroha.sdk.sorafs

/**
 * Descriptor for a SoraFS gateway provider.
 *
 * Matches the key/value structure used by the CLI (`--provider name=...`) so Android callers can
 * construct orchestrator requests deterministically.
 */
class GatewayProvider(
    @JvmField val name: String,
    @JvmField val providerIdHex: String,
    @JvmField val baseUrl: String,
    @JvmField val streamTokenBase64: String,
) {
    init {
        SorafsInputValidator.requireNonEmpty(name, "name")
        SorafsInputValidator.normalizeHexBytes(providerIdHex, "providerIdHex", 32)
        SorafsInputValidator.requireNonEmpty(baseUrl, "baseUrl")
        SorafsInputValidator.normalizeBase64MaybeUrl(streamTokenBase64, "streamTokenBase64")
    }

    /** Serialise the provider descriptor to a JSON-compatible map. */
    fun toJson(): Map<String, Any> = linkedMapOf(
        "name" to name,
        "provider_id_hex" to providerIdHex,
        "base_url" to baseUrl,
        "stream_token_b64" to streamTokenBase64,
    )
}

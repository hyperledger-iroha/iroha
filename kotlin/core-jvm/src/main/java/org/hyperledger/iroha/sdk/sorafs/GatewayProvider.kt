package org.hyperledger.iroha.sdk.sorafs

/**
 * Descriptor for a SoraFS gateway provider.
 *
 * Matches the key/value structure used by the CLI (`--provider name=...`) so Android callers can
 * construct orchestrator requests deterministically. Protocol text is accepted only in exact form:
 * provider identifiers and gateway signing keys are lowercase unprefixed 32-byte hex, while stream
 * tokens are canonical standard Base64. The constructor never trims or rewrites caller input.
 */
class GatewayProvider(
    name: String,
    providerIdHex: String,
    gatewayPublicKeyHex: String,
    baseUrl: String,
    streamTokenBase64: String,
) {
    @JvmField
    val name: String = SorafsInputValidator.requireCanonicalProviderName(name, "name")

    @JvmField
    val providerIdHex: String =
        SorafsInputValidator.requireCanonicalHexBytes(providerIdHex, "providerIdHex", 32)

    @JvmField
    val gatewayPublicKeyHex: String =
        SorafsInputValidator.requireCanonicalHexBytes(
            gatewayPublicKeyHex,
            "gatewayPublicKeyHex",
            32,
        )

    @JvmField
    val baseUrl: String =
        SorafsInputValidator.requireCanonicalGatewayBaseUrl(baseUrl, "baseUrl")

    @JvmField
    val streamTokenBase64: String =
        SorafsInputValidator.requireCanonicalStreamTokenBase64(
            streamTokenBase64,
            "streamTokenBase64",
        )

    /** Serialise the provider descriptor to a JSON-ready map. */
    fun toJson(): Map<String, Any> = linkedMapOf(
        "name" to name,
        "provider_id_hex" to providerIdHex,
        "gateway_public_key_hex" to gatewayPublicKeyHex,
        "base_url" to baseUrl,
        "stream_token_b64" to streamTokenBase64,
    )
}

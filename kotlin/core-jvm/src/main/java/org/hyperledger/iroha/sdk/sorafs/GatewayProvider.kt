package org.hyperledger.iroha.sdk.sorafs

import org.hyperledger.iroha.sdk.crypto.Ed25519PublicKeyAdmission

/**
 * Descriptor for a SoraFS gateway provider.
 *
 * Matches the key/value structure used by the CLI (`--provider name=...`) so Android callers can
 * construct orchestrator requests deterministically. Protocol text is accepted only in exact form:
 * provider identifiers are lowercase unprefixed 32-byte hex, gateway signing keys additionally
 * encode canonical prime-order Ed25519 points, and stream tokens are canonical standard Base64.
 * The constructor never trims or rewrites caller input.
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
        requireCanonicalGatewayPublicKeyHex(
            gatewayPublicKeyHex,
            "gatewayPublicKeyHex",
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

private fun requireCanonicalGatewayPublicKeyHex(value: String, field: String): String {
    val canonical = SorafsInputValidator.requireCanonicalHexBytes(value, field, 32)
    val publicKey = ByteArray(Ed25519PublicKeyAdmission.PUBLIC_KEY_LENGTH) { index ->
        val offset = index * 2
        ((Character.digit(canonical[offset], 16) shl 4) or
            Character.digit(canonical[offset + 1], 16)).toByte()
    }
    require(Ed25519PublicKeyAdmission.isValid(publicKey)) {
        "$field must encode a canonical prime-order Ed25519 public key"
    }
    return canonical
}

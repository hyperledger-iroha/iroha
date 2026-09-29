package org.hyperledger.iroha.sdk.client

/** One canonical mapping for RAM-LFE JSON metadata and Norito discriminants. */
internal object RamLfeWireTags {
    // The order is the protocol discriminant; names are exact and have no aliases.
    private val backends = listOf("hkdf-sha3-512-prf-v1", "bfv-affine-v1", "bfv-programmed-v1")
    private val verificationModes = listOf("signed", "proof")

    fun parseBackend(value: Any?, path: String): String {
        check(value is String && value in backends) { "$path must be a supported RAM-LFE backend" }
        return value
    }

    fun parseVerificationMode(value: Any?, path: String): String {
        check(value is String && value in verificationModes) { "$path must be signed or proof" }
        return value
    }

    fun backendTag(value: String): Int = backends.indexOf(value).also {
        require(it >= 0) { "unsupported RAM-LFE backend: $value" }
    }

    fun verificationModeTag(value: String): Int = verificationModes.indexOf(value).also {
        require(it >= 0) { "unsupported RAM-LFE verification mode: $value" }
    }

    fun backendName(tag: Int): String {
        require(tag in backends.indices) { "unsupported RAM-LFE backend tag: $tag" }
        return backends[tag]
    }

    fun verificationModeName(tag: Int): String {
        require(tag in verificationModes.indices) { "unsupported RAM-LFE verification mode tag: $tag" }
        return verificationModes[tag]
    }
}

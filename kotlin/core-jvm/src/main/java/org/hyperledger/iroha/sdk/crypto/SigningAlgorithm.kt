package org.hyperledger.iroha.sdk.crypto

/** Supported transaction and offline signing algorithms exposed by the JVM/Android SDKs. */
enum class SigningAlgorithm(
    val bridgeCode: Int,
    val providerName: String,
    val wireName: String,
) {
    ED25519(0, "Ed25519", "ed25519"),
    ML_DSA(4, "MlDsa", "ml-dsa");

    fun supportsHardwareBackedKeys(): Boolean = this == ED25519

    companion object {
        @JvmStatic
        fun fromAlgorithmName(name: String?): SigningAlgorithm {
            val normalized = normalize(name)
            return when {
                normalized == "ed25519" || normalized == "eddsa" -> ED25519
                normalized == "mldsa"
                    || normalized == "mldsa65"
                    || normalized == "mldsa44"
                    || normalized == "mldsa87" -> ML_DSA
                else -> ED25519
            }
        }

        @JvmStatic
        fun fromBridgeCode(code: Int): SigningAlgorithm =
            entries.firstOrNull { it.bridgeCode == code }
                ?: throw IllegalArgumentException("Unsupported signing algorithm code: $code")

        private fun normalize(name: String?): String {
            if (name.isNullOrBlank()) return ED25519.wireName
            return buildString(name.length) {
                for (ch in name) {
                    if (ch.isLetterOrDigit()) {
                        append(ch.lowercaseChar())
                    }
                }
            }
        }
    }
}

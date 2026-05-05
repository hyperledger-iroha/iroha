package org.hyperledger.iroha.sdk.crypto

/** Supported transaction and offline signing algorithms exposed by the JVM/Android SDKs. */
enum class SigningAlgorithm(
    val bridgeCode: Int,
    val providerName: String,
    val wireName: String,
) {
    ED25519(0, "Ed25519", "ed25519"),
    SECP256K1(1, "Secp256k1", "secp256k1"),
    BLS_NORMAL(2, "BlsNormal", "bls_normal"),
    BLS_SMALL(3, "BlsSmall", "bls_small"),
    ML_DSA(4, "MlDsa", "ml-dsa"),
    GOST_2012_256_A(5, "Gost3410_2012_256ParamSetA", "gost3410-2012-256-paramset-a"),
    GOST_2012_256_B(6, "Gost3410_2012_256ParamSetB", "gost3410-2012-256-paramset-b"),
    GOST_2012_256_C(7, "Gost3410_2012_256ParamSetC", "gost3410-2012-256-paramset-c"),
    GOST_2012_512_A(8, "Gost3410_2012_512ParamSetA", "gost3410-2012-512-paramset-a"),
    GOST_2012_512_B(9, "Gost3410_2012_512ParamSetB", "gost3410-2012-512-paramset-b"),
    SM2(10, "Sm2", "sm2");

    fun supportsHardwareBackedKeys(): Boolean = this == ED25519

    fun isNativeBacked(): Boolean = this != ED25519

    companion object {
        @JvmStatic
        fun fromAlgorithmName(name: String?): SigningAlgorithm {
            val normalized = normalize(name)
            return when {
                normalized == "ed25519" || normalized == "eddsa" -> ED25519
                normalized == "secp256k1"
                    || normalized == "secp"
                    || normalized == "secpk1" -> SECP256K1
                normalized == "mldsa"
                    || normalized == "mldsa65"
                    || normalized == "mldsa44"
                    || normalized == "mldsa87" -> ML_DSA
                normalized == "blsnormal"
                    || normalized == "bls12381g1" -> BLS_NORMAL
                normalized == "blssmall"
                    || normalized == "bls12381g2" -> BLS_SMALL
                normalized == "gost256a"
                    || normalized == "gost34102012256paramseta" -> GOST_2012_256_A
                normalized == "gost256b"
                    || normalized == "gost34102012256paramsetb" -> GOST_2012_256_B
                normalized == "gost256c"
                    || normalized == "gost34102012256paramsetc" -> GOST_2012_256_C
                normalized == "gost512a"
                    || normalized == "gost34102012512paramseta" -> GOST_2012_512_A
                normalized == "gost512b"
                    || normalized == "gost34102012512paramsetb" -> GOST_2012_512_B
                normalized == "sm2" -> SM2
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

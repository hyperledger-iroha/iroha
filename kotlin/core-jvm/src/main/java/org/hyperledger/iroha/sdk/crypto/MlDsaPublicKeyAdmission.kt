package org.hyperledger.iroha.sdk.crypto

/** Structural admission checks for protocol ML-DSA-65 public keys. */
object MlDsaPublicKeyAdmission {
    /** Canonical raw ML-DSA-65 public-key length. */
    const val PUBLIC_KEY_LENGTH: Int = 1_952

    /** Returns `true` only for exact-length, nonzero ML-DSA-65 public-key material. */
    @JvmStatic
    fun isValid(publicKey: ByteArray?): Boolean =
        publicKey != null &&
            publicKey.size == PUBLIC_KEY_LENGTH &&
            publicKey.any { it.toInt() != 0 }
}

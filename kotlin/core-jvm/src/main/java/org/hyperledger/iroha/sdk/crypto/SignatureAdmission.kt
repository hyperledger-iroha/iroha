package org.hyperledger.iroha.sdk.crypto

/** Fixed-width structural admission checks for protocol signatures. */
object SignatureAdmission {
    /** Canonical detached Ed25519 signature length. */
    const val ED25519_SIGNATURE_LENGTH: Int = 64

    /** Canonical detached ML-DSA-65 signature length. */
    const val ML_DSA_65_SIGNATURE_LENGTH: Int = 3_309

    /** Returns `true` when [signature] has the canonical shape for [algorithm]. */
    @JvmStatic
    fun isValid(algorithm: SigningAlgorithm, signature: ByteArray?): Boolean = when (algorithm) {
        SigningAlgorithm.ED25519 -> hasFixedNonzeroShape(signature, ED25519_SIGNATURE_LENGTH)
        SigningAlgorithm.ML_DSA -> hasFixedNonzeroShape(signature, ML_DSA_65_SIGNATURE_LENGTH)
        else -> signature != null
    }

    /** Returns `true` when [signature] has the canonical shape for a multisig [curveId]. */
    @JvmStatic
    fun isValidForCurveId(curveId: Int, signature: ByteArray?): Boolean = when (curveId) {
        0x01 -> hasFixedNonzeroShape(signature, ED25519_SIGNATURE_LENGTH)
        0x02 -> hasFixedNonzeroShape(signature, ML_DSA_65_SIGNATURE_LENGTH)
        else -> signature != null && signature.isNotEmpty()
    }

    private fun hasFixedNonzeroShape(signature: ByteArray?, expectedLength: Int): Boolean =
        signature != null &&
            signature.size == expectedLength &&
            signature.any { it.toInt() != 0 }
}

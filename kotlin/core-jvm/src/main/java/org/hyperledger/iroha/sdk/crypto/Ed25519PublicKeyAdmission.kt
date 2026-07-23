package org.hyperledger.iroha.sdk.crypto

import org.bouncycastle.math.ec.rfc8032.Ed25519

/** Strict admission checks for canonical prime-order Ed25519 public keys. */
object Ed25519PublicKeyAdmission {
    /** Canonical compressed Ed25519 public-key length. */
    const val PUBLIC_KEY_LENGTH: Int = 32

    /** Returns `true` only for canonical points in the prime-order Ed25519 subgroup. */
    @JvmStatic
    fun isValid(publicKey: ByteArray?): Boolean =
        publicKey != null &&
            publicKey.size == PUBLIC_KEY_LENGTH &&
            Ed25519.validatePublicKeyFull(publicKey, 0)
}

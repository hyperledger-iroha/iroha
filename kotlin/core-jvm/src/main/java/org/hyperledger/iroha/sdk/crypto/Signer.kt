package org.hyperledger.iroha.sdk.crypto

/** Computes digital signatures for Iroha payloads. */
interface Signer {

    /**
     * Signs the provided message and returns the encoded signature.
     *
     * The caller passes the raw payload bytes. Implementations must apply Iroha's
     * Blake2b-256 prehash (with the LSB marker) unless explicitly documented otherwise.
     *
     * @param message payload to sign (must not be null)
     * @return encoded signature bytes
     * @throws SigningException if the signature cannot be produced
     */
    @Throws(SigningException::class)
    fun sign(message: ByteArray): ByteArray

    /**
     * Returns the encoded public key corresponding to this signer.
     *
     * The encoding matches the underlying provider; for Ed25519 this is the X.509 SubjectPublicKeyInfo
     * representation. Future revisions will offer Norito-friendly wrappers.
     */
    fun publicKey(): ByteArray

    /**
     * Optional BLS public key used for multi-signature contexts. Implementations that do not support
     * BLS can return `null`.
     */
    fun blsPublicKey(): ByteArray? = null

    /** Algorithm identifier, e.g., `Ed25519`. */
    fun algorithm(): String
}

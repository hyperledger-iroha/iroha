package org.hyperledger.iroha.sdk.crypto

import java.security.GeneralSecurityException
import java.security.PrivateKey
import java.security.PublicKey
import java.security.Signature

/**
 * Ed25519 signer that relies on the JCA provider available on the runtime.
 *
 * Implements Iroha's signing convention: the payload is pre-hashed with Blake2b-256 and the
 * least-significant bit is set to `1` before signing.
 */
class Ed25519Signer(
    private val privateKey: PrivateKey,
    @JvmField val jcaPublicKey: PublicKey,
) : Signer {

    @Throws(SigningException::class)
    override fun sign(message: ByteArray): ByteArray {
        try {
            val prehashed = IrohaHash.prehash(message)
            val signature = Signature.getInstance("Ed25519")
            signature.initSign(privateKey)
            signature.update(prehashed)
            return signature.sign()
        } catch (ex: GeneralSecurityException) {
            throw SigningException("Ed25519 signing failed", ex)
        }
    }

    override fun publicKey(): ByteArray {
        val encoded = jcaPublicKey.encoded
        return encoded?.copyOf() ?: ByteArray(0)
    }

    override fun algorithm(): String = "Ed25519"
}

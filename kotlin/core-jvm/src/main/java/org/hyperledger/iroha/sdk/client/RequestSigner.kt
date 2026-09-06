package org.hyperledger.iroha.sdk.client

import java.security.PrivateKey
import java.security.Signature

/** Application-owned authority for signing the SDK's exact canonical request bytes. */
fun interface RequestSigner {
    /**
     * Sign the supplied message with the account's authorized key.
     *
     * The SDK supplies an owned message and copies the returned signature. Implementations
     * may use software, hardware or a remote signing service without exporting a private key.
     * A signing failure must throw; it must not return a placeholder signature.
     */
    fun sign(message: ByteArray): ByteArray

    companion object {
        /** Bind a JCA Ed25519 key to the canonical signer interface. */
        @JvmStatic
        fun ed25519(privateKey: PrivateKey): RequestSigner = RequestSigner { message ->
            val signature = Signature.getInstance("Ed25519")
            signature.initSign(privateKey)
            signature.update(message)
            signature.sign()
        }
    }
}

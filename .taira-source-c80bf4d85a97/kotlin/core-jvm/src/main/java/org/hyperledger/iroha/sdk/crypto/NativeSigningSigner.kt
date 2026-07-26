package org.hyperledger.iroha.sdk.crypto

/** Signer backed by native `connect_norito_bridge` helpers for non-Ed25519 algorithms. */
class NativeSigningSigner(
    private val signingAlgorithm: SigningAlgorithm,
    private val privateKey: NativeSigningPrivateKey,
    private val keyPublicKey: NativeSigningPublicKey = privateKey.publicKey(),
) : Signer {

    init {
        require(NativeSigningKeyMaterial.supports(signingAlgorithm)) {
            "Unsupported native signing algorithm: $signingAlgorithm"
        }
        require(privateKey.signingAlgorithm == signingAlgorithm) {
            "private key algorithm does not match signer algorithm"
        }
        require(keyPublicKey.signingAlgorithm == signingAlgorithm) {
            "public key algorithm does not match signer algorithm"
        }
    }

    @Throws(SigningException::class)
    override fun sign(message: ByteArray): ByteArray =
        try {
            val prehashed = IrohaHash.prehash(message)
            NativeSignerBridge.signDetached(
                signingAlgorithm,
                privateKey.encoded,
                prehashed,
            )
        } catch (ex: RuntimeException) {
            throw SigningException("${signingAlgorithm.providerName} signing failed", ex)
        }

    override fun publicKey(): ByteArray = keyPublicKey.encoded

    override fun algorithm(): String = signingAlgorithm.providerName
}

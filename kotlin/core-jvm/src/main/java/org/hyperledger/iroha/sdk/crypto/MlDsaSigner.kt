package org.hyperledger.iroha.sdk.crypto

/** ML-DSA signer backed by the native `connect_norito_bridge` helpers. */
class MlDsaSigner(
    private val privateKey: MlDsaPrivateKey,
    private val keyPublicKey: MlDsaPublicKey = privateKey.publicKey(),
) : Signer {

    @Throws(SigningException::class)
    override fun sign(message: ByteArray): ByteArray =
        try {
            val prehashed = IrohaHash.prehash(message)
            NativeSignerBridge.signDetached(
                SigningAlgorithm.ML_DSA,
                privateKey.encoded,
                prehashed,
            )
        } catch (ex: RuntimeException) {
            throw SigningException("ML-DSA signing failed", ex)
        }

    override fun publicKey(): ByteArray = keyPublicKey.encoded

    override fun algorithm(): String = SigningAlgorithm.ML_DSA.providerName
}

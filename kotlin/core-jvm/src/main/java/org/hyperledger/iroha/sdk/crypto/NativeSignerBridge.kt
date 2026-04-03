package org.hyperledger.iroha.sdk.crypto

/** Thin JVM/JNI wrapper around `connect_norito_bridge` signing helpers. */
class NativeSignerBridge private constructor() {
    companion object {
        private const val LIBRARY_NAME = "connect_norito_bridge"
        private val nativeAvailable: Boolean = loadLibrary()

        @JvmStatic
        fun isNativeAvailable(): Boolean = nativeAvailable

        @JvmStatic
        fun publicKeyFromPrivate(algorithm: SigningAlgorithm, privateKey: ByteArray): ByteArray {
            require(privateKey.isNotEmpty()) { "privateKey must not be empty" }
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            return checkNotNull(
                nativePublicKeyFromPrivate(algorithm.bridgeCode, privateKey)
            ) { "nativePublicKeyFromPrivate returned null" }
        }

        @JvmStatic
        fun keypairFromSeed(
            algorithm: SigningAlgorithm,
            seed: ByteArray,
        ): Pair<ByteArray, ByteArray> {
            require(seed.isNotEmpty()) { "seed must not be empty" }
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            val pair = checkNotNull(
                nativeKeypairFromSeed(algorithm.bridgeCode, seed)
            ) { "nativeKeypairFromSeed returned null" }
            require(pair.size == 2) { "nativeKeypairFromSeed must return private/public bytes" }
            val privateKey = pair[0] ?: ByteArray(0)
            val publicKey = pair[1] ?: ByteArray(0)
            require(privateKey.isNotEmpty()) { "nativeKeypairFromSeed returned empty private key" }
            require(publicKey.isNotEmpty()) { "nativeKeypairFromSeed returned empty public key" }
            return privateKey to publicKey
        }

        @JvmStatic
        fun signDetached(
            algorithm: SigningAlgorithm,
            privateKey: ByteArray,
            message: ByteArray,
        ): ByteArray {
            require(privateKey.isNotEmpty()) { "privateKey must not be empty" }
            require(message.isNotEmpty()) { "message must not be empty" }
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            return checkNotNull(
                nativeSignDetached(algorithm.bridgeCode, privateKey, message)
            ) { "nativeSignDetached returned null" }
        }

        @JvmStatic
        fun verifyDetached(
            algorithm: SigningAlgorithm,
            publicKey: ByteArray,
            message: ByteArray,
            signature: ByteArray,
        ): Boolean {
            require(publicKey.isNotEmpty()) { "publicKey must not be empty" }
            require(message.isNotEmpty()) { "message must not be empty" }
            require(signature.isNotEmpty()) { "signature must not be empty" }
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            return nativeVerifyDetached(algorithm.bridgeCode, publicKey, message, signature)
        }

        private fun loadLibrary(): Boolean =
            try {
                System.loadLibrary(LIBRARY_NAME)
                true
            } catch (_: UnsatisfiedLinkError) {
                false
            } catch (_: SecurityException) {
                false
            }

        @JvmStatic
        private external fun nativePublicKeyFromPrivate(
            algorithmCode: Int,
            privateKey: ByteArray,
        ): ByteArray?

        @JvmStatic
        private external fun nativeKeypairFromSeed(
            algorithmCode: Int,
            seed: ByteArray,
        ): Array<ByteArray?>?

        @JvmStatic
        private external fun nativeSignDetached(
            algorithmCode: Int,
            privateKey: ByteArray,
            message: ByteArray,
        ): ByteArray?

        @JvmStatic
        private external fun nativeVerifyDetached(
            algorithmCode: Int,
            publicKey: ByteArray,
            message: ByteArray,
            signature: ByteArray,
        ): Boolean
    }
}

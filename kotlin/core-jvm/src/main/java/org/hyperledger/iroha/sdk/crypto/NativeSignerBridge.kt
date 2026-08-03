package org.hyperledger.iroha.sdk.crypto

import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.client.JsonEncoder
import org.hyperledger.iroha.sdk.core.model.FeePaymentIntent
import org.hyperledger.iroha.sdk.core.model.instructions.RegisterZkAssetInstruction

/** Thin JVM/JNI wrapper around `connect_norito_bridge` signing helpers. */
class NativeSignerBridge private constructor() {
    companion object {
        private const val LIBRARY_NAME = "connect_norito_bridge"
        const val REQUIRED_BRIDGE_ABI_VERSION: Int = 21
        const val REQUIRED_NATIVE_SIGNER_CONTRACT_REVISION: Int = 3
        private const val HASH_BYTES = 32
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

        @JvmStatic
        fun encodeRegisterZkAssetSignedTransaction(
            algorithm: SigningAlgorithm,
            chainId: String?,
            chainDiscriminant: Int,
            authority: String?,
            creationTimeMs: Long,
            ttlMs: Long? = null,
            instruction: RegisterZkAssetInstruction?,
            privateKey: ByteArray?,
            feePayment: FeePaymentIntent,
        ): NativeSignedTransaction {
            requireCreationTime(creationTimeMs)
            val validatedChainDiscriminant = requireChainDiscriminant(chainDiscriminant)
            val selected = requireNotNull(instruction) { "instruction must be provided" }
            val key = requirePrivateKey(privateKey)
            val feePaymentJson = feePaymentJson(feePayment)
            val chainBytes = textBytes(chainId, "chainId")
            val authorityBytes = textBytes(authority, "authority")
            val assetBytes = textBytes(selected.asset, "asset")
            val unshieldBytes = optionalTextBytes(selected.unshieldVerifyingKey)
            val shieldBytes = optionalTextBytes(selected.shieldVerifyingKey)
            val ttl = ttlValue(ttlMs)
            val hasTtl = ttlPresent(ttlMs)
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            return requireNativeSignedOutput(
                nativeEncodeRegisterZkAssetSignedTransaction(
                    algorithm.bridgeCode,
                    chainBytes,
                    validatedChainDiscriminant,
                    authorityBytes,
                    creationTimeMs,
                    ttl,
                    hasTtl,
                    assetBytes,
                    selected.mode.bridgeCode,
                    selected.allowShield,
                    selected.allowUnshield,
                    unshieldBytes,
                    selected.unshieldVerifyingKey != null,
                    shieldBytes,
                    selected.shieldVerifyingKey != null,
                    key,
                    feePaymentJson,
                ),
                "encodeRegisterZkAssetSignedTransaction",
            )
        }

        private fun loadLibrary(): Boolean =
            try {
                System.loadLibrary(LIBRARY_NAME)
                nativeBridgeAbiVersion() == REQUIRED_BRIDGE_ABI_VERSION &&
                    nativeSignerContractRevision() == REQUIRED_NATIVE_SIGNER_CONTRACT_REVISION
            } catch (_: UnsatisfiedLinkError) {
                false
            } catch (_: SecurityException) {
                false
            }

        private fun requireNativeSignedOutput(
            output: Array<ByteArray?>?,
            context: String,
        ): NativeSignedTransaction {
            require(output != null && output.size == 2) { "$context returned invalid output" }
            val versioned = output[0]
            val hash = output[1]
            require(versioned != null && versioned.isNotEmpty()) { "$context returned empty transaction bytes" }
            require(hash != null && hash.size == HASH_BYTES) { "$context returned invalid hash bytes" }
            return NativeSignedTransaction(versioned, hash)
        }

        private fun textBytes(value: String?, name: String): ByteArray {
            require(value != null) { "$name must be provided" }
            require(value.isNotBlank()) { "$name must not be blank" }
            require(value.trim() == value) { "$name must not contain surrounding whitespace" }
            require(value.indexOf('\u0000') < 0) { "$name must not contain NUL" }
            return value.toByteArray(StandardCharsets.UTF_8)
        }

        private fun optionalTextBytes(value: String?): ByteArray =
            value?.toByteArray(StandardCharsets.UTF_8) ?: ByteArray(0)

        private fun feePaymentJson(value: FeePaymentIntent): ByteArray =
            JsonEncoder.encode(value.toJsonMap()).toByteArray(StandardCharsets.UTF_8)

        private fun requireCreationTime(creationTimeMs: Long) {
            require(creationTimeMs >= 0) { "creationTimeMs must be non-negative" }
        }

        private fun ttlValue(ttlMs: Long?): Long {
            if (ttlMs == null) return 0L
            require(ttlMs > 0) { "ttlMs must be positive when provided" }
            return ttlMs
        }

        private fun ttlPresent(ttlMs: Long?): Boolean = ttlMs != null

        private fun requireChainDiscriminant(value: Int): Int {
            require(value in 0..0xffff) { "chainDiscriminant must fit in u16" }
            return value
        }

        private fun requirePrivateKey(privateKey: ByteArray?): ByteArray {
            require(privateKey != null && privateKey.isNotEmpty()) { "privateKey must not be empty" }
            return privateKey.copyOf()
        }

        @JvmStatic
        private external fun nativeBridgeAbiVersion(): Int

        @JvmStatic
        private external fun nativeSignerContractRevision(): Int

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

        @JvmStatic
        private external fun nativeEncodeRegisterZkAssetSignedTransaction(
            algorithmCode: Int,
            chainId: ByteArray,
            chainDiscriminant: Int,
            authority: ByteArray,
            creationTimeMs: Long,
            ttlMs: Long,
            ttlPresent: Boolean,
            asset: ByteArray,
            modeCode: Int,
            allowShield: Boolean,
            allowUnshield: Boolean,
            unshieldVerifyingKey: ByteArray,
            unshieldVerifyingKeyPresent: Boolean,
            shieldVerifyingKey: ByteArray,
            shieldVerifyingKeyPresent: Boolean,
            privateKey: ByteArray,
            feePaymentJson: ByteArray,
        ): Array<ByteArray?>?
    }
}

/** Canonical versioned transaction bytes and native transaction hash returned by zk signers. */
class NativeSignedTransaction(
    versionedSignedTransaction: ByteArray,
    transactionHash: ByteArray,
) {
    private val _versionedSignedTransaction = versionedSignedTransaction.copyOf()
    private val _transactionHash = transactionHash.copyOf()

    init {
        require(_versionedSignedTransaction.isNotEmpty()) {
            "versionedSignedTransaction must not be empty"
        }
        require(_transactionHash.size == 32) { "transactionHash must be exactly 32 bytes" }
    }

    val versionedSignedTransaction: ByteArray get() = _versionedSignedTransaction.copyOf()

    val transactionHash: ByteArray get() = _transactionHash.copyOf()

    fun versionedSignedTransactionBytes(): ByteArray = versionedSignedTransaction

    fun transactionHashBytes(): ByteArray = transactionHash
}

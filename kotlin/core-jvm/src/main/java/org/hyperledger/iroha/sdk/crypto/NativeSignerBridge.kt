package org.hyperledger.iroha.sdk.crypto

import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.core.model.instructions.RegisterZkAssetInstruction
import org.hyperledger.iroha.sdk.core.model.instructions.ShieldInstruction
import org.hyperledger.iroha.sdk.core.model.instructions.UnshieldInstruction
import org.hyperledger.iroha.sdk.core.model.instructions.flattenFixed32
import org.hyperledger.iroha.sdk.core.model.instructions.optionalBytes

/** Thin JVM/JNI wrapper around `connect_norito_bridge` signing helpers. */
class NativeSignerBridge private constructor() {
    companion object {
        private const val LIBRARY_NAME = "connect_norito_bridge"
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
        @JvmOverloads
        fun encodeShieldSignedTransaction(
            algorithm: SigningAlgorithm,
            chainId: String?,
            authority: String?,
            creationTimeMs: Long,
            ttlMs: Long? = null,
            instruction: ShieldInstruction?,
            privateKey: ByteArray?,
        ): NativeSignedTransaction {
            requireCreationTime(creationTimeMs)
            val selected = requireNotNull(instruction) { "instruction must be provided" }
            val key = requirePrivateKey(privateKey)
            val chainBytes = textBytes(chainId, "chainId")
            val authorityBytes = textBytes(authority, "authority")
            val assetBytes = textBytes(selected.asset, "asset")
            val fromBytes = textBytes(selected.from, "from")
            val amountBytes = textBytes(selected.amount, "amount")
            val ttl = ttlValue(ttlMs)
            val hasTtl = ttlPresent(ttlMs)
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            return requireNativeSignedOutput(
                nativeEncodeShieldSignedTransaction(
                    algorithm.bridgeCode,
                    chainBytes,
                    authorityBytes,
                    creationTimeMs,
                    ttl,
                    hasTtl,
                    assetBytes,
                    fromBytes,
                    amountBytes,
                    selected.noteCommitment,
                    selected.encryptedPayload.ephemeralPublicKey,
                    selected.encryptedPayload.nonce,
                    selected.encryptedPayload.ciphertext,
                    key,
                ),
                "encodeShieldSignedTransaction",
            )
        }

        @JvmStatic
        @JvmOverloads
        fun encodeUnshieldSignedTransaction(
            algorithm: SigningAlgorithm,
            chainId: String?,
            authority: String?,
            creationTimeMs: Long,
            ttlMs: Long? = null,
            instruction: UnshieldInstruction?,
            privateKey: ByteArray?,
        ): NativeSignedTransaction {
            requireCreationTime(creationTimeMs)
            val selected = requireNotNull(instruction) { "instruction must be provided" }
            val key = requirePrivateKey(privateKey)
            val chainBytes = textBytes(chainId, "chainId")
            val authorityBytes = textBytes(authority, "authority")
            val assetBytes = textBytes(selected.asset, "asset")
            val toBytes = textBytes(selected.to, "to")
            val publicAmountBytes = textBytes(selected.publicAmount, "publicAmount")
            val inputsBytes = flattenFixed32(selected.inputs)
            val outputsBytes = flattenFixed32(selected.outputs)
            val proofJsonBytes = selected.proof.toNativeJson().toByteArray(StandardCharsets.UTF_8)
            val rootHintBytes = optionalBytes(selected.rootHint)
            val ttl = ttlValue(ttlMs)
            val hasTtl = ttlPresent(ttlMs)
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            return requireNativeSignedOutput(
                nativeEncodeUnshieldSignedTransaction(
                    algorithm.bridgeCode,
                    chainBytes,
                    authorityBytes,
                    creationTimeMs,
                    ttl,
                    hasTtl,
                    assetBytes,
                    toBytes,
                    publicAmountBytes,
                    inputsBytes,
                    outputsBytes,
                    proofJsonBytes,
                    rootHintBytes,
                    key,
                ),
                "encodeUnshieldSignedTransaction",
            )
        }

        @JvmStatic
        @JvmOverloads
        fun encodeRegisterZkAssetSignedTransaction(
            algorithm: SigningAlgorithm,
            chainId: String?,
            authority: String?,
            creationTimeMs: Long,
            ttlMs: Long? = null,
            instruction: RegisterZkAssetInstruction?,
            privateKey: ByteArray?,
        ): NativeSignedTransaction {
            requireCreationTime(creationTimeMs)
            val selected = requireNotNull(instruction) { "instruction must be provided" }
            val key = requirePrivateKey(privateKey)
            val chainBytes = textBytes(chainId, "chainId")
            val authorityBytes = textBytes(authority, "authority")
            val assetBytes = textBytes(selected.asset, "asset")
            val transferBytes = optionalTextBytes(selected.transferVerifyingKey)
            val unshieldBytes = optionalTextBytes(selected.unshieldVerifyingKey)
            val shieldBytes = optionalTextBytes(selected.shieldVerifyingKey)
            val ttl = ttlValue(ttlMs)
            val hasTtl = ttlPresent(ttlMs)
            check(nativeAvailable) { "$LIBRARY_NAME is not available in this runtime" }
            return requireNativeSignedOutput(
                nativeEncodeRegisterZkAssetSignedTransaction(
                    algorithm.bridgeCode,
                    chainBytes,
                    authorityBytes,
                    creationTimeMs,
                    ttl,
                    hasTtl,
                    assetBytes,
                    selected.mode.bridgeCode,
                    selected.allowShield,
                    selected.allowUnshield,
                    transferBytes,
                    selected.transferVerifyingKey != null,
                    unshieldBytes,
                    selected.unshieldVerifyingKey != null,
                    shieldBytes,
                    selected.shieldVerifyingKey != null,
                    key,
                ),
                "encodeRegisterZkAssetSignedTransaction",
            )
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

        private fun requireCreationTime(creationTimeMs: Long) {
            require(creationTimeMs >= 0) { "creationTimeMs must be non-negative" }
        }

        private fun ttlValue(ttlMs: Long?): Long {
            if (ttlMs == null) return 0L
            require(ttlMs > 0) { "ttlMs must be positive when provided" }
            return ttlMs
        }

        private fun ttlPresent(ttlMs: Long?): Boolean = ttlMs != null

        private fun requirePrivateKey(privateKey: ByteArray?): ByteArray {
            require(privateKey != null && privateKey.isNotEmpty()) { "privateKey must not be empty" }
            return privateKey.copyOf()
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

        @JvmStatic
        private external fun nativeEncodeShieldSignedTransaction(
            algorithmCode: Int,
            chainId: ByteArray,
            authority: ByteArray,
            creationTimeMs: Long,
            ttlMs: Long,
            ttlPresent: Boolean,
            asset: ByteArray,
            from: ByteArray,
            amount: ByteArray,
            noteCommitment: ByteArray,
            payloadEphemeralPublicKey: ByteArray,
            payloadNonce: ByteArray,
            payloadCiphertext: ByteArray,
            privateKey: ByteArray,
        ): Array<ByteArray?>?

        @JvmStatic
        private external fun nativeEncodeUnshieldSignedTransaction(
            algorithmCode: Int,
            chainId: ByteArray,
            authority: ByteArray,
            creationTimeMs: Long,
            ttlMs: Long,
            ttlPresent: Boolean,
            asset: ByteArray,
            to: ByteArray,
            publicAmount: ByteArray,
            inputs: ByteArray,
            outputs: ByteArray,
            proofJson: ByteArray,
            rootHint: ByteArray,
            privateKey: ByteArray,
        ): Array<ByteArray?>?

        @JvmStatic
        private external fun nativeEncodeRegisterZkAssetSignedTransaction(
            algorithmCode: Int,
            chainId: ByteArray,
            authority: ByteArray,
            creationTimeMs: Long,
            ttlMs: Long,
            ttlPresent: Boolean,
            asset: ByteArray,
            modeCode: Int,
            allowShield: Boolean,
            allowUnshield: Boolean,
            transferVerifyingKey: ByteArray,
            transferVerifyingKeyPresent: Boolean,
            unshieldVerifyingKey: ByteArray,
            unshieldVerifyingKeyPresent: Boolean,
            shieldVerifyingKey: ByteArray,
            shieldVerifyingKeyPresent: Boolean,
            privateKey: ByteArray,
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

package org.hyperledger.iroha.sdk.offline

/** Native helpers for computing offline balance commitments and proofs. */
class OfflineBalanceProof private constructor() {

    class Artifacts internal constructor(
        resultingCommitment: ByteArray,
        proof: ByteArray,
    ) {
        private val _resultingCommitment: ByteArray = resultingCommitment.copyOf()
        private val _proof: ByteArray = proof.copyOf()

        val resultingCommitment: ByteArray get() = _resultingCommitment.copyOf()
        val proof: ByteArray get() = _proof.copyOf()

        val resultingCommitmentHex: String get() = encodeHex(_resultingCommitment)
        val proofHex: String get() = encodeHex(_proof)
    }

    companion object {
        private val NATIVE_AVAILABLE: Boolean

        init {
            var available: Boolean
            try {
                System.loadLibrary("connect_norito_bridge")
                available = true
            } catch (_: UnsatisfiedLinkError) {
                available = false
            }
            NATIVE_AVAILABLE = available
        }

        @JvmStatic
        fun isNativeAvailable(): Boolean = NATIVE_AVAILABLE

        @JvmStatic
        fun updateCommitment(
            initialCommitment: ByteArray,
            claimedDelta: String,
            initialBlinding: ByteArray,
            resultingBlinding: ByteArray,
        ): ByteArray {
            check(NATIVE_AVAILABLE) { "connect_norito_bridge is not available in this runtime" }
            return nativeUpdateCommitment(initialCommitment, claimedDelta, initialBlinding, resultingBlinding)
        }

        @JvmStatic
        fun generateProof(
            chainId: String,
            initialCommitment: ByteArray,
            resultingCommitment: ByteArray,
            claimedDelta: String,
            resultingValue: String,
            initialBlinding: ByteArray,
            resultingBlinding: ByteArray,
        ): ByteArray {
            check(NATIVE_AVAILABLE) { "connect_norito_bridge is not available in this runtime" }
            return nativeGenerate(
                chainId,
                initialCommitment,
                resultingCommitment,
                claimedDelta,
                resultingValue,
                initialBlinding,
                resultingBlinding,
            )
        }

        @JvmStatic
        fun advanceCommitment(
            chainId: String,
            claimedDelta: String,
            resultingValue: String,
            initialCommitmentHex: String,
            initialBlindingHex: String,
            resultingBlindingHex: String,
        ): Artifacts {
            val initialCommitment = decodeHex(initialCommitmentHex, "initialCommitmentHex")
            val initialBlinding = decodeHex(initialBlindingHex, "initialBlindingHex")
            val resultingBlinding = decodeHex(resultingBlindingHex, "resultingBlindingHex")
            val resultingCommitment =
                updateCommitment(initialCommitment, claimedDelta, initialBlinding, resultingBlinding)
            val proof = generateProof(
                chainId,
                initialCommitment,
                resultingCommitment,
                claimedDelta,
                resultingValue,
                initialBlinding,
                resultingBlinding,
            )
            return Artifacts(resultingCommitment, proof)
        }

        @JvmStatic
        private external fun nativeUpdateCommitment(
            initialCommitment: ByteArray,
            claimedDelta: String,
            initialBlinding: ByteArray,
            resultingBlinding: ByteArray,
        ): ByteArray

        @JvmStatic
        private external fun nativeGenerate(
            chainId: String,
            initialCommitment: ByteArray,
            resultingCommitment: ByteArray,
            claimedDelta: String,
            resultingValue: String,
            initialBlinding: ByteArray,
            resultingBlinding: ByteArray,
        ): ByteArray

        private fun decodeHex(hex: String?, field: String): ByteArray {
            val trimmed = hex?.trim() ?: ""
            require(trimmed.length % 2 == 0) { "$field must contain an even number of hex characters" }
            val out = ByteArray(trimmed.length / 2)
            for (i in trimmed.indices step 2) {
                val hi = Character.digit(trimmed[i], 16)
                val lo = Character.digit(trimmed[i + 1], 16)
                require(hi != -1 && lo != -1) { "$field contains non-hex characters" }
                out[i / 2] = ((hi shl 4) + lo).toByte()
            }
            return out
        }

        private fun encodeHex(bytes: ByteArray): String {
            val builder = StringBuilder(bytes.size * 2)
            for (b in bytes) {
                builder.append(String.format("%02x", b))
            }
            return builder.toString()
        }
    }
}

package org.hyperledger.iroha.sdk.gpu

import java.io.File

/**
 * Explicitly selected CUDA acceleration, shared by Kotlin and Java consumers.
 *
 * Each operation accepts an ordered batch; a single input is a batch of size one.
 * Poseidon inputs and BN254 limbs use the unsigned bit pattern of each JVM long.
 * BN254 elements contain four little-endian limbs strictly below the field modulus.
 * Inputs and successful outputs are owned snapshots. A null result means the
 * backend did not compute the batch; it never means a zero result or a CPU fallback.
 * Backend exceptions remain visible. Availability can change after device failure.
 */
class CudaAccelerators(private val backend: Backend) {
    /** Current backend state; construction does not change another context's backend. */
    val status: Status get() = backend.status

    /** Compute the truncated BN254 Poseidon permutation for each pair. */
    fun poseidon2(inputs: Array<LongArray>): LongArray? =
        checkedOutput(backend.poseidon2(flatten(inputs, 2, false)), inputs.size)

    /** Compute the truncated BN254 Poseidon permutation for each six-word row. */
    fun poseidon6(inputs: Array<LongArray>): LongArray? =
        checkedOutput(backend.poseidon6(flatten(inputs, 6, false)), inputs.size)

    /** Add corresponding canonical BN254 field elements modulo the field modulus. */
    fun bn254Add(lhs: Array<LongArray>, rhs: Array<LongArray>): Array<LongArray>? =
        fieldOperation(lhs, rhs, backend::bn254Add)

    /** Subtract corresponding canonical BN254 field elements modulo the field modulus. */
    fun bn254Sub(lhs: Array<LongArray>, rhs: Array<LongArray>): Array<LongArray>? =
        fieldOperation(lhs, rhs, backend::bn254Sub)

    /** Multiply corresponding canonical BN254 field elements modulo the field modulus. */
    fun bn254Mul(lhs: Array<LongArray>, rhs: Array<LongArray>): Array<LongArray>? =
        fieldOperation(lhs, rhs, backend::bn254Mul)

    private fun fieldOperation(
        lhs: Array<LongArray>,
        rhs: Array<LongArray>,
        operation: (LongArray, LongArray) -> LongArray?,
    ): Array<LongArray>? {
        require(lhs.size == rhs.size) { "BN254 batches must have equal lengths" }
        val left = flatten(lhs, 4, true)
        val right = flatten(rhs, 4, true)
        val result = checkedOutput(operation(left, right), lhs.size * 4) ?: return null
        return Array(lhs.size) { index ->
            result.copyOfRange(index * 4, index * 4 + 4).also {
                check(canonicalField(it)) { "CUDA backend returned a noncanonical BN254 element" }
            }
        }
    }

    private fun flatten(rows: Array<LongArray>, width: Int, field: Boolean): LongArray {
        require(rows.size <= MAXIMUM_BATCH_SIZE) { "CUDA batch exceeds $MAXIMUM_BATCH_SIZE inputs" }
        // Validate every row before allocating the contiguous native input.
        for (row in rows) require(row.size == width) { "CUDA input row must contain $width words" }
        val result = LongArray(rows.size * width)
        for (index in rows.indices) {
            val row = rows[index].copyOf()
            require(row.size == width) { "CUDA input row changed size" }
            if (field) require(canonicalField(row)) { "BN254 input must be below the field modulus" }
            row.copyInto(result, index * width)
        }
        return result
    }

    private fun checkedOutput(output: LongArray?, expected: Int): LongArray? {
        if (output == null) return null
        check(output.size == expected) { "CUDA backend returned an incorrect batch size" }
        return output.copyOf()
    }

    /** One backend state, without contradictory available/disabled booleans. */
    enum class Status {
        /** Driver and CUDA implementation are available. */
        READY,
        /** The selected native bridge has no usable CUDA device or implementation. */
        UNAVAILABLE,
        /** Acceleration was explicitly disabled or disabled after a backend error. */
        DISABLED,
    }

    /**
     * Application-injected implementation. Inputs are private flattened snapshots;
     * return null if the requested computation is unavailable. Successful results
     * must preserve batch order and contain one Poseidon word or four BN254 limbs
     * per input. Implementations own their resources and concurrency policy.
     */
    interface Backend {
        val status: Status
        fun poseidon2(inputs: LongArray): LongArray?
        fun poseidon6(inputs: LongArray): LongArray?
        fun bn254Add(lhs: LongArray, rhs: LongArray): LongArray?
        fun bn254Sub(lhs: LongArray, rhs: LongArray): LongArray?
        fun bn254Mul(lhs: LongArray, rhs: LongArray): LongArray?
    }

    private object DisabledBackend : Backend {
        override val status = Status.DISABLED
        override fun poseidon2(inputs: LongArray): LongArray? = null
        override fun poseidon6(inputs: LongArray): LongArray? = null
        override fun bn254Add(lhs: LongArray, rhs: LongArray): LongArray? = null
        override fun bn254Sub(lhs: LongArray, rhs: LongArray): LongArray? = null
        override fun bn254Mul(lhs: LongArray, rhs: LongArray): LongArray? = null
    }

    private class NativeBackend : Backend {
        override val status: Status get() = when {
            nativeCudaDisabled() -> Status.DISABLED
            nativeCudaAvailable() -> Status.READY
            else -> Status.UNAVAILABLE
        }
        override fun poseidon2(inputs: LongArray): LongArray? =
            result(inputs.size / 2) { nativePoseidon2(inputs, it) }
        override fun poseidon6(inputs: LongArray): LongArray? =
            result(inputs.size / 6) { nativePoseidon6(inputs, it) }
        override fun bn254Add(lhs: LongArray, rhs: LongArray): LongArray? =
            result(lhs.size) { nativeBn254Add(lhs, rhs, it) }
        override fun bn254Sub(lhs: LongArray, rhs: LongArray): LongArray? =
            result(lhs.size) { nativeBn254Sub(lhs, rhs, it) }
        override fun bn254Mul(lhs: LongArray, rhs: LongArray): LongArray? =
            result(lhs.size) { nativeBn254Mul(lhs, rhs, it) }
        private inline fun result(size: Int, compute: (LongArray) -> Boolean): LongArray? =
            LongArray(size).let { if (compute(it)) it else null }
    }

    companion object {
        /** Maximum batch size, checked before native-buffer allocation. */
        const val MAXIMUM_BATCH_SIZE: Int = 65_536
        private val MODULUS = longArrayOf(
            0x43e1f593f0000001L, 0x2833e84879b97091L, 0xb85045b68181585dUL.toLong(), 0x30644e72e131a029L,
        )

        /** Construct a context that never loads JNI or computes an accelerated result. */
        @JvmStatic
        fun disabled(): CudaAccelerators = CudaAccelerators(DisabledBackend)

        /**
         * Load one explicitly supplied absolute library path and bind the canonical JNI API.
         * Missing libraries, loading restrictions and missing symbols are errors; none is
         * silently converted into a disabled backend. Inspect status for device availability.
         */
        @JvmStatic
        fun loadNative(libraryPath: String): CudaAccelerators {
            require(File(libraryPath).isAbsolute) { "CUDA native library path must be absolute" }
            System.load(libraryPath)
            val backend = NativeBackend()
            nativeCudaAvailable()
            nativeCudaDisabled()
            return CudaAccelerators(backend)
        }

        private fun canonicalField(limbs: LongArray): Boolean {
            for (index in 3 downTo 0) {
                val comparison = (limbs[index] xor Long.MIN_VALUE).compareTo(MODULUS[index] xor Long.MIN_VALUE)
                if (comparison != 0) return comparison < 0
            }
            return false
        }

        @JvmStatic private external fun nativeCudaAvailable(): Boolean
        @JvmStatic private external fun nativeCudaDisabled(): Boolean
        @JvmStatic private external fun nativePoseidon2(inputs: LongArray, output: LongArray): Boolean
        @JvmStatic private external fun nativePoseidon6(inputs: LongArray, output: LongArray): Boolean
        @JvmStatic private external fun nativeBn254Add(lhs: LongArray, rhs: LongArray, output: LongArray): Boolean
        @JvmStatic private external fun nativeBn254Sub(lhs: LongArray, rhs: LongArray, output: LongArray): Boolean
        @JvmStatic private external fun nativeBn254Mul(lhs: LongArray, rhs: LongArray, output: LongArray): Boolean
    }
}

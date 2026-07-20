package org.hyperledger.iroha.sdk.core.model

/** Maximum canonical argument-record size accepted by a contract invocation. */
const val MAX_CONTRACT_ARGUMENT_RECORD_BYTES: Int = 1024 * 1024

/**
 * By-reference invocation of a deployed contract instance.
 *
 * The expected code hash and optional argument record are copied on input and output so the
 * transaction bytes authorized by a signer cannot be changed through a retained array reference.
 */
class ContractInvocation(
    @JvmField val contractAddress: String,
    expectedCodeHash: ByteArray,
    @JvmField val entrypoint: String,
    arguments: ByteArray? = null,
) {
    private val _expectedCodeHash: ByteArray = expectedCodeHash.copyOf()
    private val _arguments: ByteArray? = arguments?.copyOf()

    /** Exact 32-byte hash of the deployed code authorized by this invocation. */
    val expectedCodeHash: ByteArray get() = _expectedCodeHash.copyOf()

    /** Canonical schema-bound argument-record bytes, when present. */
    val arguments: ByteArray? get() = _arguments?.copyOf()

    init {
        requireCanonicalV1ContractAddress(contractAddress)
        require(_expectedCodeHash.size == EXPECTED_CODE_HASH_BYTES) {
            "expectedCodeHash must contain exactly $EXPECTED_CODE_HASH_BYTES bytes"
        }
        require((_expectedCodeHash.last().toInt() and 1) == 1) {
            "expectedCodeHash must use Iroha's marked hash encoding"
        }
        require(entrypoint.isNotEmpty() && entrypoint == entrypoint.trim()) {
            "entrypoint must be an exact non-empty string"
        }
        require((_arguments?.size ?: 0) <= MAX_CONTRACT_ARGUMENT_RECORD_BYTES) {
            "arguments must not exceed $MAX_CONTRACT_ARGUMENT_RECORD_BYTES bytes"
        }
    }

    override fun equals(other: Any?): Boolean {
        if (this === other) return true
        if (other !is ContractInvocation) return false
        return contractAddress == other.contractAddress &&
            _expectedCodeHash.contentEquals(other._expectedCodeHash) &&
            entrypoint == other.entrypoint &&
            nullableContentEquals(_arguments, other._arguments)
    }

    override fun hashCode(): Int {
        var result = contractAddress.hashCode()
        result = 31 * result + _expectedCodeHash.contentHashCode()
        result = 31 * result + entrypoint.hashCode()
        result = 31 * result + (_arguments?.contentHashCode() ?: 0)
        return result
    }

    private fun nullableContentEquals(left: ByteArray?, right: ByteArray?): Boolean =
        when {
            left === right -> true
            left == null || right == null -> false
            else -> left.contentEquals(right)
        }

    companion object {
        /** Java-friendly alias for [MAX_CONTRACT_ARGUMENT_RECORD_BYTES]. */
        const val MAX_ARGUMENT_BYTES: Int = MAX_CONTRACT_ARGUMENT_RECORD_BYTES

        private const val EXPECTED_CODE_HASH_BYTES = 32
    }
}

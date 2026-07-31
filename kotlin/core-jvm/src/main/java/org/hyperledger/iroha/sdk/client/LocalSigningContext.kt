package org.hyperledger.iroha.sdk.client

/**
 * Immutable local context used to validate server-prepared transaction drafts before signing.
 *
 * The chain identifier is configured by the caller and is never inferred from a server response.
 */
class LocalSigningContext(
    private val chainId: String,
) {
    init {
        requireCanonicalChainId(chainId)
    }

    /** Exact chain identifier required in every locally signed draft. */
    fun chainId(): String = chainId

    override fun equals(other: Any?): Boolean =
        this === other || other is LocalSigningContext && chainId == other.chainId

    override fun hashCode(): Int = chainId.hashCode()

    companion object {
        private const val MAX_CHAIN_ID_BYTES = 128

        private fun requireCanonicalChainId(value: String) {
            require(value.isNotEmpty() && value.length <= MAX_CHAIN_ID_BYTES) {
                "chainId must contain 1..$MAX_CHAIN_ID_BYTES ASCII bytes"
            }
            require(value.first().isAsciiLetterOrDigit() && value.last().isAsciiLetterOrDigit()) {
                "chainId must begin and end with an ASCII alphanumeric character"
            }
            require(value.all { character ->
                character.isAsciiLetterOrDigit() || character == '.' || character == '_' ||
                    character == ':' || character == '-'
            }) {
                "chainId contains a non-canonical character"
            }
        }

        private fun Char.isAsciiLetterOrDigit(): Boolean =
            this in 'a'..'z' || this in 'A'..'Z' || this in '0'..'9'
    }
}

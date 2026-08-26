package org.hyperledger.iroha.sdk.nexus

/** Helpers for validating exact UAID literals before issuing Torii requests. */
object UaidLiteral {

    /**
     * Validates and returns the provided exact `uaid:<64 lowercase hex>` literal.
     *
     * @param value exact canonical UAID literal
     * @return the unchanged literal
     */
    @JvmStatic
    fun canonicalize(value: String): String = canonicalize(value, "uaid")

    /**
     * Validates and returns the provided exact `uaid:<64 lowercase hex>` literal.
     *
     * @param value exact canonical UAID literal
     * @param context field description used in validation errors
     * @return the unchanged literal
     */
    @JvmStatic
    fun canonicalize(value: String, context: String): String {
        val literal = requireExactNonEmpty(value, context)
        require(literal.matches(Regex("uaid:[0-9a-f]{64}"))) {
            "$context must be an exact canonical uaid:<64 lowercase hex> literal"
        }
        require("13579bdf".indexOf(literal.last()) >= 0) {
            "$context must have least significant bit set to 1"
        }
        return literal
    }

    private fun requireExactNonEmpty(value: String, context: String): String {
        require(value.isNotBlank()) { "$context must not be blank" }
        require(value.trim() == value) { "$context must not contain surrounding whitespace" }
        return value
    }
}

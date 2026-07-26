package org.hyperledger.iroha.sdk.nexus

/** Helpers for canonicalizing exact UAID literals before issuing Torii requests. */
object UaidLiteral {

    /**
     * Canonicalizes the provided exact UAID literal and returns the canonical `uaid:<hex>` form.
     *
     * @param value raw UAID literal (with or without the `uaid:` prefix)
     * @return canonical literal
     */
    @JvmStatic
    fun canonicalize(value: String): String = canonicalize(value, "uaid")

    /**
     * Canonicalizes the provided exact UAID literal and returns the canonical `uaid:<hex>` form.
     *
     * @param value raw UAID literal (with or without the `uaid:` prefix)
     * @param context field description used in validation errors
     * @return canonical literal
     */
    @JvmStatic
    fun canonicalize(value: String, context: String): String {
        val literal = requireExactNonEmpty(value, context)
        val lower = literal.lowercase()
        val hexPortion = if (lower.startsWith("uaid:")) literal.substring("uaid:".length) else literal
        require(hexPortion.trim() == hexPortion) { "$context must not contain surrounding whitespace" }
        require(hexPortion.length == 64 && hexPortion.matches(Regex("(?i)[0-9a-f]{64}"))) {
            "$context must contain 64 hex characters"
        }
        val lastChar = hexPortion.last()
        require("13579bdf".indexOf(lastChar.lowercaseChar()) >= 0) {
            "$context must have least significant bit set to 1"
        }
        return "uaid:" + hexPortion.lowercase()
    }

    private fun requireExactNonEmpty(value: String, context: String): String {
        require(value.isNotBlank()) { "$context must not be blank" }
        require(value.trim() == value) { "$context must not contain surrounding whitespace" }
        return value
    }
}

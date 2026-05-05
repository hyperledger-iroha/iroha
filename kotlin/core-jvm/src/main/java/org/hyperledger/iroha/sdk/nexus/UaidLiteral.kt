package org.hyperledger.iroha.sdk.nexus

/** Helpers for normalising UAID literals before issuing Torii requests. */
object UaidLiteral {

    /**
     * Normalises the provided UAID literal and returns the canonical `uaid:<hex>` form.
     *
     * @param value raw UAID literal (with or without the `uaid:` prefix)
     * @return canonical literal
     */
    @JvmStatic
    fun canonicalize(value: String): String = canonicalize(value, "uaid")

    /**
     * Normalises the provided UAID literal and returns the canonical `uaid:<hex>` form.
     *
     * @param value raw UAID literal (with or without the `uaid:` prefix)
     * @param context field description used in validation errors
     * @return canonical literal
     */
    @JvmStatic
    fun canonicalize(value: String, context: String): String {
        val literal = value.trim()
        require(literal.isNotEmpty()) { "$context must not be blank" }
        val lower = literal.lowercase()
        val hexPortion = if (lower.startsWith("uaid:")) literal.substring("uaid:".length) else literal
        val trimmedHex = hexPortion.trim()
        require(trimmedHex.length == 64 && trimmedHex.matches(Regex("(?i)[0-9a-f]{64}"))) {
            "$context must contain 64 hex characters"
        }
        val lastChar = trimmedHex.last()
        require("13579bdf".indexOf(lastChar.lowercaseChar()) >= 0) {
            "$context must have least significant bit set to 1"
        }
        return "uaid:" + trimmedHex.lowercase()
    }
}

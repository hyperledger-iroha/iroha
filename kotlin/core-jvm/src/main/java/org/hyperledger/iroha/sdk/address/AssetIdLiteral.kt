package org.hyperledger.iroha.sdk.address

private const val PREFIX = "norito:"

/**
 * Normalizes an encoded asset identifier.
 *
 * Accepted format is `norito:<hex>`.
 *
 * @param assetId asset identifier string
 * @return canonical encoded asset identifier with a lowercase hex payload
 */
fun normalizeEncoded(assetId: String): String = normalizeEncoded(assetId, "assetId")

/**
 * Normalizes an encoded asset identifier with a custom field label.
 *
 * @param assetId asset identifier string
 * @param field field name used in validation messages
 * @return canonical encoded asset identifier with a lowercase hex payload
 */
fun normalizeEncoded(assetId: String, field: String): String {
    val trimmed = assetId.trim()
    require(trimmed.isNotEmpty()) { "$field must not be blank" }
    require(trimmed.regionMatches(0, PREFIX, 0, PREFIX.length, ignoreCase = true)) {
        "$field must be encoded as norito:<hex>"
    }
    val hex = trimmed.substring(PREFIX.length)
    require(hex.isNotEmpty() && (hex.length and 1) == 0 && isHex(hex)) {
        "$field must be encoded as norito:<hex>"
    }
    return PREFIX + hex.lowercase()
}

private fun isHex(value: String): Boolean {
    for (ch in value) {
        if (ch !in '0'..'9' && ch !in 'a'..'f' && ch !in 'A'..'F') return false
    }
    return true
}

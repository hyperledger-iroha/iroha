package org.hyperledger.iroha.sdk.sorafs

internal object SorafsInputValidator {

    @JvmStatic
    fun requireNonEmpty(value: String, field: String): String {
        val trimmed = value.trim()
        require(trimmed.isNotEmpty()) { "$field must not be empty" }
        return trimmed
    }

    @JvmStatic
    fun normalizeHex(value: String, field: String): String {
        val trimmed = requireNonEmpty(value, field)
        val normalized = stripHexPrefix(trimmed)
        require(normalized.isNotEmpty()) { "$field must be a non-empty hex string" }
        require(normalized.length % 2 == 0) { "$field must contain an even number of hex characters" }
        for (c in normalized) {
            require(c.isHexDigit()) { "$field must be a hex string" }
        }
        return normalized.lowercase()
    }

    @JvmStatic
    fun normalizeHexBytes(value: String, field: String, expectedBytes: Int): String {
        require(expectedBytes > 0) { "expectedBytes must be positive" }
        val normalized = normalizeHex(value, field)
        val expectedLength = expectedBytes * 2
        require(normalized.length == expectedLength) { "$field must be a $expectedBytes-byte hex string" }
        return normalized
    }

    @JvmStatic
    fun normalizeBase64MaybeUrl(value: String, field: String): String {
        val compact = requireNonEmpty(value, field).replace("\\s+".toRegex(), "")
        require(compact.isNotEmpty()) { "$field must be a non-empty base64 string" }
        var normalized = compact
        val hadUrlChars = '-' in normalized || '_' in normalized
        if (hadUrlChars) {
            normalized = normalized.replace('-', '+').replace('_', '/')
        }
        val padded = padBase64(normalized, field, hadUrlChars)
        val errorLabel = "$field must be a valid base64${if (hadUrlChars) " or base64url" else ""} string"
        val decoded: ByteArray
        try {
            decoded = java.util.Base64.getDecoder().decode(padded)
        } catch (ex: IllegalArgumentException) {
            throw IllegalArgumentException(errorLabel, ex)
        }
        require(decoded.isNotEmpty()) { "$field must be a non-empty base64 string" }
        return java.util.Base64.getEncoder().encodeToString(decoded)
    }

    private fun stripHexPrefix(value: String): String =
        if (value.startsWith("0x") || value.startsWith("0X")) value.substring(2) else value

    private fun Char.isHexDigit(): Boolean =
        this in '0'..'9' || this in 'a'..'f' || this in 'A'..'F'

    private fun padBase64(value: String, field: String, allowUrlLabel: Boolean): String {
        val paddingIndex = value.indexOf('=')
        if (paddingIndex >= 0) {
            for (i in paddingIndex until value.length) {
                require(value[i] == '=') { labelForBase64(field, allowUrlLabel) }
            }
            val paddingCount = value.length - paddingIndex
            require(paddingCount <= 2 && value.length % 4 == 0) { labelForBase64(field, allowUrlLabel) }
            return value
        }
        val mod = value.length % 4
        require(mod != 1) { labelForBase64(field, allowUrlLabel) }
        return if (mod == 0) value else value + "===".substring(0, 4 - mod)
    }

    private fun labelForBase64(field: String, allowUrlLabel: Boolean): String =
        "$field must be a valid base64${if (allowUrlLabel) " or base64url" else ""} string"
}

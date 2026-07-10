package org.hyperledger.iroha.sdk.sorafs

import java.net.InetAddress
import java.net.URI
import java.net.URISyntaxException

private const val MAX_PROVIDER_NAME_BYTES = 128
private const val MAX_STREAM_TOKEN_ENCODED_BYTES = 90 * 1024
private const val MAX_STREAM_TOKEN_DECODED_BYTES = 64 * 1024

internal object SorafsInputValidator {

    @JvmStatic
    fun requireExactNonEmpty(value: String, field: String): String {
        require(value.isNotEmpty()) { "$field must not be empty" }
        require(!value.first().isBoundaryWhitespace() && !value.last().isBoundaryWhitespace()) {
            "$field must not contain leading or trailing whitespace"
        }
        require(value.none(Char::isISOControl)) { "$field must not contain control characters" }
        return value
    }

    @JvmStatic
    fun requireCanonicalHex(value: String, field: String): String {
        requireExactNonEmpty(value, field)
        require(value.length % 2 == 0) { "$field must contain an even number of hex characters" }
        for (c in value) {
            require(c.isLowerHexDigit()) {
                "$field must be canonical lowercase hex without a prefix"
            }
        }
        return value
    }

    @JvmStatic
    fun requireCanonicalHexBytes(value: String, field: String, expectedBytes: Int): String {
        require(expectedBytes > 0) { "expectedBytes must be positive" }
        require(expectedBytes <= Int.MAX_VALUE / 2) { "expectedBytes is too large" }
        val canonical = requireCanonicalHex(value, field)
        val expectedLength = expectedBytes * 2
        require(canonical.length == expectedLength) {
            "$field must be a $expectedBytes-byte lowercase hex string"
        }
        require(canonical.any { it != '0' }) {
            "$field must not be all zero"
        }
        return canonical
    }

    @JvmStatic
    fun requireCanonicalBase64(value: String, field: String): String {
        requireExactNonEmpty(value, field)
        val decoded = try {
            java.util.Base64.getDecoder().decode(value)
        } catch (ex: IllegalArgumentException) {
            throw IllegalArgumentException("$field must be canonical standard base64", ex)
        }
        require(decoded.isNotEmpty()) { "$field must encode at least one byte" }
        require(java.util.Base64.getEncoder().encodeToString(decoded) == value) {
            "$field must be canonical standard base64"
        }
        return value
    }

    @JvmStatic
    fun requireCanonicalStreamTokenBase64(value: String, field: String): String {
        requireExactNonEmpty(value, field)
        require(value.length <= MAX_STREAM_TOKEN_ENCODED_BYTES) {
            "$field must not exceed $MAX_STREAM_TOKEN_ENCODED_BYTES encoded bytes"
        }
        val decoded = try {
            java.util.Base64.getDecoder().decode(value)
        } catch (ex: IllegalArgumentException) {
            throw IllegalArgumentException("$field must be canonical standard base64", ex)
        }
        require(decoded.isNotEmpty() && decoded.size <= MAX_STREAM_TOKEN_DECODED_BYTES) {
            "$field must encode between 1 and $MAX_STREAM_TOKEN_DECODED_BYTES bytes"
        }
        require(java.util.Base64.getEncoder().encodeToString(decoded) == value) {
            "$field must be canonical standard base64"
        }
        return value
    }

    @JvmStatic
    fun requireCanonicalProviderName(value: String, field: String): String {
        requireExactNonEmpty(value, field)
        require(value.length <= MAX_PROVIDER_NAME_BYTES && value.all { char ->
            char in 'a'..'z' || char in 'A'..'Z' || char in '0'..'9' ||
                char == '.' || char == '_' || char == ':' || char == '-'
        }) {
            "$field must be 1-$MAX_PROVIDER_NAME_BYTES canonical ASCII bytes"
        }
        return value
    }

    @JvmStatic
    fun requireCanonicalGatewayBaseUrl(value: String, field: String): String {
        requireExactNonEmpty(value, field)
        require(value.length <= 2_048) { "$field must not exceed 2048 characters" }
        val uri = try {
            URI(value)
        } catch (ex: URISyntaxException) {
            throw IllegalArgumentException("$field must be a canonical HTTPS origin URL", ex)
        }
        require(uri.scheme == "https" && uri.isAbsolute) { "$field must use HTTPS" }
        require(uri.rawUserInfo == null) { "$field must not contain credentials" }
        require(uri.rawQuery == null && uri.rawFragment == null) {
            "$field must not contain a query or fragment"
        }
        require(uri.host != null) { "$field must contain a canonical host" }
        require(uri.host == uri.host.lowercase()) { "$field host must use canonical lowercase" }
        require(uri.port == -1) { "$field must omit the default HTTPS port" }
        require(uri.rawPath.isNullOrEmpty() || uri.rawPath == "/") {
            "$field must use the origin root path"
        }
        require(uri.toASCIIString() == value) {
            "$field must use exact canonical ASCII URL syntax"
        }
        require(isPublicGatewayHost(uri.host)) {
            "$field must target a canonical public host"
        }
        return value
    }

    @JvmStatic
    fun requireCanonicalGatewayBaseUri(value: URI, field: String): URI {
        requireCanonicalGatewayBaseUrl(value.toString(), field)
        return value
    }

    @JvmStatic
    fun requireCanonicalGatewayFetchPath(value: String, field: String): String {
        requireExactNonEmpty(value, field)
        require(value.length <= 1_024) { "$field must not exceed 1024 characters" }
        val uri = try {
            URI(value)
        } catch (ex: URISyntaxException) {
            throw IllegalArgumentException("$field must be a canonical relative path", ex)
        }
        require(!uri.isAbsolute && uri.rawAuthority == null) {
            "$field must be relative to the configured gateway origin"
        }
        require(uri.rawQuery == null && uri.rawFragment == null) {
            "$field must not contain a query or fragment"
        }
        require(uri.rawPath == value && value.startsWith('/') && value.length > 1) {
            "$field must be an absolute-path reference"
        }
        require('%' !in value && "//" !in value && !value.endsWith('/')) {
            "$field must not contain encoding ambiguity or empty path segments"
        }
        require(value.split('/').drop(1).none { it == "." || it == ".." || it.isEmpty() }) {
            "$field must not contain dot or empty path segments"
        }
        require(value.all { char ->
            char == '/' || char == '-' || char == '_' ||
                char in 'a'..'z' || char in '0'..'9'
        }) {
            "$field must use canonical lowercase ASCII path characters"
        }
        return value
    }

    @JvmStatic
    fun requireCanonicalRolloutPhase(value: String, field: String): String {
        requireExactNonEmpty(value, field)
        require(value == "canary" || value == "ramp" || value == "default") {
            "$field must be one of canary, ramp, or default"
        }
        return value
    }

    @JvmStatic
    fun requireCanonicalChunkerHandle(value: String, field: String): String {
        requireExactNonEmpty(value, field)
        val at = value.indexOf('@')
        require(at > 0 && at == value.lastIndexOf('@') && at < value.lastIndex) {
            chunkerHandleLabel(field)
        }
        val identity = value.substring(0, at)
        val separator = identity.indexOf('.')
        require(
            separator > 0 &&
                separator == identity.lastIndexOf('.') &&
                separator < identity.lastIndex,
        ) {
            chunkerHandleLabel(field)
        }
        require(identity.substring(0, separator).isCanonicalHandleToken()) {
            chunkerHandleLabel(field)
        }
        require(identity.substring(separator + 1).isCanonicalHandleToken()) {
            chunkerHandleLabel(field)
        }
        val versionParts = value.substring(at + 1).split('.')
        require(versionParts.size == 3 && versionParts.all { it.isCanonicalDecimalComponent() }) {
            chunkerHandleLabel(field)
        }
        return value
    }

    private fun String.isCanonicalHandleToken(): Boolean {
        if (isEmpty() || first() !in 'a'..'z' || last() == '-') return false
        return all { it in 'a'..'z' || it in '0'..'9' || it == '-' }
    }

    private fun String.isCanonicalDecimalComponent(): Boolean {
        if (isEmpty() || any { it !in '0'..'9' }) return false
        return length == 1 || first() != '0'
    }

    private fun Char.isBoundaryWhitespace(): Boolean =
        isWhitespace() || java.lang.Character.isSpaceChar(this)

    private fun Char.isLowerHexDigit(): Boolean =
        this in '0'..'9' || this in 'a'..'f'

    private fun isPublicGatewayHost(rawHost: String): Boolean {
        val host = if (rawHost.startsWith('[') && rawHost.endsWith(']')) {
            rawHost.substring(1, rawHost.length - 1)
        } else {
            rawHost
        }
        if (host.isEmpty() || host != host.lowercase()) return false

        parseCanonicalIpv4(host)?.let { return isPublicIpv4(it) }
        if (host.all { it in '0'..'9' || it == '.' }) return false

        if (':' in host) {
            val bytes = try {
                InetAddress.getByName(host).address
            } catch (_: RuntimeException) {
                return false
            }
            return when (bytes.size) {
                4 -> isPublicIpv4(bytes.map { it.toInt() and 0xff })
                16 -> isPublicIpv6(bytes)
                else -> false
            }
        }

        if (host == "localhost" || host.endsWith(".localhost") ||
            host.endsWith(".local") || host.endsWith(".internal") || host.endsWith(".lan")
        ) {
            return false
        }
        if (host.length > 253 || host.endsWith('.')) return false
        return host.split('.').all { label ->
            label.isNotEmpty() && label.length <= 63 &&
                label.first().isAsciiLowerAlphanumeric() &&
                label.last().isAsciiLowerAlphanumeric() &&
                label.all { it.isAsciiLowerAlphanumeric() || it == '-' }
        }
    }

    private fun parseCanonicalIpv4(host: String): List<Int>? {
        val parts = host.split('.')
        if (parts.size != 4) return null
        val octets = ArrayList<Int>(4)
        for (part in parts) {
            if (part.isEmpty() || part.any { it !in '0'..'9' } ||
                (part.length > 1 && part.first() == '0')
            ) {
                return null
            }
            val octet = part.toIntOrNull() ?: return null
            if (octet !in 0..255) return null
            octets.add(octet)
        }
        return octets
    }

    private fun isPublicIpv4(octets: List<Int>): Boolean {
        if (octets.size != 4) return false
        val first = octets[0]
        val second = octets[1]
        val third = octets[2]
        val fourth = octets[3]
        return first != 0 && first != 10 && first != 127 && first < 224 &&
            !(first == 100 && second in 64..127) &&
            !(first == 169 && second == 254) &&
            !(first == 172 && second in 16..31) &&
            !(first == 192 && second == 0 && third == 0) &&
            !(first == 192 && second == 0 && third == 2) &&
            !(first == 192 && second == 88 && third == 99) &&
            !(first == 192 && second == 168) &&
            !(first == 198 && second in 18..19) &&
            !(first == 198 && second == 51 && third == 100) &&
            !(first == 203 && second == 0 && third == 113) &&
            !(first == 255 && second == 255 && third == 255 && fourth == 255)
    }

    private fun isPublicIpv6(bytes: ByteArray): Boolean {
        if (bytes.size != 16) return false
        val first = ((bytes[0].toInt() and 0xff) shl 8) or (bytes[1].toInt() and 0xff)
        val second = ((bytes[2].toInt() and 0xff) shl 8) or (bytes[3].toInt() and 0xff)
        val globalUnicast = first and 0xe000 == 0x2000
        val documentation = (first == 0x2001 && second == 0x0db8) ||
            (first == 0x3fff && second and 0xf000 == 0)
        val specialPurpose = first == 0x2001 && second <= 0x01ff
        return globalUnicast && !documentation && !specialPurpose && first != 0x2002
    }

    private fun Char.isAsciiLowerAlphanumeric(): Boolean =
        this in 'a'..'z' || this in '0'..'9'

    private fun chunkerHandleLabel(field: String): String =
        "$field must be a canonical chunker handle (namespace.name@major.minor.patch)"
}

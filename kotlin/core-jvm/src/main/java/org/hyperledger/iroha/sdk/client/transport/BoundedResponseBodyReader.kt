package org.hyperledger.iroha.sdk.client.transport

import java.io.ByteArrayOutputStream
import java.io.IOException
import java.io.InputStream
import java.net.HttpURLConnection

/** Shared framing validation and decoded-body bounds for the canonical HTTP adapter. */
internal object BoundedResponseBodyReader {
    fun read(input: InputStream?, headers: Map<out String?, List<String>?>?, method: String, status: Int, maximum: Long): ByteArray {
        val declaredLength = canonicalContentLength(headers)
        rejectAmbiguousFraming(headers, declaredLength)
        if (responseMustNotHaveBody(method, status)) return ByteArray(0)
        val bufferedLength = contentLengthForBufferedBody(headers, declaredLength)
        if (bufferedLength != null && bufferedLength > maximum) {
            throw IOException("HTTP response Content-Length $bufferedLength exceeds the $maximum-byte limit")
        }
        if (input == null) {
            if (declaredLength != null && declaredLength != 0L) {
                throw IOException("HTTP response ended before its $declaredLength-byte Content-Length")
            }
            return ByteArray(0)
        }
        return input.use { readBoundedBody(it, maximum, bufferedLength) }
    }

    internal fun readBoundedBody(
        input: InputStream,
        maximumResponseBytes: Long,
        declaredLength: Long?,
    ): ByteArray {
        require(maximumResponseBytes in 1..Int.MAX_VALUE.toLong()) {
            "maximumResponseBytes must be between 1 and ${Int.MAX_VALUE}"
        }
        val initialCapacity = minOf(declaredLength ?: 32L, maximumResponseBytes, 8192L).toInt()
        ByteArrayOutputStream(initialCapacity).use { buffer ->
            val chunk = ByteArray(8192)
            var total = 0L
            while (true) {
                val remaining = maximumResponseBytes - total
                val requested = minOf(chunk.size.toLong(), remaining + 1L).toInt()
                val read = input.read(chunk, 0, requested)
                if (read == -1) break
                if (read < -1 || read > requested) {
                    throw IOException("HTTP response body stream returned an invalid read count")
                }
                if (read == 0) {
                    throw IOException("HTTP response body stream made no read progress")
                }
                if (read.toLong() > remaining) {
                    throw IOException(
                        "HTTP response body exceeds the $maximumResponseBytes-byte limit",
                    )
                }
                buffer.write(chunk, 0, read)
                total += read.toLong()
            }
            if (declaredLength != null && total != declaredLength) {
                throw IOException(
                    "HTTP response body length $total does not match Content-Length " +
                        declaredLength,
                )
            }
            return buffer.toByteArray()
        }
    }

    fun canonicalContentLength(raw: Map<out String?, List<String>?>?): Long? {
        if (raw == null) return null
        var value: String? = null
        for ((name, values) in raw) {
            if (name == null || !name.equals("Content-Length", ignoreCase = true)) continue
            if (value != null || values == null || values.size != 1) {
                throw IOException(
                    "HTTP response must contain at most one Content-Length value",
                )
            }
            value = values[0]
        }
        return value?.let(::parseCanonicalContentLength)
    }

    internal fun parseCanonicalContentLength(value: String): Long {
        if (value.isEmpty() || (value.length > 1 && value[0] == '0')) {
            throw IOException(
                "HTTP response Content-Length must be a canonical unsigned decimal",
            )
        }
        var parsed = 0L
        for (character in value) {
            if (character !in '0'..'9') {
                throw IOException(
                    "HTTP response Content-Length must be a canonical unsigned decimal",
                )
            }
            val digit = character.code - '0'.code
            if (parsed > (Long.MAX_VALUE - digit) / 10L) {
                throw IOException("HTTP response Content-Length exceeds the supported range")
            }
            parsed = parsed * 10L + digit
        }
        return parsed
    }

    internal fun contentLengthForBufferedBody(
        raw: Map<out String?, List<String>?>?,
        declaredLength: Long?,
    ): Long? {
        if (declaredLength == null || raw == null) return declaredLength
        for ((name, values) in raw) {
            if (name == null || !name.equals("Content-Encoding", ignoreCase = true)) continue
            if (values == null || values.isEmpty()) return null
            for (value in values) {
                val encodings = value.split(',')
                if (encodings.any { !it.trim().equals("identity", ignoreCase = true) }) {
                    return null
                }
            }
        }
        return declaredLength
    }

    fun rejectAmbiguousFraming(
        raw: Map<out String?, List<String>?>?,
        declaredLength: Long?,
    ) {
        if (declaredLength == null || raw == null) return
        val hasTransferEncoding = raw.keys.any { name ->
            name != null && name.equals("Transfer-Encoding", ignoreCase = true)
        }
        if (hasTransferEncoding) {
            throw IOException(
                "HTTP response must not combine Content-Length with Transfer-Encoding",
            )
        }
    }

    fun responseMustNotHaveBody(requestMethod: String, status: Int): Boolean =
        requestMethod.equals("HEAD", ignoreCase = true) ||
            status in 100..199 ||
            status == HttpURLConnection.HTTP_NO_CONTENT ||
            status == HttpURLConnection.HTTP_NOT_MODIFIED

}

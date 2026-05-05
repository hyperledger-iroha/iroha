package org.hyperledger.iroha.sdk.client

import java.nio.charset.StandardCharsets

/** Helpers for extracting stable HTTP error details from Torii responses. */
internal object HttpErrorMessageExtractor {

    private const val MAX_MESSAGE_LENGTH = 512

    @JvmStatic
    fun extractRejectCode(headers: Map<String, List<String>>?, headerName: String?): String? {
        if (headers.isNullOrEmpty() || headerName.isNullOrBlank()) return null
        for ((key, values) in headers) {
            if (key == null || !key.equals(headerName, ignoreCase = true)) continue
            val value = firstNonBlank(values)
            if (value != null) return value
        }
        return null
    }

    @JvmStatic
    fun extractMessage(body: ByteArray?): String? {
        if (body == null || body.isEmpty()) return null
        val text = String(body, StandardCharsets.UTF_8).trim()
        if (text.isEmpty()) return null

        try {
            val parsed = JsonParser.parse(text)
            val extracted = extractStructuredMessage(parsed)
            if (extracted != null) return truncate(extracted)
            val compact = compactJsonSorted(parsed)
            if (compact != null) return truncate(compact)
        } catch (_: RuntimeException) {
        }

        return truncate(text)
    }

    private fun extractStructuredMessage(value: Any?): String? {
        if (value is String) {
            val text = value.trim()
            return if (text.isEmpty()) null else text
        }
        if (value is List<*>) {
            for (entry in value) {
                val nested = extractStructuredMessage(entry)
                if (nested != null) return nested
            }
            return null
        }
        if (value !is Map<*, *>) return null
        val candidateKeys = arrayOf(
            "message", "error", "errors", "detail", "details", "reason", "rejection_reason", "description"
        )
        for (key in candidateKeys) {
            val nestedValue = getCaseInsensitiveValue(value, key) ?: continue
            val nested = extractStructuredMessage(nestedValue)
            if (nested != null) return nested
        }
        return null
    }

    private fun getCaseInsensitiveValue(map: Map<*, *>, candidateKey: String): Any? {
        if (map.containsKey(candidateKey)) return map[candidateKey]
        for ((rawKey, v) in map) {
            if (rawKey is String && rawKey.equals(candidateKey, ignoreCase = true)) return v
        }
        return null
    }

    private fun compactJsonSorted(value: Any?): String? {
        val builder = StringBuilder()
        appendJsonValueSorted(value, builder)
        val text = builder.toString().trim()
        return if (text.isEmpty()) null else text
    }

    private fun appendJsonValueSorted(value: Any?, builder: StringBuilder) {
        when {
            value == null -> builder.append("null")
            value is String -> appendJsonString(value, builder)
            value is Boolean || value is Int || value is Long -> builder.append(value)
            value is Number -> builder.append(value.toString())
            value is List<*> -> {
                builder.append('[')
                var first = true
                for (entry in value) {
                    if (!first) builder.append(',')
                    first = false
                    appendJsonValueSorted(entry, builder)
                }
                builder.append(']')
            }
            value is Map<*, *> -> {
                val keys = ArrayList<String>()
                for (rawKey in value.keys) {
                    if (rawKey != null) keys.add(rawKey.toString())
                }
                keys.sort()
                builder.append('{')
                var first = true
                for (key in keys) {
                    if (!first) builder.append(',')
                    first = false
                    appendJsonString(key, builder)
                    builder.append(':')
                    appendJsonValueSorted(value[key], builder)
                }
                builder.append('}')
            }
            else -> appendJsonString(value.toString(), builder)
        }
    }

    private fun appendJsonString(text: String, builder: StringBuilder) {
        builder.append('"')
        for (ch in text) {
            when (ch) {
                '"' -> builder.append("\\\"")
                '\\' -> builder.append("\\\\")
                '\b' -> builder.append("\\b")
                '\u000C' -> builder.append("\\f")
                '\n' -> builder.append("\\n")
                '\r' -> builder.append("\\r")
                '\t' -> builder.append("\\t")
                else -> {
                    if (ch < '\u0020') {
                        builder.append(String.format("\\u%04x", ch.code))
                    } else {
                        builder.append(ch)
                    }
                }
            }
        }
        builder.append('"')
    }

    private fun truncate(text: String?): String? {
        val normalized = text?.trim() ?: ""
        if (normalized.isEmpty()) return null
        if (normalized.length > MAX_MESSAGE_LENGTH) {
            return normalized.substring(0, MAX_MESSAGE_LENGTH) + "..."
        }
        return normalized
    }

    private fun firstNonBlank(values: List<String>?): String? {
        if (values.isNullOrEmpty()) return null
        for (value in values) {
            if (value.isNotBlank()) return value.trim()
        }
        return null
    }
}

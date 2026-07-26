package org.hyperledger.iroha.sdk.client

import java.nio.charset.StandardCharsets
import java.util.Optional
import org.hyperledger.iroha.sdk.norito.NoritoAdapters
import org.hyperledger.iroha.sdk.norito.NoritoCodec
import org.hyperledger.iroha.sdk.norito.NoritoDecoder
import org.hyperledger.iroha.sdk.norito.NoritoEncoder
import org.hyperledger.iroha.sdk.norito.NoritoHeader
import org.hyperledger.iroha.sdk.norito.TypeAdapter

/** Helpers for extracting stable HTTP error details from Torii responses. */
internal object HttpErrorMessageExtractor {

    private const val MAX_MESSAGE_LENGTH = 512
    private val STRING_ADAPTER = NoritoAdapters.stringAdapter()
    private val DETAILS_ADAPTER = object : TypeAdapter<ErrorDetailsSummary> {
        override fun encode(encoder: NoritoEncoder, value: ErrorDetailsSummary) {
            NoritoAdapters.option(STRING_ADAPTER).encode(
                encoder,
                Optional.ofNullable(value.rejectCode),
            )
        }

        override fun decode(decoder: NoritoDecoder): ErrorDetailsSummary {
            val rejectCode = decodeRejectCodeField(decoder)
            if (decoder.remaining() > 0) decoder.readBytes(decoder.remaining())
            return ErrorDetailsSummary(rejectCode)
        }
    }
    private val ERROR_ENVELOPE_ADAPTER = NoritoAdapters.struct(
        listOf(
            NoritoAdapters.field("code", STRING_ADAPTER),
            NoritoAdapters.field("message", STRING_ADAPTER),
            NoritoAdapters.field("details", NoritoAdapters.option(DETAILS_ADAPTER)),
        )
    ) { fields ->
        val details = fields["details"] as Optional<*>
        val summary = details.orElse(null) as? ErrorDetailsSummary
        ErrorEnvelopeSummary(
            fields["code"] as String,
            fields["message"] as String,
            summary?.rejectCode,
        )
    }

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
    fun extractRejectCode(
        headers: Map<String, List<String>>?,
        headerName: String?,
        body: ByteArray?,
    ): String? {
        val fromHeader = extractRejectCode(headers, headerName)
        if (fromHeader != null) return fromHeader
        if (body == null || body.isEmpty()) return null
        decodeNoritoErrorEnvelope(body)?.rejectCode?.let { return it }
        val text = String(body, StandardCharsets.UTF_8).trim()
        if (text.isEmpty()) return null
        return try {
            extractStructuredRejectCode(JsonParser.parse(text))
        } catch (_: RuntimeException) {
            null
        }
    }

    @JvmStatic
    fun extractMessage(body: ByteArray?): String? {
        if (body == null || body.isEmpty()) return null
        decodeNoritoErrorEnvelope(body)?.message?.let { return truncate(it) }
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

    private fun decodeNoritoErrorEnvelope(body: ByteArray): ErrorEnvelopeSummary? {
        if (!hasNoritoMagic(body)) return null
        return try {
            NoritoCodec.decode(body, ERROR_ENVELOPE_ADAPTER, null) as ErrorEnvelopeSummary
        } catch (_: RuntimeException) {
            null
        }
    }

    private fun hasNoritoMagic(body: ByteArray): Boolean =
        body.size >= 4 &&
            body[0] == 'N'.code.toByte() &&
            body[1] == 'R'.code.toByte() &&
            body[2] == 'T'.code.toByte() &&
            body[3] == '0'.code.toByte()

    private fun decodeRejectCodeField(decoder: NoritoDecoder): String? {
        val optionalString = NoritoAdapters.option(STRING_ADAPTER)
        if ((decoder.flags and NoritoHeader.PACKED_STRUCT) != 0 &&
            (decoder.flags and NoritoHeader.FIELD_BITSET) != 0
        ) {
            val fieldCount = 5
            val bitsetData = decoder.readBytes((fieldCount + 7) / 8)
            var bitset = 0
            for (i in bitsetData.indices) {
                bitset = bitset or ((bitsetData[i].toInt() and 0xFF) shl (i * 8))
            }
            val encodedSizes = ArrayList<Int?>(fieldCount)
            for (i in 0 until fieldCount) {
                if ((bitset and (1 shl i)) != 0) {
                    val size = decoder.readVarint()
                    require(size <= Int.MAX_VALUE) { "Packed field too large" }
                    encodedSizes.add(size.toInt())
                } else {
                    encodedSizes.add(null)
                }
            }
            val firstSize = encodedSizes[0]
            return if (firstSize != null) {
                val child = NoritoDecoder(decoder.readBytes(firstSize), decoder.flags, decoder.flagsHint)
                val value = optionalString.decode(child)
                require(child.remaining() == 0) { "Packed reject_code field did not consume all bytes" }
                value.orElse(null)
            } else {
                optionalString.decode(decoder).orElse(null)
            }
        }
        return optionalString.decode(decoder).orElse(null)
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

    private fun extractStructuredRejectCode(value: Any?): String? {
        if (value is List<*>) {
            for (entry in value) {
                val nested = extractStructuredRejectCode(entry)
                if (nested != null) return nested
            }
            return null
        }
        if (value !is Map<*, *>) return null
        for (key in arrayOf("reject_code", "rejectCode")) {
            val direct = coerceNonBlankString(getCaseInsensitiveValue(value, key))
            if (direct != null) return direct
        }
        val details = getCaseInsensitiveValue(value, "details")
        if (details is Map<*, *>) {
            for (key in arrayOf("reject_code", "rejectCode")) {
                val nested = coerceNonBlankString(getCaseInsensitiveValue(details, key))
                if (nested != null) return nested
            }
            val axt = getCaseInsensitiveValue(details, "axt")
            if (axt is Map<*, *>) {
                val axtCode = coerceNonBlankString(getCaseInsensitiveValue(axt, "code"))
                if (axtCode != null) return axtCode
            }
        }
        return null
    }

    private fun coerceNonBlankString(value: Any?): String? {
        val text = value?.toString()?.trim() ?: return null
        return if (text.isEmpty()) null else text
    }

    private data class ErrorEnvelopeSummary(
        val code: String,
        val message: String,
        val rejectCode: String?,
    )

    private data class ErrorDetailsSummary(val rejectCode: String?)

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

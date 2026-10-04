package org.hyperledger.iroha.sdk.client

import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.json.Json
import org.hyperledger.iroha.sdk.json.JsonObject
import org.hyperledger.iroha.sdk.json.JsonString

/**
 * An error response from Torii.
 *
 * Torii answers failures with the standard envelope `{"code": ..., "message": ..., "details": {...}}`
 * (JSON, or the equivalent Norito frame); [code], [message] and [details] carry it. Collection
 * query errors use the `invalid_filter`, `invalid_sort`, `invalid_select`, `invalid_aggregate`,
 * `invalid_limit`, `invalid_cursor`, `invalid_include_total` and `invalid_query` codes, with
 * `details.field` naming the control ([field]) and `details.hint` suggesting a fix ([hint]).
 * [rejectCode] is the `x-iroha-reject-code` header (or `details.reject_code`) when present.
 *
 * [code] is `null` only when the response carried no envelope; [message] then describes the
 * HTTP status and a bounded excerpt of the body.
 */
open class ToriiApiException @JvmOverloads constructor(
    /** HTTP status code. */
    @JvmField val status: Int,
    /** Stable machine-readable error code from the envelope. */
    @JvmField val code: String?,
    message: String,
    /** Structured `details` from the envelope, when present. */
    @JvmField val details: JsonObject?,
    /** Admission reject code from `x-iroha-reject-code` or `details.reject_code`. */
    @JvmField val rejectCode: String?,
    cause: Throwable? = null,
) : RuntimeException(message, cause) {
    /** The control or data field at fault (`details.field`), for query errors. */
    val field: String? get() = detail("field")

    /** A suggested fix (`details.hint`, e.g. the closest field name), when Torii provides one. */
    val hint: String? get() = detail("hint")

    /** The offending value or field (`details.actual`), when present. */
    val actual: String? get() = detail("actual")

    /** The accepted fields or values (`details.expected`, a comma-separated list on the wire). */
    val expected: List<String>
        get() = detail("expected")?.split(',')?.map { it.trim() }?.filter { it.isNotEmpty() } ?: emptyList()

    /** A string member of [details], or `null`. */
    fun detail(name: String): String? = (details?.get(name) as? JsonString)?.value

    override fun toString(): String = buildString {
        append(javaClass.simpleName).append("(status=").append(status)
        code?.let { append(", code=").append(it) }
        rejectCode?.let { append(", rejectCode=").append(it) }
        append("): ").append(message)
    }

    companion object {
        private const val REJECT_CODE_HEADER = "x-iroha-reject-code"
        private const val MAX_EXCERPT = 512

        /**
         * Decode an error response. Bodies without a recognisable envelope still produce an
         * exception with a `null` [code].
         */
        @JvmStatic
        fun fromResponse(status: Int, headers: Map<String, List<String>>?, body: ByteArray?): ToriiApiException =
            fromResponse(status, headers, body, null)

        /** As [fromResponse], prefixing the message with "[context] request failed with status N". */
        internal fun fromResponse(
            status: Int,
            headers: Map<String, List<String>>?,
            body: ByteArray?,
            context: String?,
        ): ToriiApiException {
            val envelope = ToriiErrorEnvelope.decode(body)
            val headerRejectCode = HttpErrorMessageExtractor.extractRejectCode(headers, REJECT_CODE_HEADER)
            val rejectCode = headerRejectCode ?: envelope?.rejectCode
            val serverMessage = envelope?.message ?: bodyExcerpt(body)
            val message = when {
                context == null -> serverMessage ?: "Torii returned HTTP $status"
                serverMessage == null -> "$context request failed with status $status"
                else -> "$context request failed with status $status: $serverMessage"
            }
            return ToriiApiException(status, envelope?.code, message, envelope?.details, rejectCode)
        }

        private fun bodyExcerpt(body: ByteArray?): String? {
            val text = body?.let { String(it, StandardCharsets.UTF_8).trim() }.orEmpty()
            if (text.isEmpty()) return null
            return if (text.length > MAX_EXCERPT) text.substring(0, MAX_EXCERPT) + "..." else text
        }
    }
}

/** The standard Torii error envelope `{code, message, details?}`. */
internal class ToriiErrorEnvelope(
    val code: String,
    val message: String,
    val details: JsonObject?,
    val rejectCode: String?,
) {
    companion object {
        fun decode(body: ByteArray?): ToriiErrorEnvelope? {
            if (body == null || body.isEmpty()) return null
            HttpErrorMessageExtractor.decodeNoritoEnvelope(body)?.let { (code, message, rejectCode) ->
                return ToriiErrorEnvelope(code, message, null, rejectCode)
            }
            val parsed = try {
                Json.parse(body)
            } catch (_: IllegalArgumentException) {
                return null
            }
            if (parsed !is JsonObject) return null
            val code = (parsed["code"] as? JsonString)?.value ?: return null
            val message = (parsed["message"] as? JsonString)?.value ?: return null
            val details = parsed["details"] as? JsonObject
            val rejectCode = (details?.get("reject_code") as? JsonString)?.value
            return ToriiErrorEnvelope(code, message, details, rejectCode)
        }
    }
}

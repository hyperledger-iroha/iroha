package org.hyperledger.iroha.sdk.client.stream

import java.math.BigInteger
import org.hyperledger.iroha.sdk.client.JsonParser

/** A terminal error reported after a canonical Torii SSE stream has started. */
class ToriiStreamException internal constructor(
    /** Stable machine-readable error code supplied by Torii. */
    @JvmField val code: String,
    /** Human-readable error message supplied by Torii. */
    @JvmField val serverMessage: String,
    /** Number of broadcast messages skipped before termination, when reported. */
    @JvmField val droppedMessages: BigInteger?,
    /** Whether Torii can replay the missing portion of this stream. */
    @JvmField val replayAvailable: Boolean,
    /** Unmodified JSON data carried by the terminal SSE frame. */
    @JvmField val rawData: String,
) : RuntimeException("$code: $serverMessage")

/** A malformed terminal `stream_error` frame that cannot be interpreted safely. */
class ToriiStreamProtocolException internal constructor(
    /** Stable explanation of the protocol violation. */
    @JvmField val reason: String,
    /** Unmodified data carried by the malformed SSE frame. */
    @JvmField val rawData: String,
    cause: Throwable? = null,
) : RuntimeException("Torii emitted a malformed stream_error event: $reason", cause)

internal object ToriiStreamErrorParser {
    private val expectedKeys = setOf("code", "message", "dropped_messages", "replay_available")
    private val maxU64 = BigInteger.ONE.shiftLeft(64).subtract(BigInteger.ONE)

    fun parse(rawData: String): ToriiStreamException {
        val parsed = try {
            JsonParser.parse(rawData)
        } catch (error: IllegalStateException) {
            throw ToriiStreamProtocolException(
                "data must be a valid JSON object without duplicate keys",
                rawData,
                error,
            )
        }
        @Suppress("UNCHECKED_CAST")
        val payload = parsed as? Map<String, Any?>
            ?: malformed("data must be a JSON object", rawData)
        if (payload.keys != expectedKeys) {
            malformed(
                "data must contain exactly code, message, dropped_messages, and replay_available",
                rawData,
            )
        }

        val code = exactText(payload["code"], "code", token = true, rawData)
        val message = exactText(payload["message"], "message", token = false, rawData)
        val droppedMessages = unsignedIntegerOrNull(payload["dropped_messages"], rawData)
        val replayAvailable = payload["replay_available"] as? Boolean
            ?: malformed("replay_available must be a boolean", rawData)
        return ToriiStreamException(
            code,
            message,
            droppedMessages,
            replayAvailable,
            rawData,
        )
    }

    private fun exactText(value: Any?, property: String, token: Boolean, rawData: String): String {
        val text = value as? String
            ?: malformed("$property must be a string", rawData)
        val hasSurroundingWhitespace = text.trim { it.isWhitespace() } != text
        val hasControl = text.any { Character.isISOControl(it.code) }
        val hasTokenWhitespace = token && text.any { it.isWhitespace() }
        if (
            text.isEmpty() ||
            hasSurroundingWhitespace ||
            hasControl ||
            hasTokenWhitespace ||
            hasUnpairedSurrogate(text)
        ) {
            val shape = if (token) "a non-empty exact token" else "non-empty exact text"
            malformed("$property must be $shape", rawData)
        }
        return text
    }

    private fun hasUnpairedSurrogate(text: String): Boolean {
        var index = 0
        while (index < text.length) {
            val current = text[index]
            when {
                Character.isHighSurrogate(current) -> {
                    if (index + 1 >= text.length || !Character.isLowSurrogate(text[index + 1])) {
                        return true
                    }
                    index += 2
                }
                Character.isLowSurrogate(current) -> return true
                else -> index++
            }
        }
        return false
    }

    private fun unsignedIntegerOrNull(value: Any?, rawData: String): BigInteger? {
        if (value == null) return null
        val integer = when (value) {
            is BigInteger -> value
            is Byte -> BigInteger.valueOf(value.toLong())
            is Short -> BigInteger.valueOf(value.toLong())
            is Int -> BigInteger.valueOf(value.toLong())
            is Long -> BigInteger.valueOf(value)
            else -> malformed("dropped_messages must be null or an unsigned integer", rawData)
        }
        if (integer.signum() < 0 || integer > maxU64) {
            malformed("dropped_messages must be null or an unsigned 64-bit integer", rawData)
        }
        return integer
    }

    private fun malformed(reason: String, rawData: String): Nothing =
        throw ToriiStreamProtocolException(reason, rawData)
}

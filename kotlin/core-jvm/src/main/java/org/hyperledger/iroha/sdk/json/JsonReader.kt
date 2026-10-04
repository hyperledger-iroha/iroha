package org.hyperledger.iroha.sdk.json

import java.nio.ByteBuffer
import java.nio.charset.CharacterCodingException
import java.nio.charset.CodingErrorAction
import java.nio.charset.StandardCharsets

/** Strict single-pass JSON reader producing [Json] trees with exact numbers. */
internal class JsonReader(private val input: String) {
    private var index = 0

    fun readDocument(): Json {
        skipWhitespace()
        val value = readValue(0)
        skipWhitespace()
        if (index != input.length) fail("unexpected trailing characters after the JSON value")
        return value
    }

    private fun readValue(depth: Int): Json {
        if (depth > Json.MAX_DEPTH) fail("JSON nests deeper than ${Json.MAX_DEPTH} levels")
        if (index >= input.length) fail("unexpected end of JSON input")
        return when (val ch = input[index]) {
            '{' -> readObject(depth)
            '[' -> readArray(depth)
            '"' -> JsonString(readString())
            't' -> { literal("true"); JsonBoolean.TRUE }
            'f' -> { literal("false"); JsonBoolean.FALSE }
            'n' -> { literal("null"); JsonNull }
            else -> if (ch == '-' || ch in '0'..'9') readNumber() else fail("unexpected character `$ch`")
        }
    }

    private fun readObject(depth: Int): JsonObject {
        index++
        val members = LinkedHashMap<String, Json>()
        skipWhitespace()
        if (peek() == '}') {
            index++
            return JsonObject(members)
        }
        while (true) {
            skipWhitespace()
            if (peek() != '"') fail("expected a member name")
            val start = index
            val name = readString()
            if (members.containsKey(name)) {
                index = start
                fail("duplicate member `$name`")
            }
            skipWhitespace()
            expect(':')
            skipWhitespace()
            members[name] = readValue(depth + 1)
            skipWhitespace()
            when (peek()) {
                ',' -> index++
                '}' -> {
                    index++
                    return JsonObject(members)
                }
                else -> fail("expected `,` or `}` in an object")
            }
        }
    }

    private fun readArray(depth: Int): JsonArray {
        index++
        val items = ArrayList<Json>()
        skipWhitespace()
        if (peek() == ']') {
            index++
            return JsonArray(items)
        }
        while (true) {
            skipWhitespace()
            items.add(readValue(depth + 1))
            skipWhitespace()
            when (peek()) {
                ',' -> index++
                ']' -> {
                    index++
                    return JsonArray(items)
                }
                else -> fail("expected `,` or `]` in an array")
            }
        }
    }

    private fun readString(): String {
        expect('"')
        val out = StringBuilder()
        while (true) {
            if (index >= input.length) fail("unterminated string")
            val ch = input[index++]
            when {
                ch == '"' -> return out.toString()
                ch == '\\' -> {
                    if (index >= input.length) fail("unterminated escape sequence")
                    when (val escaped = input[index++]) {
                        '"' -> out.append('"')
                        '\\' -> out.append('\\')
                        '/' -> out.append('/')
                        'b' -> out.append('\b')
                        'f' -> out.append('\u000C')
                        'n' -> out.append('\n')
                        'r' -> out.append('\r')
                        't' -> out.append('\t')
                        'u' -> {
                            val unit = readHexUnit()
                            if (Character.isHighSurrogate(unit)) {
                                if (index + 2 > input.length || input[index] != '\\' || input[index + 1] != 'u') {
                                    fail("unpaired UTF-16 surrogate escape")
                                }
                                index += 2
                                val low = readHexUnit()
                                if (!Character.isLowSurrogate(low)) fail("unpaired UTF-16 surrogate escape")
                                out.append(unit).append(low)
                            } else {
                                if (Character.isLowSurrogate(unit)) fail("unpaired UTF-16 surrogate escape")
                                out.append(unit)
                            }
                        }
                        else -> fail("unknown escape sequence `\\$escaped`")
                    }
                }
                ch < ' ' -> fail("unescaped control character in string")
                Character.isHighSurrogate(ch) -> {
                    if (index >= input.length || !Character.isLowSurrogate(input[index])) {
                        fail("unpaired UTF-16 surrogate in string")
                    }
                    out.append(ch).append(input[index++])
                }
                Character.isLowSurrogate(ch) -> fail("unpaired UTF-16 surrogate in string")
                else -> out.append(ch)
            }
        }
    }

    private fun readHexUnit(): Char {
        if (index + 4 > input.length) fail("invalid `\\u` escape")
        var value = 0
        for (offset in 0 until 4) {
            val digit = when (val ch = input[index + offset]) {
                in '0'..'9' -> ch - '0'
                in 'a'..'f' -> ch - 'a' + 10
                in 'A'..'F' -> ch - 'A' + 10
                else -> fail("invalid `\\u` escape")
            }
            value = (value shl 4) or digit
        }
        index += 4
        return value.toChar()
    }

    private fun readNumber(): JsonNumber {
        val start = index
        val end = numberEnd(input, start)
        if (end < 0) fail("invalid number")
        index = end
        return JsonNumber.trusted(input.substring(start, end))
    }

    private fun literal(word: String) {
        if (!input.startsWith(word, index)) fail("unexpected token")
        index += word.length
    }

    private fun skipWhitespace() {
        while (index < input.length) {
            when (input[index]) {
                ' ', '\t', '\n', '\r' -> index++
                else -> return
            }
        }
    }

    private fun peek(): Char = if (index < input.length) input[index] else '\u0000'

    private fun expect(expected: Char) {
        if (index >= input.length || input[index] != expected) fail("expected `$expected`")
        index++
    }

    private fun fail(reason: String): Nothing = throw JsonSyntaxException(reason, index)

    companion object {
        /** Decode [bytes] as UTF-8, rejecting malformed sequences instead of substituting. */
        fun decodeUtf8(bytes: ByteArray): String = try {
            StandardCharsets.UTF_8.newDecoder()
                .onMalformedInput(CodingErrorAction.REPORT)
                .onUnmappableCharacter(CodingErrorAction.REPORT)
                .decode(ByteBuffer.wrap(bytes))
                .toString()
        } catch (error: CharacterCodingException) {
            throw JsonSyntaxException("JSON text is not valid UTF-8", 0)
        }

        fun isNumberSpelling(text: String): Boolean = numberEnd(text, 0) == text.length

        /** End offset of the JSON number starting at [start], or -1 when malformed. */
        private fun numberEnd(text: String, start: Int): Int {
            var position = start
            if (position < text.length && text[position] == '-') position++
            if (position >= text.length) return -1
            when (text[position]) {
                '0' -> position++
                in '1'..'9' -> while (position < text.length && text[position] in '0'..'9') position++
                else -> return -1
            }
            if (position < text.length && text[position] == '.') {
                position++
                val digits = position
                while (position < text.length && text[position] in '0'..'9') position++
                if (position == digits) return -1
            }
            if (position < text.length && (text[position] == 'e' || text[position] == 'E')) {
                position++
                if (position < text.length && (text[position] == '+' || text[position] == '-')) position++
                val digits = position
                while (position < text.length && text[position] in '0'..'9') position++
                if (position == digits) return -1
            }
            return position
        }
    }
}

/** Compact JSON writer using Norito's canonical string escapes. */
internal object JsonWriter {
    private val HEX = "0123456789abcdef".toCharArray()

    fun write(value: Json, out: StringBuilder) {
        when (value) {
            JsonNull -> out.append("null")
            is JsonBoolean -> out.append(if (value.value) "true" else "false")
            is JsonNumber -> out.append(value.text)
            is JsonString -> writeString(value.value, out)
            is JsonArray -> {
                out.append('[')
                value.items.forEachIndexed { position, item ->
                    if (position > 0) out.append(',')
                    write(item, out)
                }
                out.append(']')
            }
            is JsonObject -> {
                out.append('{')
                var first = true
                for ((name, member) in value.members) {
                    if (!first) out.append(',')
                    first = false
                    writeString(name, out)
                    out.append(':')
                    write(member, out)
                }
                out.append('}')
            }
        }
    }

    fun writeString(value: String, out: StringBuilder) {
        out.append('"')
        for (ch in value) {
            when (ch) {
                '"' -> out.append("\\\"")
                '\\' -> out.append("\\\\")
                '\n' -> out.append("\\n")
                '\r' -> out.append("\\r")
                '\t' -> out.append("\\t")
                '\b' -> out.append("\\b")
                '\u000C' -> out.append("\\f")
                else -> if (ch < ' ') {
                    out.append("\\u00").append(HEX[(ch.code shr 4) and 0xF]).append(HEX[ch.code and 0xF])
                } else {
                    out.append(ch)
                }
            }
        }
        out.append('"')
    }
}

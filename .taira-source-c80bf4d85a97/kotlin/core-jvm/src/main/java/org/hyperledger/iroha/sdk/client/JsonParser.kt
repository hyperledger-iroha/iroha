package org.hyperledger.iroha.sdk.client

/**
 * Minimal JSON parser sufficient for the SDK polling helpers.
 *
 * Only the subset required by SDK payloads is implemented (objects, arrays, strings, booleans,
 * null, numbers).
 */
class JsonParser private constructor(private val input: String) {

    private var index = 0

    private fun parseValue(depth: Int): Any? {
        check(depth <= MAX_NESTING_DEPTH) { "JSON exceeds maximum nesting depth" }
        skipWhitespace()
        check(index < input.length) { "Unexpected end of JSON input" }
        return when (input[index]) {
            '{' -> parseObject(depth)
            '[' -> parseArray(depth)
            '"' -> parseString()
            't' -> { consumeLiteral("true"); true }
            'f' -> { consumeLiteral("false"); false }
            'n' -> { consumeLiteral("null"); null }
            else -> parseNumber()
        }
    }

    private fun parseObject(depth: Int): LinkedHashMap<String, Any?> {
        expect('{')
        skipWhitespace()
        val map = LinkedHashMap<String, Any?>()
        if (peek('}')) { index++; return map }
        while (true) {
            val key = parseString()
            check(!map.containsKey(key)) { "Duplicate JSON object key: $key" }
            skipWhitespace()
            expect(':')
            skipWhitespace()
            map[key] = parseValue(depth + 1)
            skipWhitespace()
            if (peek('}')) { index++; return map }
            expect(',')
            skipWhitespace()
        }
    }

    private fun parseArray(depth: Int): MutableList<Any?> {
        expect('[')
        skipWhitespace()
        val list = mutableListOf<Any?>()
        if (peek(']')) { index++; return list }
        while (true) {
            list.add(parseValue(depth + 1))
            skipWhitespace()
            if (peek(']')) { index++; return list }
            expect(',')
            skipWhitespace()
        }
    }

    private fun parseString(): String {
        expect('"')
        val builder = StringBuilder()
        while (index < input.length) {
            val c = input[index++]
            if (c == '"') return builder.toString()
            if (c == '\\') {
                check(index < input.length) { "Invalid escape sequence" }
                when (val esc = input[index++]) {
                    '"' -> builder.append('"')
                    '\\' -> builder.append('\\')
                    '/' -> builder.append('/')
                    'b' -> builder.append('\b')
                    'f' -> builder.append('\u000C')
                    'n' -> builder.append('\n')
                    'r' -> builder.append('\r')
                    't' -> builder.append('\t')
                    'u' -> {
                        val high = parseUnicodeEscapeUnit()
                        if (Character.isHighSurrogate(high)) {
                            check(index + 2 <= input.length && input[index] == '\\' && input[index + 1] == 'u') {
                                "Invalid unicode surrogate pair"
                            }
                            index += 2
                            val low = parseUnicodeEscapeUnit()
                            check(Character.isLowSurrogate(low)) { "Invalid unicode surrogate pair" }
                            builder.append(high).append(low)
                        } else {
                            check(!Character.isLowSurrogate(high)) { "Invalid unicode surrogate pair" }
                            builder.append(high)
                        }
                    }
                    else -> throw IllegalStateException("Unsupported escape: \\$esc")
                }
            } else {
                check(c.code >= 0x20) { "Unescaped control character in JSON string" }
                when {
                    Character.isHighSurrogate(c) -> {
                        check(index < input.length && Character.isLowSurrogate(input[index])) {
                            "Invalid unicode surrogate pair"
                        }
                        builder.append(c).append(input[index])
                        index++
                    }
                    Character.isLowSurrogate(c) ->
                        throw IllegalStateException("Invalid unicode surrogate pair")
                    else -> builder.append(c)
                }
            }
        }
        throw IllegalStateException("Unterminated string literal")
    }

    private fun parseUnicodeEscapeUnit(): Char {
        check(index + 4 <= input.length) { "Invalid unicode escape" }
        val value = input.substring(index, index + 4).toIntOrNull(16)
        check(value != null) { "Invalid unicode escape" }
        index += 4
        return value.toChar()
    }

    private fun parseNumber(): Number {
        val start = index
        if (index < input.length && input[index] == '-') index++
        check(index < input.length) { "Invalid number: expected digit" }
        var hasDigits = false
        if (index < input.length && input[index].isDigit()) {
            hasDigits = true
            if (input[index] == '0') {
                index++
                check(index >= input.length || !input[index].isDigit()) { "Invalid number: leading zero" }
            } else {
                while (index < input.length && input[index].isDigit()) index++
            }
        }
        check(hasDigits) { "Invalid number: expected digit" }
        var hasFraction = false
        if (index < input.length && input[index] == '.') {
            hasFraction = true
            index++
            check(index < input.length && input[index].isDigit()) { "Invalid number: missing digit after decimal point" }
            while (index < input.length && input[index].isDigit()) index++
        }
        var hasExponent = false
        if (index < input.length && (input[index] == 'e' || input[index] == 'E')) {
            hasExponent = true
            index++
            if (index < input.length && (input[index] == '+' || input[index] == '-')) index++
            check(index < input.length && input[index].isDigit()) { "Invalid number: missing exponent digits" }
            while (index < input.length && input[index].isDigit()) index++
        }
        val token = input.substring(start, index)
        return try {
            if (!hasFraction && !hasExponent) {
                try {
                    token.toLong()
                } catch (_: NumberFormatException) {
                    java.math.BigInteger(token)
                }
            } else {
                java.math.BigDecimal(token)
            }
        } catch (ex: NumberFormatException) {
            throw IllegalStateException("Invalid number: $token", ex)
        }
    }

    private fun consumeLiteral(literal: String) {
        check(input.regionMatches(index, literal, 0, literal.length)) { "Expected literal '$literal'" }
        index += literal.length
    }

    private fun skipWhitespace() {
        while (index < input.length && input[index] in " \t\n\r") index++
    }

    private fun expect(expected: Char) {
        check(index < input.length && input[index] == expected) { "Expected '$expected'" }
        index++
    }

    private fun peek(expected: Char): Boolean =
        index < input.length && input[index] == expected

    private fun Char.isDigit(): Boolean = this in '0'..'9'

    companion object {
        private const val MAX_NESTING_DEPTH = 128

        @JvmStatic
        fun parse(json: String): Any? {
            val parser = JsonParser(json)
            parser.skipWhitespace()
            val value = parser.parseValue(0)
            parser.skipWhitespace()
            check(parser.index == parser.input.length) { "Trailing characters after JSON payload" }
            return value
        }
    }
}

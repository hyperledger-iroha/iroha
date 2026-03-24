package org.hyperledger.iroha.sdk.client

/**
 * Minimal JSON parser sufficient for the SDK polling helpers.
 *
 * Only the subset required by SDK payloads is implemented (objects, arrays, strings, booleans,
 * null, numbers).
 */
class JsonParser private constructor(private val input: String) {

    private var index = 0

    private fun parseValue(): Any? {
        skipWhitespace()
        check(index < input.length) { "Unexpected end of JSON input" }
        return when (input[index]) {
            '{' -> parseObject()
            '[' -> parseArray()
            '"' -> parseString()
            't' -> { consumeLiteral("true"); true }
            'f' -> { consumeLiteral("false"); false }
            'n' -> { consumeLiteral("null"); null }
            else -> parseNumber()
        }
    }

    private fun parseObject(): LinkedHashMap<String, Any?> {
        expect('{')
        skipWhitespace()
        val map = LinkedHashMap<String, Any?>()
        if (peek('}')) { index++; return map }
        while (true) {
            val key = parseString()
            skipWhitespace()
            expect(':')
            skipWhitespace()
            map[key] = parseValue()
            skipWhitespace()
            if (peek('}')) { index++; return map }
            expect(',')
            skipWhitespace()
        }
    }

    private fun parseArray(): MutableList<Any?> {
        expect('[')
        skipWhitespace()
        val list = mutableListOf<Any?>()
        if (peek(']')) { index++; return list }
        while (true) {
            list.add(parseValue())
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
                        check(index + 4 <= input.length) { "Invalid unicode escape" }
                        val hex = input.substring(index, index + 4)
                        index += 4
                        builder.append(hex.toInt(16).toChar())
                    }
                    else -> throw IllegalStateException("Unsupported escape: \\$esc")
                }
            } else {
                builder.append(c)
            }
        }
        throw IllegalStateException("Unterminated string literal")
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
                token.toLong()
            } else {
                val value = token.toDouble()
                check(value.isFinite()) { "Invalid number: $token" }
                value
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
        @JvmStatic
        fun parse(json: String): Any? {
            val parser = JsonParser(json)
            parser.skipWhitespace()
            val value = parser.parseValue()
            parser.skipWhitespace()
            check(parser.index == parser.input.length) { "Trailing characters after JSON payload" }
            return value
        }
    }
}

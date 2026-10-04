package org.hyperledger.iroha.sdk.query

import java.nio.charset.StandardCharsets
import org.hyperledger.iroha.sdk.json.Json
import org.hyperledger.iroha.sdk.json.JsonBoolean
import org.hyperledger.iroha.sdk.json.JsonNull
import org.hyperledger.iroha.sdk.json.JsonNumber
import org.hyperledger.iroha.sdk.json.JsonString
import org.hyperledger.iroha.sdk.json.JsonWriter

/**
 * The human text form of filters and sort specifications, mirroring
 * `iroha_torii_shared::list_query::text` (grammar, error wording and canonical rendering).
 *
 * ```text
 * filter     := or
 * or         := and ("or" and)*
 * and        := unary ("and" unary)*
 * unary      := "not" unary | primary
 * primary    := "(" filter ")" | "exists" "(" path ")" | path predicate
 * predicate  := compare literal | ["not"] "in" list | "is" ["not"] "null"
 * compare    := "=" | "==" | "!=" | "<>" | "<" | "<=" | ">" | ">="
 * list       := "[" literal ("," literal)* [","] "]" | "(" literal ("," literal)* [","] ")"
 * literal    := string | number | "true" | "false" | "null"
 * path       := segment ("." segment)*
 * segment    := [A-Za-z_][A-Za-z0-9_]* | "`" any character except "`" "`"
 * number     := "-"? ("0" | [1-9][0-9]*) ("." [0-9]+)?
 * sort       := key ("," key)*
 * key        := ["-"] path
 * ```
 */
internal object QueryText {
    /** Maximum accepted UTF-8 length of a text filter. */
    const val FILTER_TEXT_MAX_BYTES = 32 * 1024

    /** Maximum number of keys in one sort specification. */
    const val SORT_MAX_KEYS = 8

    private const val PARSE_MAX_NESTING = 64
    private val KEYWORDS = listOf("and", "or", "not", "in", "is", "null", "true", "false", "exists")

    // ---------------------------------------------------------------- rendering

    fun renderFilter(filter: Filter): String = StringBuilder().also { write(filter, Parent.ROOT, it) }.toString()

    fun renderPath(path: FieldPath): String = StringBuilder().also { writePath(path, it) }.toString()

    private enum class Parent { ROOT, OR, AND, NOT }

    private fun write(filter: Filter, parent: Parent, out: StringBuilder) {
        when (filter) {
            is Filter.Or -> writeJunction("or", filter.operands, Parent.OR, parent != Parent.ROOT, out)
            is Filter.And ->
                writeJunction("and", filter.operands, Parent.AND, parent == Parent.AND || parent == Parent.NOT, out)
            is Filter.Not -> {
                val inner = filter.operand
                if (inner is Filter.IsNull) {
                    writePath(inner.field, out)
                    out.append(" is not null")
                } else {
                    out.append("not ")
                    write(inner, Parent.NOT, out)
                }
            }
            is Filter.Comparison -> {
                writePath(filter.field, out)
                out.append(' ').append(filter.operator.symbol).append(' ')
                writeLiteral(filter.value, out)
            }
            is Filter.Membership -> {
                writePath(filter.field, out)
                out.append(' ').append(filter.operator.symbol).append(" [")
                filter.values.forEachIndexed { index, value ->
                    if (index > 0) out.append(", ")
                    writeLiteral(value, out)
                }
                out.append(']')
            }
            is Filter.Exists -> {
                out.append("exists(")
                writePath(filter.field, out)
                out.append(')')
            }
            is Filter.IsNull -> {
                writePath(filter.field, out)
                out.append(" is null")
            }
        }
    }

    private fun writeJunction(
        keyword: String,
        operands: List<Filter>,
        me: Parent,
        parenthesize: Boolean,
        out: StringBuilder,
    ) {
        if (parenthesize) out.append('(')
        operands.forEachIndexed { index, operand ->
            if (index > 0) out.append(' ').append(keyword).append(' ')
            write(operand, me, out)
        }
        if (parenthesize) out.append(')')
    }

    /** Literals render as compact JSON; numbers with an exponent use their plain decimal spelling. */
    private fun writeLiteral(value: Json, out: StringBuilder) {
        if (value is JsonNumber && (value.text.indexOf('e') >= 0 || value.text.indexOf('E') >= 0)) {
            out.append(value.toBigDecimal().toPlainString())
        } else {
            JsonWriter.write(value, out)
        }
    }

    fun writePath(path: FieldPath, out: StringBuilder) {
        path.segments.forEachIndexed { index, segment ->
            if (index > 0) out.append('.')
            if (isBareSegment(segment, index == 0)) out.append(segment) else out.append('`').append(segment).append('`')
        }
    }

    private fun isBareSegment(segment: String, first: Boolean): Boolean {
        if (segment.isEmpty()) return false
        val head = segment[0]
        val startsWell = head in 'a'..'z' || head in 'A'..'Z' || head == '_'
        return startsWell &&
            segment.all { it in 'a'..'z' || it in 'A'..'Z' || it in '0'..'9' || it == '_' } &&
            !(first && KEYWORDS.any { it.equals(segment, ignoreCase = true) })
    }

    // ---------------------------------------------------------------- parsing

    fun parseFilter(text: String): Filter {
        if (text.toByteArray(StandardCharsets.UTF_8).size > FILTER_TEXT_MAX_BYTES) {
            throw FilterSyntaxException.at(
                text,
                utf16OffsetOfByte(text, FILTER_TEXT_MAX_BYTES),
                "filters must not exceed $FILTER_TEXT_MAX_BYTES bytes",
                "filter",
            )
        }
        if (text.isBlank()) throw FilterSyntaxException.at(text, 0, "expected a filter expression", "filter")
        val parser = Parser(text, allowMinus = false, parameter = "filter")
        val filter = parser.filter()
        parser.finish()
        try {
            filter.validate()
        } catch (error: InvalidFilterException) {
            throw FilterSyntaxException.structure(error.message ?: "invalid filter", "filter")
        }
        return filter
    }

    fun parseSort(text: String): List<SortKey> {
        if (text.isBlank()) throw FilterSyntaxException.at(text, 0, "expected at least one sort key", "sort")
        val parser = Parser(text, allowMinus = true, parameter = "sort")
        val keys = ArrayList<SortKey>()
        while (true) {
            val token = parser.peek()
            val descending = if (token.kind == Kind.MINUS) {
                parser.advance()
                true
            } else {
                false
            }
            val key = parser.path()
            if (keys.any { it.field == key }) {
                throw parser.errorAt(token, "sort key `$key` appears more than once")
            }
            keys.add(SortKey.unchecked(key, descending))
            if (keys.size > SORT_MAX_KEYS) {
                throw parser.errorAt(token, "sort specifications accept at most $SORT_MAX_KEYS keys")
            }
            val next = parser.advance()
            when {
                next.kind == Kind.END -> return keys
                next.kind == Kind.COMMA -> Unit
                next.kind == Kind.WORD &&
                    (next.text.equals("asc", ignoreCase = true) || next.text.equals("desc", ignoreCase = true)) ->
                    throw parser.errorAt(next, "write `field` for ascending and `-field` for descending order")
                else -> throw parser.errorAt(next, "expected `,` between sort keys, found ${next.describe()}")
            }
        }
    }

    private fun utf16OffsetOfByte(text: String, byteLimit: Int): Int {
        var bytes = 0
        var index = 0
        while (index < text.length) {
            val codePoint = text.codePointAt(index)
            val width = when {
                codePoint < 0x80 -> 1
                codePoint < 0x800 -> 2
                codePoint < 0x10000 -> 3
                else -> 4
            }
            if (bytes + width > byteLimit) return index
            bytes += width
            index += Character.charCount(codePoint)
        }
        return text.length
    }

    private enum class Kind {
        WORD, QUOTED, STRING, NUMBER, COMPARE, MINUS, LPAREN, RPAREN, LBRACKET, RBRACKET, COMMA, DOT, END
    }

    private class Token(
        val kind: Kind,
        val start: Int,
        val text: String = "",
        val compare: ComparisonOperator? = null,
    ) {
        fun describe(): String = when (kind) {
            Kind.WORD, Kind.QUOTED -> "`$text`"
            Kind.STRING -> "a string literal"
            Kind.NUMBER -> "the number `$text`"
            Kind.COMPARE -> "a comparison operator"
            Kind.MINUS -> "`-`"
            Kind.LPAREN -> "`(`"
            Kind.RPAREN -> "`)`"
            Kind.LBRACKET -> "`[`"
            Kind.RBRACKET -> "`]`"
            Kind.COMMA -> "`,`"
            Kind.DOT -> "`.`"
            Kind.END -> "the end of the input"
        }

        fun keyword(): String? =
            if (kind == Kind.WORD) KEYWORDS.firstOrNull { it.equals(text, ignoreCase = true) } else null

        fun isKeyword(keyword: String): Boolean = kind == Kind.WORD && text.equals(keyword, ignoreCase = true)
    }

    private class Lexer(private val input: String, private val allowMinus: Boolean, private val parameter: String) {
        private var position = 0

        fun tokens(): List<Token> {
            val out = ArrayList<Token>()
            while (true) {
                val token = next()
                out.add(token)
                if (token.kind == Kind.END) return out
            }
        }

        private fun error(offset: Int, reason: String) = FilterSyntaxException.at(input, offset, reason, parameter)

        private fun peekChar(ahead: Int): Char? = input.getOrNull(position + ahead)

        private fun isDigit(ch: Char?) = ch != null && ch in '0'..'9'

        private fun isAlpha(ch: Char?) = ch != null && (ch in 'a'..'z' || ch in 'A'..'Z')

        private fun isAlnum(ch: Char?) = isAlpha(ch) || isDigit(ch)

        private fun single(kind: Kind, start: Int): Token {
            position += 1
            return Token(kind, start)
        }

        private fun next(): Token {
            while (position < input.length && input[position].let { it == ' ' || it == '\t' || it == '\r' || it == '\n' }) {
                position++
            }
            val start = position
            val ch = peekChar(0) ?: return Token(Kind.END, start)
            return when (ch) {
                '(' -> single(Kind.LPAREN, start)
                ')' -> single(Kind.RPAREN, start)
                '[' -> single(Kind.LBRACKET, start)
                ']' -> single(Kind.RBRACKET, start)
                ',' -> single(Kind.COMMA, start)
                '.' -> {
                    if (isDigit(peekChar(1))) throw error(start, "decimal literals need a leading digit, e.g. `0.5`")
                    single(Kind.DOT, start)
                }
                '=' -> {
                    position += if (peekChar(1) == '=') 2 else 1
                    Token(Kind.COMPARE, start, compare = ComparisonOperator.EQ)
                }
                '!' -> {
                    if (peekChar(1) != '=') throw error(start, "use the keyword `not` instead of `!`")
                    position += 2
                    Token(Kind.COMPARE, start, compare = ComparisonOperator.NE)
                }
                '<' -> {
                    val (operator, width) = when (peekChar(1)) {
                        '=' -> ComparisonOperator.LTE to 2
                        '>' -> ComparisonOperator.NE to 2
                        else -> ComparisonOperator.LT to 1
                    }
                    position += width
                    Token(Kind.COMPARE, start, compare = operator)
                }
                '>' -> {
                    val (operator, width) =
                        if (peekChar(1) == '=') ComparisonOperator.GTE to 2 else ComparisonOperator.GT to 1
                    position += width
                    Token(Kind.COMPARE, start, compare = operator)
                }
                '&' -> throw error(start, "use the keyword `and` instead of `&` or `&&`")
                '|' -> throw error(start, "use the keyword `or` instead of `|` or `||`")
                '"', '\'' -> string(ch)
                '`' -> quotedSegment()
                '-' -> when {
                    isDigit(peekChar(1)) -> number()
                    allowMinus -> single(Kind.MINUS, start)
                    else -> throw error(
                        start,
                        "unexpected `-`; quote field names that contain `-` with backticks, e.g. `display-name`",
                    )
                }
                in '0'..'9' -> number()
                ':' -> throw error(
                    start,
                    if (allowMinus) {
                        "unexpected `:`; write `field` for ascending and `-field` for descending order"
                    } else {
                        "unexpected `:`; compare values with `=`, e.g. `status = \"active\"`"
                    },
                )
                else -> if (isAlpha(ch) || ch == '_') word(start) else throw error(
                    start,
                    "unexpected character `${String(Character.toChars(input.codePointAt(start)))}`",
                )
            }
        }

        private fun word(start: Int): Token {
            var end = start + 1
            while (end < input.length && (isAlnum(input[end]) || input[end] == '_')) end++
            position = end
            if (peekChar(0) == '-' && isAlpha(peekChar(1))) {
                var wordEnd = end
                while (wordEnd < input.length && (isAlnum(input[wordEnd]) || input[wordEnd] == '_' || input[wordEnd] == '-')) {
                    wordEnd++
                }
                throw error(start, "wrap field names containing `-` in backticks, e.g. `${input.substring(start, wordEnd)}`")
            }
            return Token(Kind.WORD, start, input.substring(start, end))
        }

        private fun number(): Token {
            val start = position
            var end = start
            if (input.getOrNull(end) == '-') end++
            val integerStart = end
            while (isDigit(input.getOrNull(end))) end++
            if (end - integerStart > 1 && input[integerStart] == '0') {
                throw error(start, "numbers must not have leading zeros")
            }
            if (input.getOrNull(end) == '.') {
                end++
                val fractionStart = end
                while (isDigit(input.getOrNull(end))) end++
                if (end == fractionStart) throw error(start, "decimal literals need digits after `.`")
            }
            val following = input.getOrNull(end)
            if (following == 'e' || following == 'E') {
                throw error(start, "exponent notation is not supported; write the full decimal value")
            }
            if (isAlpha(following) || following == '_') {
                throw error(start, "a number cannot be followed directly by letters; quote text values")
            }
            position = end
            return Token(Kind.NUMBER, start, input.substring(start, end))
        }

        private fun string(quote: Char): Token {
            val start = position
            val out = StringBuilder()
            var index = start + 1
            while (true) {
                if (index >= input.length) throw error(start, "unterminated string literal")
                val ch = input[index]
                when {
                    ch == quote -> {
                        position = index + 1
                        return Token(Kind.STRING, start, out.toString())
                    }
                    ch == '\\' -> {
                        val escapeAt = index
                        if (index + 1 >= input.length) throw error(escapeAt, "unterminated escape sequence")
                        val escaped = input[index + 1]
                        index += 2
                        when (escaped) {
                            '"' -> out.append('"')
                            '\'' -> out.append('\'')
                            '\\' -> out.append('\\')
                            '/' -> out.append('/')
                            'b' -> out.append('\b')
                            'f' -> out.append('\u000C')
                            'n' -> out.append('\n')
                            'r' -> out.append('\r')
                            't' -> out.append('\t')
                            'u' -> index = unicodeEscape(index, escapeAt, out)
                            else -> {
                                val shown = String(Character.toChars(input.codePointAt(index - 1)))
                                throw error(escapeAt, "unknown escape sequence `\\$shown`")
                            }
                        }
                        continue
                    }
                    // As in JSON, only U+0000..U+001F must be escaped; DEL and C1 characters stay
                    // literal, so every rendered literal parses.
                    ch < ' ' ->
                        throw error(index, "control characters must be escaped inside string literals")
                    else -> {
                        out.append(ch)
                        index++
                    }
                }
            }
        }

        /** Decode the four hex digits after `\u` at [index]; returns the index after the escape. */
        private fun unicodeEscape(index: Int, at: Int, out: StringBuilder): Int {
            fun unit(from: Int): Int? {
                if (from + 4 > input.length) return null
                var value = 0
                for (offset in 0 until 4) {
                    val digit = when (val ch = input[from + offset]) {
                        in '0'..'9' -> ch - '0'
                        in 'a'..'f' -> ch - 'a' + 10
                        in 'A'..'F' -> ch - 'A' + 10
                        else -> return null
                    }
                    value = value * 16 + digit
                }
                return value
            }
            val invalid = { error(at, "invalid `\\u` escape; expected four hexadecimal digits") }
            val unpaired = { error(at, "unpaired UTF-16 surrogate in `\\u` escape") }
            val first = unit(index) ?: throw invalid()
            var next = index + 4
            if (first in 0xD800 until 0xDC00) {
                if (input.getOrNull(next) != '\\' || input.getOrNull(next + 1) != 'u') throw unpaired()
                val second = unit(next + 2) ?: throw invalid()
                if (second !in 0xDC00 until 0xE000) throw unpaired()
                out.append(first.toChar()).append(second.toChar())
                next += 6
                return next
            }
            if (first in 0xDC00 until 0xE000) throw unpaired()
            out.append(first.toChar())
            return next
        }

        private fun quotedSegment(): Token {
            val start = position
            val close = input.indexOf('`', start + 1)
            if (close < 0) throw error(start, "unterminated backtick-quoted field name")
            val segment = input.substring(start + 1, close)
            if (segment.isEmpty()) throw error(start, "backtick-quoted field names must not be empty")
            if (segment.indexOf('.') >= 0) {
                throw error(start, "a backtick-quoted segment must not contain `.`; quote each segment separately")
            }
            position = close + 1
            return Token(Kind.QUOTED, start, segment)
        }
    }

    private class Parser(private val input: String, allowMinus: Boolean, private val parameter: String) {
        private val tokens = Lexer(input, allowMinus, parameter).tokens()
        private var position = 0
        private var nesting = 0

        fun peek(): Token = tokens[minOf(position, tokens.size - 1)]

        private fun peekAt(ahead: Int): Token = tokens[minOf(position + ahead, tokens.size - 1)]

        fun advance(): Token {
            val token = peek()
            if (token.kind != Kind.END) position++
            return token
        }

        fun errorAt(token: Token, reason: String) = FilterSyntaxException.at(input, token.start, reason, parameter)

        private fun enter(token: Token) {
            nesting++
            if (nesting > PARSE_MAX_NESTING) throw errorAt(token, "filter nests too deeply")
        }

        fun filter(): Filter {
            val first = and()
            if (!peek().isKeyword("or")) return first
            val operands = arrayListOf(first)
            while (peek().isKeyword("or")) {
                advance()
                operands.add(and())
            }
            return Filter.Or(operands)
        }

        private fun and(): Filter {
            val first = unary()
            if (!peek().isKeyword("and")) return first
            val operands = arrayListOf(first)
            while (peek().isKeyword("and")) {
                advance()
                operands.add(unary())
            }
            return Filter.And(operands)
        }

        private fun unary(): Filter {
            if (peek().isKeyword("not")) {
                val token = advance()
                enter(token)
                val inner = unary()
                nesting--
                return Filter.Not(inner)
            }
            return primary()
        }

        private fun primary(): Filter {
            val token = peek()
            return when (token.kind) {
                Kind.LPAREN -> {
                    advance()
                    enter(token)
                    val inner = filter()
                    nesting--
                    expectClose(Kind.RPAREN, token)
                    inner
                }
                Kind.WORD, Kind.QUOTED -> {
                    if (token.isKeyword("exists") && peekAt(1).kind == Kind.LPAREN) {
                        advance()
                        val open = advance()
                        val field = path()
                        expectClose(Kind.RPAREN, open)
                        Filter.Exists(field)
                    } else {
                        predicate(path())
                    }
                }
                Kind.STRING, Kind.NUMBER ->
                    throw errorAt(token, "expected a field name on the left-hand side, e.g. `quantity > 5`")
                Kind.END -> throw errorAt(token, "expected a filter expression")
                else -> throw errorAt(token, "expected a field name, found ${token.describe()}")
            }
        }

        private fun expectClose(close: Kind, open: Token) {
            val token = advance()
            if (token.kind == close) return
            val (symbol, opened) = if (close == Kind.RPAREN) "`)`" to "`(`" else "`]`" to "`[`"
            val column = FilterSyntaxException.at(input, open.start, "", parameter).column
            throw errorAt(token, "expected $symbol to close the $opened at column $column, found ${token.describe()}")
        }

        fun path(): FieldPath {
            val first = advance()
            val path = StringBuilder()
            when (first.kind) {
                Kind.WORD -> {
                    val keyword = first.keyword()
                    if (keyword != null) {
                        throw errorAt(
                            first,
                            "expected a field name, found the keyword `$keyword`; quote a field with this name as `${first.text}` in backticks",
                        )
                    }
                    path.append(first.text)
                }
                Kind.QUOTED -> path.append(first.text)
                else -> throw errorAt(first, "expected a field name, found ${first.describe()}")
            }
            while (peek().kind == Kind.DOT) {
                advance()
                val segment = advance()
                if (segment.kind == Kind.WORD || segment.kind == Kind.QUOTED) {
                    path.append('.').append(segment.text)
                } else {
                    throw errorAt(segment, "expected a field name after `.`, found ${segment.describe()}")
                }
            }
            val text = path.toString()
            FieldPath.validationError(text)?.let { throw errorAt(first, it) }
            return FieldPath.unchecked(text)
        }

        private fun predicate(field: FieldPath): Filter {
            val token = advance()
            return when {
                token.kind == Kind.COMPARE -> Filter.Comparison(field, token.compare!!, literal())
                token.isKeyword("in") -> Filter.Membership(field, MembershipOperator.IN, list())
                token.isKeyword("not") -> {
                    val next = advance()
                    if (!next.isKeyword("in")) {
                        throw errorAt(next, "expected `in` after `not` (as in `field not in [...]`)")
                    }
                    Filter.Membership(field, MembershipOperator.NOT_IN, list())
                }
                token.isKeyword("is") -> {
                    val next = advance()
                    when {
                        next.isKeyword("null") -> Filter.IsNull(field)
                        next.isKeyword("not") -> {
                            val nullToken = advance()
                            if (!nullToken.isKeyword("null")) throw errorAt(nullToken, "expected `null` after `is not`")
                            Filter.Not(Filter.IsNull(field))
                        }
                        else -> throw errorAt(next, "expected `null` or `not null` after `is`; compare values with `=`")
                    }
                }
                token.kind == Kind.END -> throw errorAt(
                    token,
                    "expected an operator after `$field` (=, !=, <, <=, >, >=, in, not in, is null)",
                )
                else -> throw errorAt(
                    token,
                    "expected an operator after `$field` (=, !=, <, <=, >, >=, in, not in, is null), found ${token.describe()}",
                )
            }
        }

        private fun list(): List<Json> {
            val open = advance()
            val close = when (open.kind) {
                Kind.LBRACKET -> Kind.RBRACKET
                Kind.LPAREN -> Kind.RPAREN
                else -> throw errorAt(open, "expected `[` to start a value list, found ${open.describe()}")
            }
            val values = ArrayList<Json>()
            while (true) {
                if (peek().kind == close) {
                    if (values.isEmpty()) throw errorAt(peek(), "value lists must not be empty")
                    advance()
                    return values
                }
                values.add(literal())
                val separator = peek()
                when (separator.kind) {
                    Kind.COMMA -> advance()
                    close -> Unit
                    else -> {
                        expectClose(close, open)
                        return values
                    }
                }
            }
        }

        private fun literal(): Json {
            val token = advance()
            return when {
                token.kind == Kind.STRING -> JsonString(token.text)
                token.kind == Kind.NUMBER -> Literals.numberValue(token.text)
                token.isKeyword("true") -> JsonBoolean.TRUE
                token.isKeyword("false") -> JsonBoolean.FALSE
                token.isKeyword("null") -> JsonNull
                token.kind == Kind.WORD -> throw errorAt(
                    token,
                    "expected a literal value, found `${token.text}`; quote text values, e.g. \"${token.text}\"",
                )
                else -> throw errorAt(token, "expected a literal value, found ${token.describe()}")
            }
        }

        fun finish() {
            val token = peek()
            if (token.kind == Kind.END) return
            val hint = if (token.kind == Kind.WORD || token.kind == Kind.QUOTED) {
                "; combine conditions with `and` or `or`"
            } else {
                ""
            }
            throw errorAt(token, "unexpected ${token.describe()} after a complete filter$hint")
        }
    }
}

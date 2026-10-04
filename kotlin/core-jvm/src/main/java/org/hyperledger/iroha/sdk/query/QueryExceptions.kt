package org.hyperledger.iroha.sdk.query

/**
 * A list-query control rejected before the request was sent.
 *
 * [parameter] names the control (`filter`, `sort`, `select`, `aggregate`, `limit`, `cursor`,
 * `include_total`, or `query` for the request as a whole) and [code] is the error code Torii
 * would return for the same problem (`invalid_filter`, `invalid_sort`, ...).
 */
open class ListQueryException(
    /** The control at fault. */
    @JvmField val parameter: String,
    message: String,
    cause: Throwable? = null,
) : IllegalArgumentException(message, cause) {
    /** Stable error code matching Torii's error envelope. */
    val code: String get() = codeFor(parameter)

    companion object {
        /** Torii error code for a rejected [parameter]. */
        @JvmStatic
        fun codeFor(parameter: String): String = when (parameter) {
            "filter" -> "invalid_filter"
            "sort" -> "invalid_sort"
            "select" -> "invalid_select"
            "aggregate" -> "invalid_aggregate"
            "limit" -> "invalid_limit"
            "cursor" -> "invalid_cursor"
            "include_total" -> "invalid_include_total"
            else -> "invalid_query"
        }
    }
}

/**
 * A structurally invalid filter: a malformed JSON node, an invalid field path, an operand that does
 * not fit its operator, or an exceeded size limit.
 */
class InvalidFilterException @JvmOverloads constructor(
    message: String,
    /** The field the problem belongs to, when there is one. */
    @JvmField val field: String? = null,
    cause: Throwable? = null,
) : ListQueryException("filter", message, cause)

/**
 * A text filter or sort specification that does not parse.
 *
 * [reason] is the bare description (it matches the golden vectors); [message] appends the position
 * the way Torii does: `… (column 17)` or `… (line 2, column 7)` for multi-line input.
 */
class FilterSyntaxException internal constructor(
    /** Description of the problem, including a fix when one is obvious. */
    @JvmField val reason: String,
    /** UTF-16 offset of the offending token in the input. */
    @JvmField val offset: Int,
    /** 1-based line of the offending token. */
    @JvmField val line: Int,
    /** 1-based column (in Unicode scalar values) of the offending token. */
    @JvmField val column: Int,
    multiline: Boolean,
    parameter: String,
) : ListQueryException(
    parameter,
    if (multiline) "$reason (line $line, column $column)" else "$reason (column $column)",
) {
    internal companion object {
        /** Error at UTF-16 [offset] of [input], counting lines and Unicode scalar columns. */
        fun at(input: String, offset: Int, reason: String, parameter: String): FilterSyntaxException {
            val bounded = offset.coerceIn(0, input.length)
            val before = input.substring(0, bounded)
            val line = before.count { it == '\n' } + 1
            val lineStart = before.lastIndexOf('\n') + 1
            val column = before.codePointCount(lineStart, before.length) + 1
            return FilterSyntaxException(reason, bounded, line, column, input.indexOf('\n') >= 0, parameter)
        }

        /** A whole-tree (structural) error reported at the first column, as Torii does. */
        fun structure(reason: String, parameter: String): FilterSyntaxException =
            FilterSyntaxException(reason, 0, 1, 1, false, parameter)
    }
}

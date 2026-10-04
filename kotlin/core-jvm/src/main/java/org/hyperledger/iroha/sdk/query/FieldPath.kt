package org.hyperledger.iroha.sdk.query

import java.nio.charset.StandardCharsets

/**
 * A dotted field path such as `owned_by`, `quantity` or `metadata.tier`.
 *
 * [path] is the raw dotted spelling used by the JSON form; segments are never quoted there.
 * [toString] renders the canonical text spelling, which wraps a segment in backticks when it is
 * not an identifier or when the first segment is a keyword (``metadata.`display-name` ``).
 */
class FieldPath private constructor(
    /** Dotted spelling, for example `metadata.display-name`. */
    @JvmField val path: String,
) {
    /** The dot-separated segments. */
    val segments: List<String> get() = path.split('.')

    /** Canonical text spelling. */
    override fun toString(): String = QueryText.renderPath(this)

    override fun equals(other: Any?): Boolean = other is FieldPath && other.path == path

    override fun hashCode(): Int = path.hashCode()

    companion object {
        /** Maximum UTF-8 length of one field path. */
        const val MAX_BYTES: Int = 256

        /**
         * Validate a dotted path: non-empty segments, at most [MAX_BYTES] UTF-8 bytes and no
         * whitespace or control characters. Whether a collection exposes the field is decided by
         * Torii.
         */
        @JvmStatic
        fun of(path: String): FieldPath {
            validationError(path)?.let { throw IllegalArgumentException(it) }
            return FieldPath(path)
        }

        /** [of], reporting a bad path as a [ListQueryException] for [parameter]. */
        internal fun forParameter(path: String, parameter: String): FieldPath {
            validationError(path)?.let { reason ->
                throw if (parameter == "filter") InvalidFilterException(reason, path) else ListQueryException(parameter, reason)
            }
            return FieldPath(path)
        }

        /** A path from individual segments, none of which may contain `.`. */
        @JvmStatic
        fun ofSegments(vararg segments: String): FieldPath {
            require(segments.isNotEmpty()) { "a field path needs at least one segment" }
            segments.forEach { segment ->
                require(segment.indexOf('.') < 0) { "invalid field `$segment`: a segment must not contain `.`" }
            }
            return of(segments.joinToString("."))
        }

        /** Rust `FieldPath::validate` wording, or `null` when [path] is valid. */
        internal fun validationError(path: String): String? {
            val reason = when {
                path.isEmpty() -> "field paths must not be empty"
                path.toByteArray(StandardCharsets.UTF_8).size > MAX_BYTES ->
                    "field paths must not exceed 256 bytes"
                path.any { Character.isWhitespace(it) || Character.isISOControl(it) || isUnicodeSpace(it) } ->
                    "field paths must not contain whitespace or control characters"
                path.split('.').any { it.isEmpty() } -> "field path segments must not be empty"
                else -> return null
            }
            return "invalid field `$path`: $reason"
        }

        internal fun unchecked(path: String): FieldPath = FieldPath(path)

        private fun isUnicodeSpace(ch: Char): Boolean =
            ch == ' ' || ch == ' ' || ch == ' ' || Character.isSpaceChar(ch)
    }
}

package org.hyperledger.iroha.sdk.query

/**
 * One sort key: `field` sorts ascending and `-field` descending.
 *
 * [toString] is the canonical spelling used in both `sort=-quantity,id` and the JSON
 * `"sort": ["-quantity", "id"]` array (non-identifier segments are backtick-quoted).
 */
class SortKey private constructor(
    /** The field to sort by. */
    @JvmField val field: FieldPath,
    /** Whether larger values come first. */
    @JvmField val descending: Boolean,
) {
    /** Canonical spelling such as `-quantity` or ``metadata.`ui-order` ``. */
    override fun toString(): String = (if (descending) "-" else "") + field.toString()

    override fun equals(other: Any?): Boolean =
        other is SortKey && other.field == field && other.descending == descending

    override fun hashCode(): Int = field.hashCode() * 31 + descending.hashCode()

    companion object {
        /** Ascending key on [field]. */
        @JvmStatic
        fun asc(field: FieldPath): SortKey = SortKey(field, false)

        /** Ascending key on the dotted path [field]. */
        @JvmStatic
        fun asc(field: String): SortKey = SortKey(FieldPath.forParameter(field, "sort"), false)

        /** Descending key on [field]. */
        @JvmStatic
        fun desc(field: FieldPath): SortKey = SortKey(field, true)

        /** Descending key on the dotted path [field]. */
        @JvmStatic
        fun desc(field: String): SortKey = SortKey(FieldPath.forParameter(field, "sort"), true)

        /**
         * Parse one key such as `-quantity` or ``metadata.`ui-order` ``.
         *
         * @throws FilterSyntaxException with [ListQueryException.code] `invalid_sort`
         */
        @JvmStatic
        fun parse(text: String): SortKey {
            val keys = QueryText.parseSort(text)
            if (keys.size != 1) {
                throw FilterSyntaxException.at(
                    text,
                    0,
                    "expected exactly one sort key; pass each key as its own array element",
                    "sort",
                )
            }
            return keys[0]
        }

        /**
         * Parse a comma-separated specification such as `-quantity,id` (at most 8 unique keys).
         *
         * @throws FilterSyntaxException with [ListQueryException.code] `invalid_sort`
         */
        @JvmStatic
        fun parseList(text: String): List<SortKey> = QueryText.parseSort(text)

        /** Render keys as the canonical `GET` spelling, e.g. `-quantity,id`. */
        @JvmStatic
        fun render(keys: List<SortKey>): String = keys.joinToString(",")

        internal fun unchecked(field: FieldPath, descending: Boolean): SortKey = SortKey(field, descending)
    }
}

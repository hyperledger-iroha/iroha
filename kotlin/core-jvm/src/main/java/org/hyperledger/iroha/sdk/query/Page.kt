package org.hyperledger.iroha.sdk.query

import java.util.Collections
import org.hyperledger.iroha.sdk.json.Json
import org.hyperledger.iroha.sdk.json.JsonArray
import org.hyperledger.iroha.sdk.json.JsonNull
import org.hyperledger.iroha.sdk.json.JsonNumber
import org.hyperledger.iroha.sdk.json.JsonObject
import org.hyperledger.iroha.sdk.json.JsonString

/**
 * One page of a collection read: `{"items": [...], "next_cursor": "..." | null, "total": N}`.
 *
 * Pass [nextCursor] unchanged as the next request's cursor (see [ListQuery.after]); it is `null`
 * on the last page. [total] is present only when the query asked for it.
 */
class Page<T>(
    items: List<T>,
    /** Opaque continuation token, or `null` on the last page. */
    @JvmField val nextCursor: String?,
    /** Exact number of matching rows when `include_total` was requested. */
    @JvmField val total: Long?,
) {
    /** Items on this page in the requested order. */
    @JvmField
    val items: List<T> = Collections.unmodifiableList(ArrayList(items))

    /** Whether another page follows. */
    fun hasMore(): Boolean = nextCursor != null

    /** The same page with every item converted by [transform]. */
    fun <U> map(transform: (T) -> U): Page<U> = Page(items.map(transform), nextCursor, total)

    override fun equals(other: Any?): Boolean =
        other is Page<*> && other.items == items && other.nextCursor == nextCursor && other.total == total

    override fun hashCode(): Int = (items.hashCode() * 31 + (nextCursor?.hashCode() ?: 0)) * 31 + (total?.hashCode() ?: 0)

    override fun toString(): String = "Page(items=${items.size}, nextCursor=$nextCursor, total=$total)"

    companion object {
        /**
         * Decode the page envelope; unknown members are ignored as the contract requires.
         *
         * @throws IllegalArgumentException when `items`, `next_cursor` or `total` is malformed
         */
        @JvmStatic
        fun fromJson(json: Json): Page<Json> {
            require(json is JsonObject) { "a page must be a JSON object" }
            val items = json["items"] as? JsonArray
                ?: throw IllegalArgumentException("a page must contain an `items` array")
            val nextCursor = when (val cursor = json["next_cursor"]) {
                null, JsonNull -> null
                is JsonString -> cursor.value
                else -> throw IllegalArgumentException("`next_cursor` must be a string or null")
            }
            val total = when (val count = json["total"]) {
                null, JsonNull -> null
                is JsonNumber -> {
                    val exact = if (count.isInteger) count.toBigInteger() else null
                    require(exact != null && exact.signum() >= 0 && exact.bitLength() < 64) {
                        "`total` must be a non-negative integer"
                    }
                    exact.toLong()
                }
                else -> throw IllegalArgumentException("`total` must be a non-negative integer")
            }
            return Page(items.items, nextCursor, total)
        }
    }
}

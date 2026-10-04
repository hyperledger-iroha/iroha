package org.hyperledger.iroha.sdk.query

import java.math.BigInteger
import java.net.URLEncoder
import java.nio.charset.StandardCharsets
import java.util.AbstractMap
import java.util.Collections
import org.hyperledger.iroha.sdk.json.Json
import org.hyperledger.iroha.sdk.json.JsonArray
import org.hyperledger.iroha.sdk.json.JsonBoolean
import org.hyperledger.iroha.sdk.json.JsonNull
import org.hyperledger.iroha.sdk.json.JsonNumber
import org.hyperledger.iroha.sdk.json.JsonObject
import org.hyperledger.iroha.sdk.json.JsonString

/**
 * Filter, ordering, projection and page controls for one Torii collection read
 * (`specs/torii/collection_queries.md`).
 *
 * ```kotlin
 * val query = listQuery {
 *     filter((field("owned_by") eq alice) and (field("quantity") gt 1))
 *     sort(field("quantity").desc(), field("id").asc())
 *     limit(50)
 * }
 * ```
 * Java: `ListQuery.builder().filter(...).sort("-quantity,id").limit(50).build()`.
 *
 * Instances are immutable and validated: [Builder.build] rejects exactly what Torii would reject
 * for the same controls ([ListQueryException.code] carries the matching `invalid_*` code). A text
 * filter given through [Builder.filter] (String) is passed to Torii unchanged.
 */
class ListQuery private constructor(
    /** Rows to keep, when given as a tree. */
    @JvmField val filter: Filter?,
    /** Rows to keep, when given as text; passed to Torii unchanged. */
    @JvmField val filterText: String?,
    sort: List<SortKey>,
    select: List<FieldPath>?,
    /** Grouped metrics instead of items (`POST` only). */
    @JvmField val aggregate: AggregateSpec?,
    /** Rows per page; Torii's default page size applies when `null`. */
    @JvmField val limit: Int?,
    /** Continuation token from a previous page's `next_cursor`. */
    @JvmField val cursor: String?,
    /** Whether to compute the exact number of matching rows (costs a full scan). */
    @JvmField val includeTotal: Boolean,
) {
    /** Ordering; the collection's default order applies when empty. */
    @JvmField
    val sort: List<SortKey> = Collections.unmodifiableList(ArrayList(sort))

    /** Fields returned per item; all fields when `null`. */
    @JvmField
    val select: List<FieldPath>? = select?.let { Collections.unmodifiableList(ArrayList(it)) }

    /** Canonical `POST /v1/<collection>/query` body; absent controls are omitted. */
    fun toJson(): JsonObject {
        val members = LinkedHashMap<String, Json>()
        filter?.let { members["filter"] = it.toJson() }
        filterText?.let { members["filter"] = JsonString(it) }
        if (sort.isNotEmpty()) members["sort"] = JsonArray(sort.map { JsonString(it.toString()) })
        select?.let { fields -> members["select"] = JsonArray(fields.map { JsonString(it.path) }) }
        aggregate?.let { members["aggregate"] = it.toJson() }
        limit?.let { members["limit"] = JsonNumber.of(it.toLong()) }
        cursor?.let { members["cursor"] = JsonString(it) }
        if (includeTotal) members["include_total"] = JsonBoolean.TRUE
        return JsonObject(members)
    }

    /**
     * `GET` parameters in canonical order, not yet percent-encoded.
     *
     * @throws ListQueryException for aggregates, which have no URL form, and for filters with
     *   object or array literals, which exist only in the JSON form
     */
    fun toQueryPairs(): List<Map.Entry<String, String>> {
        if (aggregate != null) {
            throw ListQueryException("aggregate", "aggregates are only available through POST /query")
        }
        filter?.let { FilterValidation.requireTextRepresentable(it, "GET query parameters") }
        val pairs = ArrayList<Map.Entry<String, String>>()
        fun add(name: String, value: String) {
            pairs.add(AbstractMap.SimpleImmutableEntry(name, value))
        }
        filter?.let { add("filter", it.toString()) }
        filterText?.let { add("filter", it) }
        if (sort.isNotEmpty()) add("sort", SortKey.render(sort))
        select?.let { fields -> add("select", fields.joinToString(",") { it.path }) }
        limit?.let { add("limit", it.toString()) }
        cursor?.let { add("cursor", it) }
        if (includeTotal) add("include_total", "true")
        return Collections.unmodifiableList(pairs)
    }

    /** The percent-encoded `GET` query string (without `?`), e.g. `filter=a%20%3D%201&limit=10`. */
    fun toQueryString(): String = toQueryPairs().joinToString("&") { (name, value) ->
        "${percentEncode(name)}=${percentEncode(value)}"
    }

    /** The same query positioned at [cursor] (`null` restarts from the first page). */
    fun withCursor(cursor: String?): ListQuery = toBuilder().cursor(cursor).build()

    /** The same query positioned after [page], or `null` when [page] was the last one. */
    fun after(page: Page<*>): ListQuery? = page.nextCursor?.let(::withCursor)

    /** A builder pre-populated with every control of this query. */
    fun toBuilder(): Builder = Builder().also { builder ->
        builder.filter = filter
        builder.filterText = filterText
        builder.sort.addAll(sort)
        builder.select = select?.let { ArrayList(it) }
        builder.aggregate = aggregate
        builder.limit = limit
        builder.cursor = cursor
        builder.includeTotal = includeTotal
    }

    /** The canonical body JSON. */
    override fun toString(): String = toJson().toJsonString()

    override fun equals(other: Any?): Boolean =
        other is ListQuery && other.filter == filter && other.filterText == filterText && other.sort == sort &&
            other.select == select && other.aggregate == aggregate && other.limit == limit &&
            other.cursor == cursor && other.includeTotal == includeTotal

    override fun hashCode(): Int = toJson().hashCode()

    companion object {
        /** Maximum number of fields in one projection. */
        const val SELECT_MAX_FIELDS: Int = 64

        /** Maximum length of a pagination cursor. */
        const val CURSOR_MAX_BYTES: Int = 4096

        /** Default order, all fields, Torii's default page size. */
        @JvmField
        val EMPTY: ListQuery = Builder().build()

        @JvmStatic
        fun builder(): Builder = Builder()

        /** Members accepted in a `POST /query` body, in canonical order. */
        @JvmField
        val MEMBERS: List<String> =
            Collections.unmodifiableList(listOf("filter", "sort", "select", "aggregate", "limit", "cursor", "include_total"))

        /** Parameters accepted by `GET` collection endpoints. */
        @JvmField
        val PARAMETERS: List<String> =
            Collections.unmodifiableList(listOf("filter", "sort", "select", "limit", "cursor", "include_total"))

        /**
         * Decode a `POST /query` body with Torii's rules (unknown members rejected).
         *
         * @throws ListQueryException naming the offending member
         */
        @JvmStatic
        fun fromJson(body: Json): ListQuery {
            if (body !is JsonObject) {
                throw ListQueryException(
                    "query",
                    "the request body must be a JSON object such as {\"filter\": \"...\", \"limit\": 50}",
                )
            }
            val builder = Builder()
            // Torii decodes members in sorted order, which fixes which error wins.
            for (name in body.keys.sorted()) {
                val value = body.members.getValue(name)
                when (name) {
                    "filter" -> if (value !== JsonNull) {
                        builder.filter = wrap("filter") { Filter.fromJson(value) }
                    }
                    "sort" -> when (value) {
                        JsonNull -> Unit
                        is JsonArray -> value.items.forEach { key ->
                            val text = (key as? JsonString)?.value
                                ?: throw ListQueryException("sort", "sort keys are strings such as \"-quantity\" or \"id\"")
                            builder.sort.add(wrap("sort") { SortKey.parse(text) })
                        }
                        else -> throw ListQueryException(
                            "sort",
                            "`sort` must be an array of keys such as [\"-quantity\", \"id\"]",
                        )
                    }
                    "select" -> when (value) {
                        JsonNull -> Unit
                        is JsonArray -> builder.select = value.items.mapTo(ArrayList()) { field ->
                            FieldPath.unchecked(
                                (field as? JsonString)?.value
                                    ?: throw ListQueryException("select", "`select` must be an array of field names"),
                            )
                        }
                        else -> throw ListQueryException(
                            "select",
                            "`select` must be an array of field names such as [\"id\", \"quantity\"]",
                        )
                    }
                    "aggregate" -> if (value !== JsonNull) {
                        builder.aggregate = AggregateSpec.fromJson(value)
                    }
                    "limit" -> if (value !== JsonNull) {
                        val number = value as? JsonNumber
                        val limit = number?.takeIf { it.isInteger && !it.text.startsWith("-") }?.toBigInteger()
                            ?: throw ListQueryException("limit", "`limit` must be a positive integer")
                        builder.limit = limitOf(limit)
                    }
                    "cursor" -> when (value) {
                        JsonNull -> Unit
                        is JsonString -> builder.cursor = value.value
                        else -> throw ListQueryException("cursor", "`cursor` must be the string returned as `next_cursor`")
                    }
                    "include_total" -> when (value) {
                        JsonNull -> builder.includeTotal = false
                        is JsonBoolean -> builder.includeTotal = value.value
                        else -> throw ListQueryException("include_total", "`include_total` must be true or false")
                    }
                    else -> throw ListQueryException(
                        "query",
                        "unknown member `$name`; expected one of: ${MEMBERS.joinToString(", ")}",
                    )
                }
            }
            return builder.build()
        }

        /**
         * Decode already percent-decoded `GET` parameters with Torii's rules.
         *
         * @throws ListQueryException naming the offending parameter
         */
        @JvmStatic
        fun fromQueryPairs(pairs: List<Map.Entry<String, String>>): ListQuery {
            val builder = Builder()
            val seen = HashSet<String>()
            for ((key, value) in pairs) {
                if (key !in PARAMETERS) {
                    val hint = if (key == "aggregate") "; aggregates are only available through POST /query" else ""
                    throw ListQueryException(
                        "query",
                        "unknown parameter `$key`; expected one of: ${PARAMETERS.joinToString(", ")}$hint",
                    )
                }
                if (!seen.add(key)) throw ListQueryException(key, "`$key` must appear at most once")
                when (key) {
                    "filter" -> builder.filter = wrap("filter") { Filter.parse(value) }
                    "sort" -> builder.sort.addAll(wrap("sort") { SortKey.parseList(value) })
                    "select" -> builder.select = value.split(',').mapTo(ArrayList()) { FieldPath.unchecked(it.trim()) }
                    "limit" -> {
                        val parsed = value.takeIf { text -> text.isNotEmpty() && text.all { it in '0'..'9' } }
                            ?.let { BigInteger(it) }
                            ?: throw ListQueryException("limit", "`limit` must be a positive integer, got `$value`")
                        builder.limit = limitOf(parsed)
                    }
                    "cursor" -> builder.cursor = value
                    else -> builder.includeTotal = when (value) {
                        "true" -> true
                        "false" -> false
                        else -> throw ListQueryException(
                            "include_total",
                            "`include_total` must be `true` or `false`, got `$value`",
                        )
                    }
                }
            }
            return builder.build()
        }

        private fun limitOf(value: BigInteger): Int {
            if (value.signum() == 0) throw ListQueryException("limit", "`limit` must be at least 1")
            if (value.bitLength() > 31) throw ListQueryException("limit", "`limit` is too large")
            return value.toInt()
        }

        private inline fun <T> wrap(parameter: String, block: () -> T): T = try {
            block()
        } catch (error: ListQueryException) {
            if (error.parameter == parameter) throw error
            throw ListQueryException(parameter, error.message ?: "invalid $parameter", error)
        }

        private fun percentEncode(value: String): String =
            URLEncoder.encode(value, StandardCharsets.UTF_8.name()).replace("+", "%20")
                .replace("*", "%2A").replace("%7E", "~")
    }

    /** Builder for [ListQuery]. */
    class Builder internal constructor() {
        internal var filter: Filter? = null
        internal var filterText: String? = null
        internal val sort = ArrayList<SortKey>()
        internal var select: MutableList<FieldPath>? = null
        internal var aggregate: AggregateSpec? = null
        internal var limit: Int? = null
        internal var cursor: String? = null
        internal var includeTotal = false

        /** Keep rows matching [filter] (replaces any previous filter). */
        fun filter(filter: Filter?): Builder = apply {
            this.filter = filter
            this.filterText = null
        }

        /** Keep rows matching the text filter [text], passed to Torii unchanged. */
        fun filter(text: String): Builder = apply {
            this.filterText = text
            this.filter = null
        }

        /** AND [filter] into the current tree filter. */
        fun and(filter: Filter): Builder = apply {
            check(filterText == null) { "a text filter cannot be combined with a tree; parse it with Filter.parse first" }
            this.filter = this.filter?.let { it and filter } ?: filter
        }

        /** Append sort keys (the first key is the most significant). */
        fun sort(vararg keys: SortKey): Builder = apply { sort.addAll(keys) }

        /** Append sort keys parsed from a specification such as `-quantity,id`. */
        fun sort(specification: String): Builder = apply { sort.addAll(SortKey.parseList(specification)) }

        /** Return only these fields per item. */
        fun select(vararg fields: String): Builder = apply {
            select = fields.mapTo(ArrayList()) { FieldPath.forParameter(it, "select") }
        }

        /** Return only these fields per item. */
        fun select(fields: List<FieldPath>): Builder = apply { select = ArrayList(fields) }

        /** Return grouped metrics instead of items (`POST` only). */
        fun aggregate(spec: AggregateSpec?): Builder = apply { aggregate = spec }

        /** Rows per page (1..`torii.app_api_max_list_limit`). */
        fun limit(limit: Int): Builder = apply { this.limit = limit }

        /** Continue after a previous page's `next_cursor` (`null` starts from the first page). */
        fun cursor(cursor: String?): Builder = apply { this.cursor = cursor }

        /** Ask for the exact number of matching rows. */
        @JvmOverloads
        fun includeTotal(include: Boolean = true): Builder = apply { includeTotal = include }

        /**
         * Validate every control (Rust `ListQuery::validate`).
         *
         * @throws ListQueryException naming the first invalid control
         */
        fun build(): ListQuery {
            filter?.let {
                try {
                    it.validate()
                } catch (error: InvalidFilterException) {
                    throw ListQueryException("filter", error.message ?: "invalid filter", error)
                }
            }
            if (sort.size > QueryText.SORT_MAX_KEYS) {
                throw ListQueryException("sort", "at most ${QueryText.SORT_MAX_KEYS} sort keys are allowed")
            }
            sort.forEachIndexed { index, key ->
                FieldPath.validationError(key.field.path)?.let { throw ListQueryException("sort", it) }
                if (sort.subList(0, index).any { it.field == key.field }) {
                    throw ListQueryException("sort", "sort key `${key.field}` appears more than once")
                }
            }
            select?.let { fields ->
                if (fields.isEmpty()) throw ListQueryException("select", "`select` must list at least one field")
                if (fields.size > SELECT_MAX_FIELDS) {
                    throw ListQueryException("select", "at most $SELECT_MAX_FIELDS fields can be selected")
                }
                fields.forEachIndexed { index, path ->
                    FieldPath.validationError(path.path)?.let { throw ListQueryException("select", it) }
                    if (fields.subList(0, index).contains(path)) {
                        throw ListQueryException("select", "field `$path` is selected more than once")
                    }
                }
            }
            if (select != null && aggregate != null) {
                throw ListQueryException(
                    "select",
                    "`select` and `aggregate` cannot be combined; aggregates define their own columns",
                )
            }
            limit?.let { if (it < 1) throw ListQueryException("limit", "`limit` must be at least 1") }
            cursor?.let { token ->
                if (token.isEmpty() || token.length > CURSOR_MAX_BYTES ||
                    !token.all { it in 'A'..'Z' || it in 'a'..'z' || it in '0'..'9' || it == '-' || it == '_' }
                ) {
                    throw ListQueryException("cursor", "`cursor` must be a `next_cursor` value returned by a previous page")
                }
            }
            return ListQuery(filter, filterText, sort, select, aggregate, limit, cursor, includeTotal)
        }
    }
}

/** Kotlin builder shorthand: `listQuery { filter(...); limit(50) }`. */
inline fun listQuery(configure: ListQuery.Builder.() -> Unit): ListQuery =
    ListQuery.builder().apply(configure).build()

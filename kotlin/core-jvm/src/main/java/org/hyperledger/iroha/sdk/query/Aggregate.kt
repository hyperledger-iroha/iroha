package org.hyperledger.iroha.sdk.query

import java.util.Collections
import org.hyperledger.iroha.sdk.json.JsonArray
import org.hyperledger.iroha.sdk.json.JsonObject
import org.hyperledger.iroha.sdk.json.JsonString

/** Aggregate functions; [jsonName] is the wire spelling. */
enum class AggregateFunction(val jsonName: String) {
    /** Number of rows in the group (takes no field). */
    COUNT("count"),

    /** Sum of a numeric field. */
    SUM("sum"),

    /** Minimum of a numeric field. */
    MIN("min"),

    /** Maximum of a numeric field. */
    MAX("max"),

    /** Average of a numeric field. */
    AVG("avg"),

    /** Number of distinct values of a scalar field. */
    DISTINCT_COUNT("distinct_count"),
}

/** One metric computed per group; [alias] names the output column, usable in `having` and `sort`. */
class AggregateMetric private constructor(
    @JvmField val alias: String,
    @JvmField val function: AggregateFunction,
    @JvmField val field: FieldPath?,
) {
    internal fun toJson(): JsonObject {
        val members = linkedMapOf("alias" to JsonString(alias), "fn" to JsonString(function.jsonName))
        field?.let { members["field"] = JsonString(it.path) }
        return JsonObject(members)
    }

    override fun equals(other: Any?): Boolean =
        other is AggregateMetric && other.alias == alias && other.function == function && other.field == field

    override fun hashCode(): Int = (alias.hashCode() * 31 + function.hashCode()) * 31 + (field?.hashCode() ?: 0)

    companion object {
        /** `count` of the rows in each group. */
        @JvmStatic
        fun count(alias: String): AggregateMetric = AggregateMetric(requireAlias(alias), AggregateFunction.COUNT, null)

        /** [function] applied to [field]; use [count] for row counts. */
        @JvmStatic
        fun of(alias: String, function: AggregateFunction, field: String): AggregateMetric {
            require(function != AggregateFunction.COUNT) { "`count` takes no field; use AggregateMetric.count(alias)" }
            return AggregateMetric(requireAlias(alias), function, FieldPath.forParameter(field, "aggregate"))
        }

        @JvmStatic
        fun sum(alias: String, field: String): AggregateMetric = of(alias, AggregateFunction.SUM, field)

        @JvmStatic
        fun min(alias: String, field: String): AggregateMetric = of(alias, AggregateFunction.MIN, field)

        @JvmStatic
        fun max(alias: String, field: String): AggregateMetric = of(alias, AggregateFunction.MAX, field)

        @JvmStatic
        fun avg(alias: String, field: String): AggregateMetric = of(alias, AggregateFunction.AVG, field)

        @JvmStatic
        fun distinctCount(alias: String, field: String): AggregateMetric =
            of(alias, AggregateFunction.DISTINCT_COUNT, field)

        private fun requireAlias(alias: String): String {
            require(alias.isNotEmpty()) { "aggregate metric aliases must not be empty" }
            return alias
        }
    }
}

/**
 * Grouping and metrics evaluated after filtering and before pagination (`POST /query` only).
 *
 * Aggregates are computed where the rows live: Torii rejects a read whose visible rows span
 * several dataspace routes with `invalid_aggregate` (overlapping routes cannot be summed
 * exactly); page through the rows without `aggregate` instead. History collections
 * (transactions) never accept aggregates.
 *
 * ```kotlin
 * AggregateSpec.builder()
 *     .groupBy("asset")
 *     .metric(AggregateMetric.count("holders"))
 *     .metric(AggregateMetric.sum("supply", "quantity"))
 *     .having(field("holders") gte 10)
 *     .build()
 * ```
 */
class AggregateSpec private constructor(
    groupBy: List<FieldPath>,
    metrics: List<AggregateMetric>,
    /** Filter applied to the aggregated rows (group fields and metric aliases). */
    @JvmField val having: Filter?,
    /** A text `having` filter passed to Torii unchanged. */
    @JvmField val havingText: String?,
) {
    /** Grouping dimensions. */
    @JvmField
    val groupBy: List<FieldPath> = Collections.unmodifiableList(ArrayList(groupBy))

    /** Metrics computed per group. */
    @JvmField
    val metrics: List<AggregateMetric> = Collections.unmodifiableList(ArrayList(metrics))

    /** The `aggregate` member of a `POST /query` body. */
    fun toJson(): JsonObject {
        val members = LinkedHashMap<String, org.hyperledger.iroha.sdk.json.Json>()
        if (groupBy.isNotEmpty()) members["group_by"] = JsonArray(groupBy.map { JsonString(it.path) })
        members["metrics"] = JsonArray(metrics.map(AggregateMetric::toJson))
        having?.let { members["having"] = it.toJson() }
        havingText?.let { members["having"] = JsonString(it) }
        return JsonObject(members)
    }

    override fun equals(other: Any?): Boolean =
        other is AggregateSpec && other.groupBy == groupBy && other.metrics == metrics &&
            other.having == having && other.havingText == havingText

    override fun hashCode(): Int =
        ((groupBy.hashCode() * 31 + metrics.hashCode()) * 31 + (having?.hashCode() ?: 0)) * 31 +
            (havingText?.hashCode() ?: 0)

    companion object {
        /** Maximum number of `group_by` fields in one aggregate. */
        const val MAX_GROUP_BY: Int = 8

        /** Maximum number of metrics in one aggregate. */
        const val MAX_METRICS: Int = 16

        @JvmStatic
        fun builder(): Builder = Builder()

        /**
         * Decode the `aggregate` member of a `POST /query` body.
         *
         * @throws ListQueryException with parameter `aggregate`
         */
        @JvmStatic
        fun fromJson(json: org.hyperledger.iroha.sdk.json.Json): AggregateSpec {
            fun invalid(reason: String): Nothing = throw ListQueryException("aggregate", reason)
            if (json !is JsonObject) invalid("`aggregate` must be an object with `group_by`, `metrics` and `having`")
            val builder = Builder()
            for ((name, value) in json.members) {
                when (name) {
                    "group_by" -> {
                        val fields = value as? JsonArray ?: invalid("`group_by` must be an array of field names")
                        fields.items.forEach { field ->
                            val path = (field as? JsonString)?.value ?: invalid("`group_by` must be an array of field names")
                            builder.groupBy(path)
                        }
                    }
                    "metrics" -> {
                        val metrics = value as? JsonArray ?: invalid("`metrics` must be an array of metrics")
                        metrics.items.forEach { metric -> builder.metric(metricFromJson(metric)) }
                    }
                    "having" -> when (value) {
                        org.hyperledger.iroha.sdk.json.JsonNull -> Unit
                        is JsonString -> builder.having(value.value)
                        else -> builder.having(
                            try {
                                Filter.fromJson(value)
                            } catch (error: ListQueryException) {
                                throw ListQueryException("aggregate", "having: ${error.message}", error)
                            },
                        )
                    }
                    else -> invalid("unknown aggregate member `$name`; expected group_by, metrics, having")
                }
            }
            return builder.build()
        }

        private fun metricFromJson(json: org.hyperledger.iroha.sdk.json.Json): AggregateMetric {
            fun invalid(reason: String): Nothing = throw ListQueryException("aggregate", reason)
            if (json !is JsonObject) invalid("each metric must be an object such as {\"alias\": \"n\", \"fn\": \"count\"}")
            json.keys.firstOrNull { it != "alias" && it != "fn" && it != "field" }?.let {
                invalid("unknown metric member `$it`; expected alias, fn, field")
            }
            val alias = (json["alias"] as? JsonString)?.value ?: invalid("each metric needs a string `alias`")
            val name = (json["fn"] as? JsonString)?.value ?: invalid("each metric needs a string `fn`")
            val function = AggregateFunction.values().firstOrNull { it.jsonName == name }
                ?: invalid("unknown aggregate function `$name`; expected one of: count, sum, min, max, avg, distinct_count")
            val field = when (val raw = json["field"]) {
                null, org.hyperledger.iroha.sdk.json.JsonNull -> null
                is JsonString -> raw.value
                else -> invalid("metric `field` must be a field name")
            }
            return try {
                if (field == null) {
                    if (function != AggregateFunction.COUNT) invalid("`${function.jsonName}` needs a `field`")
                    AggregateMetric.count(alias)
                } else {
                    AggregateMetric.of(alias, function, field)
                }
            } catch (error: IllegalArgumentException) {
                if (error is ListQueryException) throw error
                throw ListQueryException("aggregate", error.message ?: "invalid metric", error)
            }
        }
    }

    /**
     * Builder for [AggregateSpec]: at least one and at most [MAX_METRICS] metrics, and at most
     * [MAX_GROUP_BY] grouping fields.
     */
    class Builder internal constructor() {
        private val groupBy = ArrayList<FieldPath>()
        private val metrics = ArrayList<AggregateMetric>()
        private var having: Filter? = null
        private var havingText: String? = null

        /** Add grouping fields in order. */
        fun groupBy(vararg fields: String): Builder =
            apply { fields.mapTo(groupBy) { FieldPath.forParameter(it, "aggregate") } }

        /** Add a metric. */
        fun metric(metric: AggregateMetric): Builder = apply { metrics.add(metric) }

        /** Keep only groups matching [filter]. */
        fun having(filter: Filter): Builder = apply {
            having = filter
            havingText = null
        }

        /** Keep only groups matching the text filter [filter], passed to Torii unchanged. */
        fun having(filter: String): Builder = apply {
            havingText = filter
            having = null
        }

        fun build(): AggregateSpec {
            if (metrics.isEmpty()) throw ListQueryException("aggregate", "`metrics` must list at least one metric")
            if (groupBy.size > MAX_GROUP_BY) {
                throw ListQueryException("aggregate", "`group_by` lists at most $MAX_GROUP_BY fields")
            }
            if (metrics.size > MAX_METRICS) {
                throw ListQueryException("aggregate", "`metrics` lists at most $MAX_METRICS metrics")
            }
            having?.let {
                try {
                    it.validate()
                } catch (error: InvalidFilterException) {
                    throw ListQueryException("aggregate", "having: ${error.message}", error)
                }
            }
            return AggregateSpec(groupBy, metrics, having, havingText)
        }
    }
}

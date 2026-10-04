package org.hyperledger.iroha.sdk.query

import java.math.BigDecimal
import java.math.BigInteger
import java.util.Collections
import org.hyperledger.iroha.sdk.json.Json
import org.hyperledger.iroha.sdk.json.JsonArray
import org.hyperledger.iroha.sdk.json.JsonBoolean
import org.hyperledger.iroha.sdk.json.JsonNull
import org.hyperledger.iroha.sdk.json.JsonNumber
import org.hyperledger.iroha.sdk.json.JsonObject
import org.hyperledger.iroha.sdk.json.JsonString
import org.hyperledger.iroha.sdk.json.JsonSyntaxException

/**
 * A Torii collection filter: the same tree behind the text form
 * (`owned_by = "alice" and quantity >= 10.5`) and the JSON form
 * (`{"op":"and","args":[...]}`) of `specs/torii/collection_queries.md`.
 *
 * Build filters with [field] and combine them with [and], [or] and [not]:
 * ```kotlin
 * val filter = field("owned_by") eq "alice" and (field("quantity") gte BigDecimal("10.5"))
 * filter.toString()  // owned_by = "alice" and quantity >= "10.5"
 * ```
 * Java: `Filter.field("owned_by").eq("alice").and(Filter.field("quantity").gte(new BigDecimal("10.5")))`.
 *
 * [toString] is the canonical text rendering, [toJson] the canonical JSON form; [parse] and
 * [fromJson] read either form back into the same tree.
 */
sealed class Filter {
    /** `self and other`; chains of `and` are flattened. */
    infix fun and(other: Filter): Filter = And(andOperands(this) + andOperands(other))

    /** `self or other`; chains of `or` are flattened. */
    infix fun or(other: Filter): Filter = Or(orOperands(this) + orOperands(other))

    /** `not self` (Kotlin `!filter`). */
    operator fun not(): Filter = Not(this)

    /** Canonical JSON form (`{"op": ..., "args": [...]}`). */
    fun toJson(): JsonObject = QueryJson.filterToJson(this)

    /**
     * Canonical text form; [parse] reads it back to the same tree. Object and array literals
     * (valid only against `metadata.<key>`) exist only in the JSON form: they are rendered here as
     * JSON for display, but such filters must be sent with [toJson] (`POST /query`).
     */
    override fun toString(): String = QueryText.renderFilter(this)

    /**
     * Check the structural limits (depth 10, 1,024 nodes, 1,024 values per list, 4,096 list
     * values in total) and operand shapes of the whole tree.
     *
     * @throws InvalidFilterException naming the first violation
     */
    fun validate() {
        FilterValidation.validate(this)
    }

    /** All nested filters match. */
    class And internal constructor(operands: List<Filter>) : Filter() {
        @JvmField
        val operands: List<Filter> = Collections.unmodifiableList(ArrayList(operands))

        override fun equals(other: Any?): Boolean = other is And && other.operands == operands

        override fun hashCode(): Int = 31 + operands.hashCode()
    }

    /** At least one nested filter matches. */
    class Or internal constructor(operands: List<Filter>) : Filter() {
        @JvmField
        val operands: List<Filter> = Collections.unmodifiableList(ArrayList(operands))

        override fun equals(other: Any?): Boolean = other is Or && other.operands == operands

        override fun hashCode(): Int = 37 + operands.hashCode()
    }

    /** The nested filter does not match. */
    class Not internal constructor(@JvmField val operand: Filter) : Filter() {
        override fun equals(other: Any?): Boolean = other is Not && other.operand == operand

        override fun hashCode(): Int = 41 + operand.hashCode()
    }

    /** `field <op> value`. `!=` also matches rows where the field is absent. */
    class Comparison internal constructor(
        @JvmField val field: FieldPath,
        @JvmField val operator: ComparisonOperator,
        @JvmField val value: Json,
    ) : Filter() {
        override fun equals(other: Any?): Boolean =
            other is Comparison && other.field == field && other.operator == operator && other.value == value

        override fun hashCode(): Int = (field.hashCode() * 31 + operator.hashCode()) * 31 + value.hashCode()
    }

    /** `field in [...]` or `field not in [...]`; `not in` also matches absent fields. */
    class Membership internal constructor(
        @JvmField val field: FieldPath,
        @JvmField val operator: MembershipOperator,
        values: List<Json>,
    ) : Filter() {
        @JvmField
        val values: List<Json> = Collections.unmodifiableList(ArrayList(values))

        override fun equals(other: Any?): Boolean =
            other is Membership && other.field == field && other.operator == operator && other.values == values

        override fun hashCode(): Int = (field.hashCode() * 31 + operator.hashCode()) * 31 + values.hashCode()
    }

    /** `exists(field)`: the field is present. */
    class Exists internal constructor(@JvmField val field: FieldPath) : Filter() {
        override fun equals(other: Any?): Boolean = other is Exists && other.field == field

        override fun hashCode(): Int = 43 + field.hashCode()
    }

    /** `field is null`: the field is absent or null. `not` of it renders as `field is not null`. */
    class IsNull internal constructor(@JvmField val field: FieldPath) : Filter() {
        override fun equals(other: Any?): Boolean = other is IsNull && other.field == field

        override fun hashCode(): Int = 47 + field.hashCode()
    }

    companion object {
        /** Start a predicate or sort key on a dotted field path such as `metadata.tier`. */
        @JvmStatic
        fun field(path: String): Field = Field(FieldPath.forParameter(path, "filter"))

        /** Start a predicate or sort key on [path]. */
        @JvmStatic
        fun field(path: FieldPath): Field = Field(path)

        /**
         * Parse the text form, e.g. `owned_by = "alice" and quantity >= 10`.
         *
         * @throws FilterSyntaxException with the line and column of the offending token
         */
        @JvmStatic
        fun parse(text: String): Filter = QueryText.parseFilter(text)

        /**
         * Decode the JSON form, or the text form when [json] is a JSON string.
         *
         * @throws InvalidFilterException naming the offending node, e.g. `args[1].args[0]`
         * @throws FilterSyntaxException when a text filter does not parse
         */
        @JvmStatic
        fun fromJson(json: Json): Filter = when (json) {
            is JsonString -> parse(json.value)
            else -> QueryJson.filterFromJson(json)
        }

        /** Decode the JSON form from JSON text. */
        @JvmStatic
        fun fromJson(json: String): Filter = try {
            fromJson(Json.parse(json))
        } catch (error: JsonSyntaxException) {
            throw InvalidFilterException("filter JSON is malformed: ${error.message}", null, error)
        }

        /** Conjunction of [filters], or `null` when there are none. */
        @JvmStatic
        fun all(filters: Iterable<Filter>): Filter? = filters.reduceOrNull { left, right -> left and right }

        /** Conjunction of [filters]. */
        @JvmStatic
        fun all(first: Filter, vararg rest: Filter): Filter = rest.fold(first) { left, right -> left and right }

        /** Disjunction of [filters], or `null` when there are none. */
        @JvmStatic
        fun any(filters: Iterable<Filter>): Filter? = filters.reduceOrNull { left, right -> left or right }

        /** Disjunction of [filters]. */
        @JvmStatic
        fun any(first: Filter, vararg rest: Filter): Filter = rest.fold(first) { left, right -> left or right }

        private fun andOperands(filter: Filter): List<Filter> =
            if (filter is And) filter.operands else listOf(filter)

        private fun orOperands(filter: Filter): List<Filter> =
            if (filter is Or) filter.operands else listOf(filter)
    }
}

/** Comparison operators; [symbol] is the canonical text spelling, [jsonName] the JSON `op`. */
enum class ComparisonOperator(val jsonName: String, val symbol: String) {
    EQ("eq", "="),
    NE("ne", "!="),
    LT("lt", "<"),
    LTE("lte", "<="),
    GT("gt", ">"),
    GTE("gte", ">="),
}

/** Membership operators; [symbol] is the canonical text spelling, [jsonName] the JSON `op`. */
enum class MembershipOperator(val jsonName: String, val symbol: String) {
    IN("in", "in"),
    NOT_IN("nin", "not in"),
}

/**
 * A field awaiting an operator; see [Filter.field]. Reusable: every method returns a new filter.
 *
 * Literals follow the wire rules: integers that fit `u64`/`i64` are JSON numbers; decimals and
 * wider integers become exact decimal strings (there are no `double` overloads, so values never
 * pass through IEEE floating point). [Json] literals are accepted for `metadata.*` values.
 */
class Field internal constructor(
    /** The field path. */
    @JvmField val path: FieldPath,
) {
    infix fun eq(value: String): Filter = compare(ComparisonOperator.EQ, Literals.of(value))
    infix fun eq(value: Long): Filter = compare(ComparisonOperator.EQ, Literals.of(value))
    infix fun eq(value: Int): Filter = compare(ComparisonOperator.EQ, Literals.of(value.toLong()))
    infix fun eq(value: BigInteger): Filter = compare(ComparisonOperator.EQ, Literals.of(value))
    infix fun eq(value: BigDecimal): Filter = compare(ComparisonOperator.EQ, Literals.of(value))
    infix fun eq(value: Boolean): Filter = compare(ComparisonOperator.EQ, Literals.of(value))
    infix fun eq(value: Json): Filter = compare(ComparisonOperator.EQ, Literals.of(value))

    infix fun ne(value: String): Filter = compare(ComparisonOperator.NE, Literals.of(value))
    infix fun ne(value: Long): Filter = compare(ComparisonOperator.NE, Literals.of(value))
    infix fun ne(value: Int): Filter = compare(ComparisonOperator.NE, Literals.of(value.toLong()))
    infix fun ne(value: BigInteger): Filter = compare(ComparisonOperator.NE, Literals.of(value))
    infix fun ne(value: BigDecimal): Filter = compare(ComparisonOperator.NE, Literals.of(value))
    infix fun ne(value: Boolean): Filter = compare(ComparisonOperator.NE, Literals.of(value))
    infix fun ne(value: Json): Filter = compare(ComparisonOperator.NE, Literals.of(value))

    infix fun lt(value: String): Filter = compare(ComparisonOperator.LT, Literals.of(value))
    infix fun lt(value: Long): Filter = compare(ComparisonOperator.LT, Literals.of(value))
    infix fun lt(value: Int): Filter = compare(ComparisonOperator.LT, Literals.of(value.toLong()))
    infix fun lt(value: BigInteger): Filter = compare(ComparisonOperator.LT, Literals.of(value))
    infix fun lt(value: BigDecimal): Filter = compare(ComparisonOperator.LT, Literals.of(value))
    infix fun lt(value: Json): Filter = compare(ComparisonOperator.LT, Literals.of(value))

    infix fun lte(value: String): Filter = compare(ComparisonOperator.LTE, Literals.of(value))
    infix fun lte(value: Long): Filter = compare(ComparisonOperator.LTE, Literals.of(value))
    infix fun lte(value: Int): Filter = compare(ComparisonOperator.LTE, Literals.of(value.toLong()))
    infix fun lte(value: BigInteger): Filter = compare(ComparisonOperator.LTE, Literals.of(value))
    infix fun lte(value: BigDecimal): Filter = compare(ComparisonOperator.LTE, Literals.of(value))
    infix fun lte(value: Json): Filter = compare(ComparisonOperator.LTE, Literals.of(value))

    infix fun gt(value: String): Filter = compare(ComparisonOperator.GT, Literals.of(value))
    infix fun gt(value: Long): Filter = compare(ComparisonOperator.GT, Literals.of(value))
    infix fun gt(value: Int): Filter = compare(ComparisonOperator.GT, Literals.of(value.toLong()))
    infix fun gt(value: BigInteger): Filter = compare(ComparisonOperator.GT, Literals.of(value))
    infix fun gt(value: BigDecimal): Filter = compare(ComparisonOperator.GT, Literals.of(value))
    infix fun gt(value: Json): Filter = compare(ComparisonOperator.GT, Literals.of(value))

    infix fun gte(value: String): Filter = compare(ComparisonOperator.GTE, Literals.of(value))
    infix fun gte(value: Long): Filter = compare(ComparisonOperator.GTE, Literals.of(value))
    infix fun gte(value: Int): Filter = compare(ComparisonOperator.GTE, Literals.of(value.toLong()))
    infix fun gte(value: BigInteger): Filter = compare(ComparisonOperator.GTE, Literals.of(value))
    infix fun gte(value: BigDecimal): Filter = compare(ComparisonOperator.GTE, Literals.of(value))
    infix fun gte(value: Json): Filter = compare(ComparisonOperator.GTE, Literals.of(value))

    /** `field in [values...]`. */
    fun isIn(vararg values: String): Filter = member(MembershipOperator.IN, values.map(Literals::of))

    /** `field in [values...]`. */
    fun isIn(vararg values: Long): Filter = member(MembershipOperator.IN, values.map(Literals::of))

    /** `field in [values...]`. */
    fun isIn(vararg values: Int): Filter = member(MembershipOperator.IN, values.map { Literals.of(it.toLong()) })

    /** `field in [values...]`; values are converted with the literal rules. */
    fun isIn(values: Iterable<Any>): Filter = member(MembershipOperator.IN, values.map(Literals::ofAny))

    /** `field not in [values...]` (also matches rows where the field is absent). */
    fun notIn(vararg values: String): Filter = member(MembershipOperator.NOT_IN, values.map(Literals::of))

    /** `field not in [values...]` (also matches rows where the field is absent). */
    fun notIn(vararg values: Long): Filter = member(MembershipOperator.NOT_IN, values.map(Literals::of))

    /** `field not in [values...]` (also matches rows where the field is absent). */
    fun notIn(vararg values: Int): Filter =
        member(MembershipOperator.NOT_IN, values.map { Literals.of(it.toLong()) })

    /** `field not in [values...]`; values are converted with the literal rules. */
    fun notIn(values: Iterable<Any>): Filter = member(MembershipOperator.NOT_IN, values.map(Literals::ofAny))

    /** `exists(field)`. */
    fun exists(): Filter = Filter.Exists(path)

    /** `field is null` (absent or null). */
    fun isNull(): Filter = Filter.IsNull(path)

    /** `field is not null`. */
    fun isNotNull(): Filter = Filter.Not(Filter.IsNull(path))

    /** Ascending sort key on this field. */
    fun asc(): SortKey = SortKey.asc(path)

    /** Descending sort key on this field. */
    fun desc(): SortKey = SortKey.desc(path)

    /** Canonical text spelling of the path. */
    override fun toString(): String = path.toString()

    override fun equals(other: Any?): Boolean = other is Field && other.path == path

    override fun hashCode(): Int = path.hashCode()

    private fun compare(operator: ComparisonOperator, value: Json): Filter {
        FilterValidation.comparisonError(path, operator, value)?.let { throw it }
        return Filter.Comparison(path, operator, value)
    }

    private fun member(operator: MembershipOperator, values: List<Json>): Filter {
        FilterValidation.membershipError(path, values)?.let { throw it }
        return Filter.Membership(path, operator, values)
    }
}

/** Start a predicate or sort key on a dotted field path (Kotlin shorthand for [Filter.field]). */
fun field(path: String): Field = Filter.field(path)

/** Conversion of host values into filter literals following the text-grammar rules. */
internal object Literals {
    private val U64_MAX = BigInteger("18446744073709551615")
    private val I64_MIN = BigInteger.valueOf(Long.MIN_VALUE)

    fun of(value: String): Json = JsonString(value)

    fun of(value: Long): Json = JsonNumber.of(value)

    fun of(value: Boolean): Json = JsonBoolean.of(value)

    fun of(value: BigInteger): Json =
        if (value in I64_MIN..U64_MAX) JsonNumber.of(value) else JsonString(value.toString())

    fun of(value: BigDecimal): Json = numberValue(value.toPlainString())

    /** Normalise caller-supplied JSON numbers with the same rules; other JSON passes through. */
    fun of(value: Json): Json = when (value) {
        is JsonNumber -> {
            val decimal = value.toBigDecimal()
            numberValue(if (value.isInteger) value.text else decimal.toPlainString())
        }
        else -> value
    }

    fun ofAny(value: Any): Json = when (value) {
        is String -> of(value)
        is Long -> of(value)
        is Int -> of(value.toLong())
        is Short -> of(value.toLong())
        is Byte -> of(value.toLong())
        is BigInteger -> of(value)
        is BigDecimal -> of(value)
        is Boolean -> of(value)
        is Json -> of(value)
        is Double, is Float -> throw IllegalArgumentException(
            "floating-point literals are not exact; use BigDecimal or a decimal string",
        )
        else -> throw IllegalArgumentException(
            "unsupported filter literal type ${value.javaClass.name}; use String, Long, BigInteger, BigDecimal, Boolean or Json",
        )
    }

    /** Rust `number_value`: integers fitting u64/i64 are numbers, everything else exact strings. */
    fun numberValue(raw: String): Json {
        if (raw.indexOf('.') < 0) {
            val integer = try {
                BigInteger(raw)
            } catch (_: NumberFormatException) {
                null
            }
            if (integer != null && integer in I64_MIN..U64_MAX && raw == integer.toString()) {
                return JsonNumber.of(integer)
            }
            if (integer != null && raw == "-0") return JsonNumber.of(0L)
        }
        return JsonString(raw)
    }

    /** Rust `is_numeric_literal`: finite JSON numbers and canonical decimal strings. */
    fun isNumeric(value: Json): Boolean = when (value) {
        is JsonNumber -> value.toBigDecimal().toDouble().let { !it.isInfinite() && !it.isNaN() }
        is JsonString -> isDecimalText(value.value)
        else -> false
    }

    /** `-?(0|[1-9][0-9]*)(\.[0-9]+)?` */
    fun isDecimalText(text: String): Boolean {
        val unsigned = if (text.startsWith("-")) text.substring(1) else text
        val dot = unsigned.indexOf('.')
        val integer = if (dot >= 0) unsigned.substring(0, dot) else unsigned
        val fraction = if (dot >= 0) unsigned.substring(dot + 1) else null
        val integerOk = integer == "0" ||
            (integer.isNotEmpty() && integer[0] in '1'..'9' && integer.all { it in '0'..'9' })
        val fractionOk = fraction == null || (fraction.isNotEmpty() && fraction.all { it in '0'..'9' })
        return integerOk && fractionOk
    }

    fun isScalar(value: Json): Boolean = value !is JsonArray && value !is JsonObject
}

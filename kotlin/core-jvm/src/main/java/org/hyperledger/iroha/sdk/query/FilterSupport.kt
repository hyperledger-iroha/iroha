package org.hyperledger.iroha.sdk.query

import org.hyperledger.iroha.sdk.json.Json
import org.hyperledger.iroha.sdk.json.JsonArray
import org.hyperledger.iroha.sdk.json.JsonBoolean
import org.hyperledger.iroha.sdk.json.JsonNull
import org.hyperledger.iroha.sdk.json.JsonNumber
import org.hyperledger.iroha.sdk.json.JsonObject
import org.hyperledger.iroha.sdk.json.JsonString

/** Structural limits and operand rules shared by builders, the JSON decoder and the text parser. */
internal object FilterValidation {
    /** Maximum nesting depth (a single leaf has depth 0). */
    const val MAX_DEPTH = 10

    /** Maximum operator nodes in one filter. */
    const val MAX_NODES = 1_024

    /** Maximum literals in one `in` / `not in` list. */
    const val MAX_MEMBERSHIP_VALUES = 1_024

    /** Maximum list literals across one filter. */
    const val MAX_TOTAL_MEMBERSHIP_VALUES = 4_096

    class Budget {
        var nodes = 0
        var membershipValues = 0

        fun enter(depth: Int) {
            if (depth > MAX_DEPTH) throw limit("nesting depth", MAX_DEPTH)
            nodes += 1
            if (nodes > MAX_NODES) throw limit("node count", MAX_NODES)
        }

        fun membership(field: FieldPath, values: List<Json>) {
            values.forEach { value -> inexactNumberError(field, value)?.let { throw it } }
            if (values.isEmpty()) throw operand(field, "membership lists must not be empty")
            if (values.size > MAX_MEMBERSHIP_VALUES) {
                throw limit("membership list size", MAX_MEMBERSHIP_VALUES)
            }
            membershipValues += values.size
            if (membershipValues > MAX_TOTAL_MEMBERSHIP_VALUES) {
                throw limit("total membership values", MAX_TOTAL_MEMBERSHIP_VALUES)
            }
            membershipShapeError(field, values)?.let { throw it }
        }
    }

    fun validate(filter: Filter) {
        validate(filter, 0, Budget())
    }

    private fun validate(filter: Filter, depth: Int, budget: Budget) {
        budget.enter(depth)
        when (filter) {
            is Filter.And -> validateList("and", filter.operands, depth, budget)
            is Filter.Or -> validateList("or", filter.operands, depth, budget)
            is Filter.Not -> validate(filter.operand, depth + 1, budget)
            is Filter.Comparison -> {
                pathError(filter.field)?.let { throw it }
                comparisonError(filter.field, filter.operator, filter.value)?.let { throw it }
            }
            is Filter.Membership -> {
                pathError(filter.field)?.let { throw it }
                budget.membership(filter.field, filter.values)
            }
            is Filter.Exists -> pathError(filter.field)?.let { throw it }
            is Filter.IsNull -> pathError(filter.field)?.let { throw it }
        }
    }

    private fun validateList(op: String, operands: List<Filter>, depth: Int, budget: Budget) {
        if (operands.isEmpty()) throw InvalidFilterException("`$op` needs at least one operand")
        operands.forEach { validate(it, depth + 1, budget) }
    }

    fun pathError(path: FieldPath): InvalidFilterException? =
        FieldPath.validationError(path.path)?.let { InvalidFilterException(it, path.path) }

    /** Operand rules for comparisons (Rust `validate_rec` leaf checks). */
    fun comparisonError(field: FieldPath, operator: ComparisonOperator, value: Json): InvalidFilterException? =
        inexactNumberError(field, value) ?: when (operator) {
            ComparisonOperator.EQ, ComparisonOperator.NE ->
                if (!Literals.isScalar(value) && !isMetadata(field)) {
                    operand(field, "comparison literals must be strings, numbers, booleans or null")
                } else {
                    null
                }
            else ->
                if (Literals.isNumeric(value) || value is JsonString) {
                    null
                } else {
                    operand(field, "range comparisons need a number, decimal or string literal")
                }
        }

    /** Local membership rules (emptiness, size, uniqueness, one literal type). */
    fun membershipError(field: FieldPath, values: List<Json>): InvalidFilterException? {
        values.forEach { value -> inexactNumberError(field, value)?.let { return it } }
        if (values.isEmpty()) return operand(field, "membership lists must not be empty")
        if (values.size > MAX_MEMBERSHIP_VALUES) return limit("membership list size", MAX_MEMBERSHIP_VALUES)
        return membershipShapeError(field, values)
    }

    private fun membershipShapeError(field: FieldPath, values: List<Json>): InvalidFilterException? {
        if (values.toHashSet().size != values.size) {
            return operand(field, "membership list values must be unique")
        }
        val homogeneous = values.all { it is JsonString } ||
            values.all(Literals::isNumeric) ||
            values.all { it is JsonBoolean }
        if (!homogeneous && !isMetadata(field)) {
            return operand(field, "membership list values must all be strings, numbers or booleans")
        }
        return null
    }

    /**
     * Torii rejects JSON numbers that are not exact integers (fractions, exponents, beyond `u128`);
     * decimals are written as strings. Arrays are checked element by element.
     */
    fun inexactNumberError(field: FieldPath, value: Json): InvalidFilterException? = when (value) {
        is JsonNumber -> if (isExactInteger(value)) {
            null
        } else {
            operand(field, "fractional JSON numbers are not exact; write decimals as strings such as \"1.5\"")
        }
        is JsonArray -> value.items.firstNotNullOfOrNull { inexactNumberError(field, it) }
        is JsonObject -> value.members.values.firstNotNullOfOrNull { inexactNumberError(field, it) }
        else -> null
    }

    /**
     * The first field compared against an object or array literal. Such literals (valid only for
     * `metadata.<key>`) exist only in the JSON form, so these filters cannot be sent as text.
     */
    fun structuredLiteralField(filter: Filter): FieldPath? = when (filter) {
        is Filter.And -> filter.operands.firstNotNullOfOrNull(::structuredLiteralField)
        is Filter.Or -> filter.operands.firstNotNullOfOrNull(::structuredLiteralField)
        is Filter.Not -> structuredLiteralField(filter.operand)
        is Filter.Comparison -> filter.field.takeIf { !Literals.isScalar(filter.value) }
        is Filter.Membership -> filter.field.takeIf { filter.values.any { !Literals.isScalar(it) } }
        is Filter.Exists, is Filter.IsNull -> null
    }

    /** Reject [filter] for text use (`GET` parameters, event streams) when it needs the JSON form. */
    fun requireTextRepresentable(filter: Filter, use: String) {
        structuredLiteralField(filter)?.let { field ->
            throw InvalidFilterException(
                "`${field.path}` is compared with an object or array literal, which exists only in the JSON form; " +
                    "$use cannot carry it, send the filter with POST /query instead",
                field.path,
            )
        }
    }

    private val I64_MIN = java.math.BigInteger.valueOf(Long.MIN_VALUE)
    private val U128_MAX = java.math.BigInteger.ONE.shiftLeft(128).subtract(java.math.BigInteger.ONE)

    private fun isExactInteger(number: JsonNumber): Boolean {
        if (!number.isInteger) return false
        val integer = number.toBigInteger()
        return integer >= I64_MIN && integer <= U128_MAX
    }

    private fun isMetadata(field: FieldPath): Boolean = field.path.startsWith("metadata.")

    private fun operand(field: FieldPath, reason: String) =
        InvalidFilterException("invalid operand for `${field.path}`: $reason", field.path)

    private fun limit(limit: String, max: Int) =
        InvalidFilterException("filter exceeds the $limit limit of $max")
}

/** The canonical JSON form of filters (`{"op": ..., "args": [...]}`). */
internal object QueryJson {
    private const val OPERATORS = "and, or, not, eq, ne, lt, lte, gt, gte, in, nin, exists, is_null"

    fun filterToJson(filter: Filter): JsonObject {
        val (op, args) = when (filter) {
            is Filter.And -> "and" to filter.operands.map(::filterToJson)
            is Filter.Or -> "or" to filter.operands.map(::filterToJson)
            is Filter.Not -> "not" to listOf(filterToJson(filter.operand))
            is Filter.Comparison -> filter.operator.jsonName to listOf(JsonString(filter.field.path), filter.value)
            is Filter.Membership ->
                filter.operator.jsonName to listOf(JsonString(filter.field.path), JsonArray(filter.values))
            is Filter.Exists -> "exists" to listOf(JsonString(filter.field.path))
            is Filter.IsNull -> "is_null" to listOf(JsonString(filter.field.path))
        }
        return JsonObject(linkedMapOf("op" to JsonString(op), "args" to JsonArray(args)))
    }

    fun filterFromJson(value: Json): Filter = Decoder().decode(value, 0)

    /** Rust `from_value_rec`, including its error wording and `args[1].args[0]` locations. */
    private class Decoder {
        private val budget = FilterValidation.Budget()
        private val location = ArrayList<Any>()

        fun decode(value: Json, depth: Int): Filter {
            budget.enter(depth)
            if (value !is JsonObject) {
                throw malformed(
                    "a filter node must be an object such as {\"op\": \"eq\", \"args\": [\"field\", value]}",
                )
            }
            val op = when (val raw = value["op"]) {
                is JsonString -> raw.value
                null -> throw malformed("a filter node needs an `op` member")
                else -> throw malformed("`op` must be a string")
            }
            val args = value["args"] ?: JsonNull
            value.keys.firstOrNull { it != "op" && it != "args" }?.let { unknown ->
                throw malformed("unknown member `$unknown`; a filter node has only `op` and `args`")
            }
            location.add("args")
            val parsed = when (op) {
                "and", "or" -> {
                    if (args !is JsonArray) throw malformed("`$op` takes an array of filter nodes")
                    if (args.size == 0) throw malformed("`$op` needs at least one operand")
                    if (args.size > FilterValidation.MAX_NODES - budget.nodes) {
                        throw InvalidFilterException(
                            "filter exceeds the node count limit of ${FilterValidation.MAX_NODES}",
                        )
                    }
                    val operands = args.items.mapIndexed { index, nested ->
                        location.add(index)
                        decode(nested, depth + 1).also { location.removeAt(location.size - 1) }
                    }
                    if (op == "and") Filter.And(operands) else Filter.Or(operands)
                }
                "not" -> {
                    if (args !is JsonArray || args.size != 1) {
                        throw malformed("`not` takes an array with exactly one filter node")
                    }
                    location.add(0)
                    val inner = decode(args[0], depth + 1)
                    location.removeAt(location.size - 1)
                    Filter.Not(inner)
                }
                "eq", "ne", "lt", "lte", "gt", "gte" -> {
                    val (field, operand) = binaryArgs(args, op)
                    val operator = ComparisonOperator.values().first { it.jsonName == op }
                    FilterValidation.pathError(field)?.let { throw it }
                    FilterValidation.comparisonError(field, operator, operand)?.let { throw it }
                    Filter.Comparison(field, operator, operand)
                }
                "in", "nin" -> {
                    val (field, operand) = binaryArgs(args, op)
                    if (operand !is JsonArray) throw malformed("`$op` takes [\"field\", [value, ...]]")
                    FilterValidation.pathError(field)?.let { throw it }
                    budget.membership(field, operand.items)
                    Filter.Membership(
                        field,
                        if (op == "in") MembershipOperator.IN else MembershipOperator.NOT_IN,
                        operand.items,
                    )
                }
                "exists", "is_null" -> {
                    if (args !is JsonArray || args.size != 1) throw malformed("`$op` takes [\"field\"]")
                    val name = args[0] as? JsonString ?: throw malformed("the field must be a string")
                    val field = FieldPath.unchecked(name.value)
                    FilterValidation.pathError(field)?.let { throw it }
                    if (op == "exists") Filter.Exists(field) else Filter.IsNull(field)
                }
                else -> {
                    location.removeAt(location.size - 1)
                    throw malformed("unknown operator `$op`; expected one of: $OPERATORS")
                }
            }
            location.removeAt(location.size - 1)
            return parsed
        }

        private fun binaryArgs(args: Json, op: String): Pair<FieldPath, Json> {
            if (args !is JsonArray || args.size != 2) throw malformed("`$op` takes [\"field\", value]")
            val name = args[0] as? JsonString ?: throw malformed("the first argument must be the field name")
            return FieldPath.unchecked(name.value) to args[1]
        }

        private fun malformed(reason: String): InvalidFilterException {
            val rendered = buildString {
                for (segment in location) {
                    if (segment is Int) {
                        append('[').append(segment).append(']')
                    } else {
                        if (isNotEmpty()) append('.')
                        append(segment)
                    }
                }
            }
            return InvalidFilterException(if (rendered.isEmpty()) reason else "$reason (at `$rendered`)")
        }
    }
}

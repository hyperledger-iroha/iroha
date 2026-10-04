package org.hyperledger.iroha.sdk.json

import java.math.BigDecimal
import java.math.BigInteger
import java.nio.charset.StandardCharsets
import java.util.Collections

/**
 * Immutable JSON value with exact numbers.
 *
 * Numbers keep their exact lexical spelling, so amounts, `u64` counters and `u128` quantities
 * never pass through an IEEE double. Objects preserve member order and compare like JSON objects
 * (member order does not affect equality). Parsing is strict: duplicate object members, invalid
 * UTF-8, trailing input and nesting deeper than [MAX_DEPTH] are rejected.
 */
sealed class Json {
    /** Compact JSON text with the canonical Norito string escapes. */
    fun toJsonString(): String = StringBuilder().also { JsonWriter.write(this, it) }.toString()

    /** Compact UTF-8 JSON bytes. */
    fun toJsonBytes(): ByteArray = toJsonString().toByteArray(StandardCharsets.UTF_8)

    /** Same as [toJsonString]. */
    override fun toString(): String = toJsonString()

    companion object {
        /** Maximum nesting accepted by [parse]. */
        const val MAX_DEPTH: Int = 128

        /** The JSON `null` literal. */
        @JvmField
        val NULL: JsonNull = JsonNull

        /** Parse exactly one JSON document from text. */
        @JvmStatic
        fun parse(text: String): Json = JsonReader(text).readDocument()

        /** Parse exactly one JSON document from strict UTF-8 bytes. */
        @JvmStatic
        fun parse(bytes: ByteArray): Json = JsonReader(JsonReader.decodeUtf8(bytes)).readDocument()

        /** A JSON string. */
        @JvmStatic
        fun of(value: String): JsonString = JsonString(value)

        /** A JSON boolean. */
        @JvmStatic
        fun of(value: Boolean): JsonBoolean = JsonBoolean.of(value)

        /** A JSON integer. */
        @JvmStatic
        fun of(value: Long): JsonNumber = JsonNumber.of(value)

        /** A JSON integer. */
        @JvmStatic
        fun of(value: Int): JsonNumber = JsonNumber.of(value.toLong())

        /** A JSON integer with any magnitude. */
        @JvmStatic
        fun of(value: BigInteger): JsonNumber = JsonNumber.of(value)

        /** A JSON number with the exact plain decimal spelling of [value]. */
        @JvmStatic
        fun of(value: BigDecimal): JsonNumber = JsonNumber.of(value)

        /** A JSON array. */
        @JvmStatic
        fun array(items: List<Json>): JsonArray = JsonArray(items)

        /** A JSON array. */
        @JvmStatic
        fun array(vararg items: Json): JsonArray = JsonArray(items.toList())

        /** A JSON object with the given members in order. */
        @JvmStatic
        fun obj(members: Map<String, Json>): JsonObject = JsonObject(members)
    }
}

/** The JSON `null` literal. */
object JsonNull : Json()

/** A JSON boolean. */
class JsonBoolean private constructor(@JvmField val value: Boolean) : Json() {
    override fun equals(other: Any?): Boolean = other is JsonBoolean && other.value == value

    override fun hashCode(): Int = if (value) 1231 else 1237

    companion object {
        @JvmField
        val TRUE: JsonBoolean = JsonBoolean(true)

        @JvmField
        val FALSE: JsonBoolean = JsonBoolean(false)

        @JvmStatic
        fun of(value: Boolean): JsonBoolean = if (value) TRUE else FALSE
    }
}

/** A JSON string. */
class JsonString(value: String) : Json() {
    @JvmField
    val value: String = value

    override fun equals(other: Any?): Boolean = other is JsonString && other.value == value

    override fun hashCode(): Int = value.hashCode()
}

/**
 * A JSON number holding its exact lexical spelling (`-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?`).
 *
 * Equality compares spellings: `1` and `1.0` are different literals, as they are on the wire.
 */
class JsonNumber private constructor(@JvmField val text: String) : Json() {
    /** Whether the spelling has no fraction or exponent. */
    val isInteger: Boolean get() = text.indexOf('.') < 0 && text.indexOf('e') < 0 && text.indexOf('E') < 0

    /** The exact value. */
    fun toBigDecimal(): BigDecimal = BigDecimal(text)

    /** The exact integer value; fails when the number has a fractional part. */
    fun toBigInteger(): BigInteger = try {
        toBigDecimal().toBigIntegerExact()
    } catch (error: ArithmeticException) {
        throw IllegalStateException("JSON number $text is not an integer", error)
    }

    /** The exact value as a signed 64-bit integer; fails when it does not fit. */
    fun toLongExact(): Long {
        val integer = toBigInteger()
        check(integer.bitLength() < 64) { "JSON number $text does not fit a signed 64-bit integer" }
        return integer.toLong()
    }

    override fun equals(other: Any?): Boolean = other is JsonNumber && other.text == text

    override fun hashCode(): Int = text.hashCode()

    companion object {
        /** An integer literal. */
        @JvmStatic
        fun of(value: Long): JsonNumber = JsonNumber(value.toString())

        /** An integer literal of any magnitude. */
        @JvmStatic
        fun of(value: BigInteger): JsonNumber = JsonNumber(value.toString())

        /** A decimal literal using the plain (non-exponent) spelling of [value]. */
        @JvmStatic
        fun of(value: BigDecimal): JsonNumber = JsonNumber(value.toPlainString())

        /** Validate [text] as a JSON number spelling and keep it unchanged. */
        @JvmStatic
        fun parse(text: String): JsonNumber {
            require(JsonReader.isNumberSpelling(text)) { "`$text` is not a JSON number" }
            return JsonNumber(text)
        }

        internal fun trusted(text: String): JsonNumber = JsonNumber(text)
    }
}

/** An immutable JSON array. */
class JsonArray(items: List<Json>) : Json(), Iterable<Json> {
    /** Elements in order. */
    @JvmField
    val items: List<Json> = Collections.unmodifiableList(ArrayList(items))

    /** Number of elements. */
    val size: Int get() = items.size

    /** Element at [index]. */
    operator fun get(index: Int): Json = items[index]

    override fun iterator(): Iterator<Json> = items.iterator()

    override fun equals(other: Any?): Boolean = other is JsonArray && other.items == items

    override fun hashCode(): Int = items.hashCode()
}

/** An immutable JSON object; member order is kept for rendering and ignored by equality. */
class JsonObject(members: Map<String, Json>) : Json() {
    /** Members in order. */
    @JvmField
    val members: Map<String, Json> = Collections.unmodifiableMap(LinkedHashMap(members))

    /** Number of members. */
    val size: Int get() = members.size

    /** Member names in order. */
    val keys: Set<String> get() = members.keys

    /** The member called [name], or `null` when absent. */
    operator fun get(name: String): Json? = members[name]

    /** Whether a member called [name] exists (its value may be JSON `null`). */
    fun containsKey(name: String): Boolean = members.containsKey(name)

    /** The string member [name], or `null` when absent or JSON `null`; fails for other types. */
    fun stringOrNull(name: String): String? = when (val value = members[name]) {
        null, JsonNull -> null
        is JsonString -> value.value
        else -> throw IllegalStateException("`$name` must be a string")
    }

    /** The object member [name], or `null` when absent or JSON `null`; fails for other types. */
    fun objectOrNull(name: String): JsonObject? = when (val value = members[name]) {
        null, JsonNull -> null
        is JsonObject -> value
        else -> throw IllegalStateException("`$name` must be an object")
    }

    override fun equals(other: Any?): Boolean = other is JsonObject && other.members == members

    override fun hashCode(): Int = members.hashCode()

    companion object {
        /** An empty object. */
        @JvmField
        val EMPTY: JsonObject = JsonObject(emptyMap())

        /** A builder that keeps insertion order and rejects duplicate members. */
        @JvmStatic
        fun builder(): Builder = Builder()
    }

    /** Ordered builder for [JsonObject]. */
    class Builder internal constructor() {
        private val members = LinkedHashMap<String, Json>()

        /** Add a member; fails when [name] was already added. */
        fun put(name: String, value: Json): Builder {
            require(!members.containsKey(name)) { "duplicate JSON member `$name`" }
            members[name] = value
            return this
        }

        fun put(name: String, value: String): Builder = put(name, JsonString(value))

        fun put(name: String, value: Long): Builder = put(name, JsonNumber.of(value))

        fun put(name: String, value: Boolean): Builder = put(name, JsonBoolean.of(value))

        fun build(): JsonObject = JsonObject(members)
    }
}

/** Raised for malformed JSON text. */
class JsonSyntaxException internal constructor(
    /** What is wrong. */
    @JvmField val reason: String,
    /** Character offset where the problem was found. */
    @JvmField val offset: Int,
) : IllegalArgumentException("$reason (at offset $offset)")

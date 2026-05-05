package org.hyperledger.iroha.sdk.client

/** Minimal JSON encoder mirroring the structures parsed by `JsonParser` (sorted keys). */
object JsonEncoder {

    @JvmStatic
    fun encode(value: Any?): String = buildString { write(this, value) }

    @Suppress("UNCHECKED_CAST")
    private fun write(builder: StringBuilder, value: Any?) {
        when (value) {
            null -> builder.append("null")
            is String -> writeString(builder, value)
            is Number -> builder.append(value)
            is Boolean -> builder.append(if (value) "true" else "false")
            is Map<*, *> -> {
                builder.append('{')
                val keys = value.keys.map { key ->
                    require(key is String) { "JSON object keys must be strings" }
                    key
                }.sorted()
                keys.forEachIndexed { i, key ->
                    if (i > 0) builder.append(',')
                    writeString(builder, key)
                    builder.append(':')
                    write(builder, (value as Map<String, Any?>)[key])
                }
                builder.append('}')
            }
            is List<*> -> {
                builder.append('[')
                value.forEachIndexed { i, element ->
                    if (i > 0) builder.append(',')
                    write(builder, element)
                }
                builder.append(']')
            }
            else -> throw IllegalStateException("Unsupported JSON value: ${value::class.java}")
        }
    }

    private fun writeString(builder: StringBuilder, value: String) {
        builder.append('"')
        for (c in value) {
            when (c) {
                '"' -> builder.append("\\\"")
                '\\' -> builder.append("\\\\")
                '\b' -> builder.append("\\b")
                '\u000C' -> builder.append("\\f")
                '\n' -> builder.append("\\n")
                '\r' -> builder.append("\\r")
                '\t' -> builder.append("\\t")
                else -> {
                    if (c < '\u0020') {
                        builder.append("\\u%04x".format(c.code))
                    } else {
                        builder.append(c)
                    }
                }
            }
        }
        builder.append('"')
    }
}

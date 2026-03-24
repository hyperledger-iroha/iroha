package org.hyperledger.iroha.sdk.sorafs

/** Minimal JSON writer for the SoraFS helpers. */
internal object JsonWriter {

    @JvmStatic
    fun encode(value: Any?): String {
        val builder = StringBuilder()
        write(builder, value)
        return builder.toString()
    }

    @JvmStatic
    fun encodeBytes(value: Any?): ByteArray =
        encode(value).toByteArray(Charsets.UTF_8)

    @Suppress("UNCHECKED_CAST")
    private fun write(builder: StringBuilder, value: Any?) {
        when (value) {
            null -> builder.append("null")
            is String -> writeString(builder, value)
            is Number -> builder.append(value)
            is Boolean -> builder.append(if (value) "true" else "false")
            is Map<*, *> -> writeObject(builder, value as Map<String, Any>)
            is Iterable<*> -> writeArray(builder, value.iterator())
            else -> {
                if (value.javaClass.isArray) {
                    throw IllegalArgumentException("array values are not supported: ${value.javaClass}")
                }
                throw IllegalArgumentException("Unsupported JSON value: $value")
            }
        }
    }

    private fun writeObject(builder: StringBuilder, map: Map<String, Any>) {
        builder.append('{')
        val iterator = map.entries.iterator()
        while (iterator.hasNext()) {
            val entry = iterator.next()
            writeString(builder, entry.key)
            builder.append(':')
            write(builder, entry.value)
            if (iterator.hasNext()) {
                builder.append(',')
            }
        }
        builder.append('}')
    }

    private fun writeArray(builder: StringBuilder, iterator: Iterator<*>) {
        builder.append('[')
        var first = true
        while (iterator.hasNext()) {
            if (!first) builder.append(',') else first = false
            write(builder, iterator.next())
        }
        builder.append(']')
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

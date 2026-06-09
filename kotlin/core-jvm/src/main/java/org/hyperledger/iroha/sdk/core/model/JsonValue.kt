package org.hyperledger.iroha.sdk.core.model

@JvmInline
value class JsonValue(val rawJson: String) {
    companion object {
        fun string(value: String): JsonValue {
            val sb = StringBuilder(value.length + 2)
            sb.append('"')
            for (c in value) {
                when (c) {
                    '"' -> sb.append("\\\"")
                    '\\' -> sb.append("\\\\")
                    '\b' -> sb.append("\\b")
                    '\u000C' -> sb.append("\\f")
                    '\n' -> sb.append("\\n")
                    '\r' -> sb.append("\\r")
                    '\t' -> sb.append("\\t")
                    else -> {
                        if (c < ' ') {
                            sb.append("\\u00")
                            sb.append(HEX_DIGITS[(c.code shr 4) and 0xF])
                            sb.append(HEX_DIGITS[c.code and 0xF])
                        } else {
                            sb.append(c)
                        }
                    }
                }
            }
            sb.append('"')
            return JsonValue(sb.toString())
        }

        fun number(value: Long): JsonValue = JsonValue(value.toString())

        fun bool(value: Boolean): JsonValue = JsonValue(if (value) "true" else "false")

        fun raw(json: String): JsonValue = JsonValue(json)

        private val HEX_DIGITS = charArrayOf(
            '0', '1', '2', '3', '4', '5', '6', '7',
            '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
        )
    }
}

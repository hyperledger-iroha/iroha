// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.norito

class StructAdapter internal constructor(
    fields: List<StructField<*>>,
    private val factory: StructFactory?,
) : TypeAdapter<Any> {

    private val fields: List<StructField<*>> = fields.toList()

    fun interface StructFactory {
        fun create(fields: Map<String, Any?>): Any
    }

    override fun encode(encoder: NoritoEncoder, value: Any) {
        for (field in fields) {
            val fieldValue = extractField(value, field)
            NoritoAdapters.encodeAdapter(field.adapter, encoder, fieldValue)
        }
    }

    override fun decode(decoder: NoritoDecoder): Any {
        val values = LinkedHashMap<String, Any?>()
        for (field in fields) {
            values[field.name] = NoritoAdapters.decodeAdapter(field.adapter, decoder)
        }
        return factory?.create(values) ?: values
    }

    companion object {
        private fun extractField(value: Any, field: StructField<*>): Any? {
            if (field.accessor != null) return field.accessor.invoke(value)
            if (value is Map<*, *>) {
                require(value.containsKey(field.name)) { "Missing struct field ${field.name}" }
                return value[field.name]
            }
            throw IllegalArgumentException("Unable to extract field ${field.name}: no accessor and not a Map")
        }
    }
}

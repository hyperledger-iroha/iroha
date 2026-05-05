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
        if ((encoder.flags and NoritoHeader.PACKED_STRUCT) != 0
            && (encoder.flags and NoritoHeader.FIELD_BITSET) != 0
        ) {
            encodePacked(encoder, value)
            return
        }
        for (field in fields) {
            val fieldValue = extractField(value, field)
            NoritoAdapters.encodeAdapter(field.adapter, encoder, fieldValue)
        }
    }

    private fun encodePacked(encoder: NoritoEncoder, value: Any) {
        val payloads = ArrayList<ByteArray>(fields.size)
        var bitset = 0
        for (i in fields.indices) {
            val field = fields[i]
            val fieldValue = extractField(value, field)
            val child = encoder.childEncoder()
            NoritoAdapters.encodeAdapter(field.adapter, child, fieldValue)
            val bytes = child.toByteArray()
            payloads.add(bytes)
            if (needsExplicitSize(field.adapter)) {
                bitset = bitset or (1 shl i)
            }
        }
        val bitsetBytes = (fields.size + 7) / 8
        for (i in 0 until bitsetBytes) {
            encoder.writeByte((bitset shr (i * 8)) and 0xFF)
        }
        for (i in fields.indices) {
            if ((bitset and (1 shl i)) != 0) {
                encoder.append(Varint.encode(payloads[i].size.toLong()))
            }
        }
        for (bytes in payloads) {
            encoder.append(bytes)
        }
    }

    override fun decode(decoder: NoritoDecoder): Any {
        val values = LinkedHashMap<String, Any?>()
        if ((decoder.flags and NoritoHeader.PACKED_STRUCT) != 0
            && (decoder.flags and NoritoHeader.FIELD_BITSET) != 0
        ) {
            decodePacked(decoder, values)
        } else {
            for (field in fields) {
                val value = NoritoAdapters.decodeAdapter(field.adapter, decoder)
                values[field.name] = value
            }
        }
        return factory?.create(values) ?: values
    }

    private fun decodePacked(decoder: NoritoDecoder, values: MutableMap<String, Any?>) {
        val bitsetBytes = (fields.size + 7) / 8
        val bitsetData = decoder.readBytes(bitsetBytes)
        var bitset = 0
        for (i in 0 until bitsetBytes) {
            bitset = bitset or ((bitsetData[i].toInt() and 0xFF) shl (i * 8))
        }
        val encodedSizes = ArrayList<Int?>(fields.size)
        for (i in fields.indices) {
            if ((bitset and (1 shl i)) != 0) {
                val size = decoder.readVarint()
                require(size <= Int.MAX_VALUE) { "Packed field too large" }
                encodedSizes.add(size.toInt())
            } else {
                encodedSizes.add(null)
            }
        }
        for (i in fields.indices) {
            val field = fields[i]
            val adapter = field.adapter
            val size = encodedSizes[i]
            val value: Any?
            if (size != null) {
                val chunk = decoder.readBytes(size)
                val child = NoritoDecoder(chunk, decoder.flags, decoder.flagsHint)
                value = NoritoAdapters.decodeAdapter(adapter, child)
                require(child.remaining() == 0) { "Packed field did not consume all bytes" }
            } else {
                value = NoritoAdapters.decodeAdapter(adapter, decoder)
            }
            values[field.name] = value
        }
    }

    companion object {
        private fun needsExplicitSize(adapter: TypeAdapter<*>): Boolean {
            if (adapter.fixedSize() >= 0) return false
            return !adapter.isSelfDelimiting()
        }

        private fun extractField(value: Any, field: StructField<*>): Any? {
            if (field.accessor != null) return field.accessor.invoke(value)
            if (value is Map<*, *>) return value[field.name]
            throw IllegalArgumentException("Unable to extract field ${field.name}: no accessor and not a Map")
        }
    }
}

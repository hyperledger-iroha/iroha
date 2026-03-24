// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.norito

import java.nio.charset.StandardCharsets
import java.util.AbstractMap
import java.util.Optional

/** Factory helpers for Norito adapters. */
object NoritoAdapters {

    @Suppress("UNCHECKED_CAST")
    internal fun encodeAdapter(adapter: TypeAdapter<*>, encoder: NoritoEncoder, value: Any?) {
        (adapter as TypeAdapter<Any?>).encode(encoder, value)
    }

    internal fun decodeAdapter(adapter: TypeAdapter<*>, decoder: NoritoDecoder): Any? {
        return adapter.decode(decoder)
    }

    @JvmStatic
    fun uint(bits: Int): TypeAdapter<Long> = UIntAdapter(bits)

    @JvmStatic
    fun sint(bits: Int): TypeAdapter<Long> = IntAdapter(bits)

    @JvmStatic
    fun boolAdapter(): TypeAdapter<Boolean> = BoolAdapter

    @JvmStatic
    fun bytesAdapter(): TypeAdapter<ByteArray> = BytesAdapter

    @JvmStatic
    fun byteVecAdapter(): TypeAdapter<ByteArray> = ByteVecAdapter

    @JvmStatic
    fun rawByteVecAdapter(): TypeAdapter<ByteArray> = RawByteVecAdapter

    @JvmStatic
    fun fixedBytes(length: Int): TypeAdapter<ByteArray> = FixedBytesAdapter(length)

    @JvmStatic
    fun stringAdapter(): TypeAdapter<String> = StringAdapter

    @JvmStatic
    fun <T> option(inner: TypeAdapter<T>): TypeAdapter<Optional<T>> = OptionAdapter(inner)

    @JvmStatic
    fun <T, E> result(ok: TypeAdapter<T>, err: TypeAdapter<E>): TypeAdapter<Result<T, E>> =
        ResultAdapter(ok, err)

    @JvmStatic
    fun <T> sequence(element: TypeAdapter<T>): TypeAdapter<List<T>> = SequenceAdapter(element)

    @JvmStatic
    fun <K, V> map(key: TypeAdapter<K>, value: TypeAdapter<V>): TypeAdapter<Map<K, V>> =
        MapAdapter(key, value)

    @JvmStatic
    fun tuple(elements: List<TypeAdapter<*>>): TypeAdapter<List<Any>> = TupleAdapter(elements)

    @JvmStatic
    fun <T> field(name: String, adapter: TypeAdapter<T>): StructField<T> =
        StructField(name, adapter)

    @JvmStatic
    fun struct(
        fields: List<StructField<*>>,
        factory: StructAdapter.StructFactory,
    ): StructAdapter = StructAdapter(fields, factory)

    @JvmStatic
    fun struct(fields: List<StructField<*>>): StructAdapter = StructAdapter(fields, null)

    private class UIntAdapter(private val bits: Int) : TypeAdapter<Long> {
        init {
            require(bits == 8 || bits == 16 || bits == 32 || bits == 64) {
                "Unsupported unsigned size"
            }
        }

        override fun encode(encoder: NoritoEncoder, value: Long) {
            encoder.writeUInt(value, bits)
        }

        override fun decode(decoder: NoritoDecoder): Long = decoder.readUInt(bits)

        override fun fixedSize(): Int = bits / 8
    }

    private class IntAdapter(private val bits: Int) : TypeAdapter<Long> {
        init {
            require(bits == 8 || bits == 16 || bits == 32 || bits == 64) {
                "Unsupported integer size"
            }
        }

        override fun encode(encoder: NoritoEncoder, value: Long) {
            encoder.writeInt(value, bits)
        }

        override fun decode(decoder: NoritoDecoder): Long = decoder.readInt(bits)

        override fun fixedSize(): Int = bits / 8
    }

    private object BoolAdapter : TypeAdapter<Boolean> {
        override fun encode(encoder: NoritoEncoder, value: Boolean) {
            encoder.writeByte(if (value) 1 else 0)
        }

        override fun decode(decoder: NoritoDecoder): Boolean = decoder.readByte() != 0

        override fun fixedSize(): Int = 1
    }

    private object BytesAdapter : TypeAdapter<ByteArray> {
        override fun encode(encoder: NoritoEncoder, value: ByteArray) {
            val compact = (encoder.flags and NoritoHeader.COMPACT_LEN) != 0
            encoder.writeLength(value.size.toLong(), compact)
            encoder.writeBytes(value)
        }

        override fun decode(decoder: NoritoDecoder): ByteArray {
            val length = decoder.readLength(decoder.compactLenActive())
            require(length <= Int.MAX_VALUE) { "Byte array too large" }
            return decoder.readBytes(length.toInt())
        }

        override fun isSelfDelimiting(): Boolean = true
    }

    private object ByteVecAdapter : TypeAdapter<ByteArray> {
        override fun encode(encoder: NoritoEncoder, value: ByteArray) {
            encoder.writeLength(value.size.toLong(), false)
            if ((encoder.flags and NoritoHeader.PACKED_SEQ) != 0) {
                encodePacked(encoder, value)
            } else {
                val compactLen = (encoder.flags and NoritoHeader.COMPACT_LEN) != 0
                for (b in value) {
                    encoder.writeLength(1L, compactLen)
                    encoder.writeByte(b.toInt())
                }
            }
        }

        override fun decode(decoder: NoritoDecoder): ByteArray {
            val length = decoder.readLength(false)
            require(length <= Int.MAX_VALUE) { "Byte vector too large" }
            val count = length.toInt()
            if ((decoder.flags and NoritoHeader.PACKED_SEQ) != 0) {
                return decodePacked(decoder, count)
            }
            val out = ByteArray(count)
            val compactLen = decoder.compactLenActive()
            for (i in 0 until count) {
                val elemLen = decoder.readLength(compactLen)
                require(elemLen <= Int.MAX_VALUE) { "Byte vector element too large" }
                val elemSize = elemLen.toInt()
                if (elemSize == 1) {
                    out[i] = decoder.readByte().toByte()
                    continue
                }
                val payload = decoder.readBytes(elemSize)
                val child = NoritoDecoder(payload, decoder.flags, decoder.flagsHint)
                val value = child.readByte()
                require(child.remaining() == 0) {
                    "Byte vector element did not consume all bytes"
                }
                out[i] = value.toByte()
            }
            return out
        }

        override fun isSelfDelimiting(): Boolean = true

        private fun encodePacked(encoder: NoritoEncoder, value: ByteArray) {
            var offset = 0L
            encoder.writeUInt(offset, 64)
            for (i in value.indices) {
                offset += 1
                encoder.writeUInt(offset, 64)
            }
            encoder.writeBytes(value)
        }

        private fun decodePacked(decoder: NoritoDecoder, count: Int): ByteArray {
            if (count == 0) {
                val tailLen = decoder.remaining()
                if (tailLen == 0) return ByteArray(0)
                if (tailLen >= java.lang.Long.BYTES) {
                    val prefix = decoder.readBytes(java.lang.Long.BYTES)
                    for (b in prefix) {
                        require(b.toInt() == 0) {
                            "Packed byte vector declared zero length but carried trailing data"
                        }
                    }
                    return ByteArray(0)
                }
                throw IllegalArgumentException(
                    "Packed byte vector declared zero length but carried trailing data"
                )
            }

            val sizes = ArrayList<Int>(count)
            var previous = decoder.readUInt(64)
            require(previous == 0L) { "Packed offsets must start at 0" }
            for (i in 0 until count) {
                val current = decoder.readUInt(64)
                val delta = current - previous
                require(delta in 0..Int.MAX_VALUE) { "Invalid packed offsets" }
                sizes.add(delta.toInt())
                previous = current
            }

            val out = ByteArray(count)
            for (i in 0 until count) {
                val payload = decoder.readBytes(sizes[i])
                val child = NoritoDecoder(payload, decoder.flags, decoder.flagsHint)
                val value = child.readByte()
                require(child.remaining() == 0) {
                    "Packed byte element did not consume all bytes"
                }
                out[i] = value.toByte()
            }
            return out
        }
    }

    /**
     * Adapter that encodes `byte[]` as `length(u64) + raw bytes`, matching the Rust
     * `Vec<u8>` fast-path serialization in Norito.
     *
     * Use this for opaque byte payloads (wire instruction payloads, IVM bytecode) where
     * per-element framing is not expected by the server.
     *
     * @see byteVecAdapter for the sequence-of-elements encoding used for signatures
     */
    private object RawByteVecAdapter : TypeAdapter<ByteArray> {
        override fun encode(encoder: NoritoEncoder, value: ByteArray) {
            val compactLen = (encoder.flags and NoritoHeader.COMPACT_LEN) != 0
            encoder.writeLength(value.size.toLong(), compactLen)
            encoder.writeBytes(value)
        }

        override fun decode(decoder: NoritoDecoder): ByteArray {
            val compactLen = decoder.compactLenActive()
            val length = decoder.readLength(compactLen)
            require(length <= Int.MAX_VALUE) { "Raw byte vector too large" }
            return decoder.readBytes(length.toInt())
        }

        override fun isSelfDelimiting(): Boolean = true
    }

    private class FixedBytesAdapter(private val length: Int) : TypeAdapter<ByteArray> {
        init {
            require(length > 0) { "length must be positive" }
        }

        override fun encode(encoder: NoritoEncoder, value: ByteArray) {
            require(value.size == length) {
                "expected $length bytes, found ${value.size}"
            }
            encoder.writeBytes(value)
        }

        override fun decode(decoder: NoritoDecoder): ByteArray = decoder.readBytes(length)

        override fun fixedSize(): Int = length
    }

    private object StringAdapter : TypeAdapter<String> {
        override fun encode(encoder: NoritoEncoder, value: String) {
            val data = value.toByteArray(StandardCharsets.UTF_8)
            BytesAdapter.encode(encoder, data)
        }

        override fun decode(decoder: NoritoDecoder): String {
            val data = BytesAdapter.decode(decoder)
            return String(data, StandardCharsets.UTF_8)
        }

        override fun isSelfDelimiting(): Boolean = true
    }

    private class OptionAdapter<T>(private val inner: TypeAdapter<T>) : TypeAdapter<Optional<T>> {
        override fun encode(encoder: NoritoEncoder, value: Optional<T>) {
            if (value.isPresent) {
                encoder.writeByte(1)
                val child = encoder.childEncoder()
                inner.encode(child, value.get())
                val payload = child.toByteArray()
                val compact = (encoder.flags and NoritoHeader.COMPACT_LEN) != 0
                encoder.writeLength(payload.size.toLong(), compact)
                encoder.writeBytes(payload)
            } else {
                encoder.writeByte(0)
            }
        }

        @Suppress("UNCHECKED_CAST")
        override fun decode(decoder: NoritoDecoder): Optional<T> {
            val tag = decoder.readByte()
            if (tag == 0) return Optional.empty<Any>() as Optional<T>
            require(tag == 1) { "Invalid Option tag: $tag" }
            val length = decoder.readLength(decoder.compactLenActive())
            require(length <= Int.MAX_VALUE) { "Option payload too large" }
            val payload = decoder.readBytes(length.toInt())
            val child = NoritoDecoder(payload, decoder.flags, decoder.flagsHint)
            val value = inner.decode(child)
            require(child.remaining() == 0) { "Trailing bytes after Option payload" }
            return Optional.of(value as Any) as Optional<T>
        }

        override fun isSelfDelimiting(): Boolean = true
    }

    private class ResultAdapter<T, E>(
        private val ok: TypeAdapter<T>,
        private val err: TypeAdapter<E>,
    ) : TypeAdapter<Result<T, E>> {

        override fun encode(encoder: NoritoEncoder, value: Result<T, E>) {
            when (value) {
                is Result.Ok -> {
                    encoder.writeByte(0)
                    val child = encoder.childEncoder()
                    ok.encode(child, value.value)
                    val payload = child.toByteArray()
                    val compact = (encoder.flags and NoritoHeader.COMPACT_LEN) != 0
                    encoder.writeLength(payload.size.toLong(), compact)
                    encoder.writeBytes(payload)
                }
                is Result.Err -> {
                    encoder.writeByte(1)
                    val child = encoder.childEncoder()
                    err.encode(child, value.error)
                    val payload = child.toByteArray()
                    val compact = (encoder.flags and NoritoHeader.COMPACT_LEN) != 0
                    encoder.writeLength(payload.size.toLong(), compact)
                    encoder.writeBytes(payload)
                }
            }
        }

        override fun decode(decoder: NoritoDecoder): Result<T, E> {
            val tag = decoder.readByte()
            require(tag == 0 || tag == 1) { "Invalid Result tag: $tag" }
            val length = decoder.readLength(decoder.compactLenActive())
            require(length <= Int.MAX_VALUE) { "Result payload too large" }
            val payload = decoder.readBytes(length.toInt())
            val child = NoritoDecoder(payload, decoder.flags, decoder.flagsHint)
            val value = if (tag == 0) ok.decode(child) else err.decode(child)
            require(child.remaining() == 0) { "Trailing bytes after Result payload" }
            return if (tag == 0) {
                @Suppress("UNCHECKED_CAST")
                Result.Ok(value as T)
            } else {
                @Suppress("UNCHECKED_CAST")
                Result.Err(value as E)
            }
        }

        override fun isSelfDelimiting(): Boolean = true
    }

    private class SequenceAdapter<T>(private val element: TypeAdapter<T>) : TypeAdapter<List<T>> {
        override fun encode(encoder: NoritoEncoder, values: List<T>) {
            encoder.writeLength(values.size.toLong(), false)
            if ((encoder.flags and NoritoHeader.PACKED_SEQ) != 0) {
                encodePacked(encoder, values)
            } else {
                val compact = (encoder.flags and NoritoHeader.COMPACT_LEN) != 0
                for (value in values) {
                    val child = encoder.childEncoder()
                    element.encode(child, value)
                    val payload = child.toByteArray()
                    encoder.writeLength(payload.size.toLong(), compact)
                    encoder.writeBytes(payload)
                }
            }
        }

        private fun encodePacked(encoder: NoritoEncoder, values: List<T>) {
            val encodedElements = ArrayList<ByteArray>(values.size)
            for (value in values) {
                val child = encoder.childEncoder()
                element.encode(child, value)
                encodedElements.add(child.toByteArray())
            }
            var offset = 0L
            encoder.writeUInt(offset, 64)
            for (chunk in encodedElements) {
                offset += chunk.size
                encoder.writeUInt(offset, 64)
            }
            for (chunk in encodedElements) {
                encoder.append(chunk)
            }
        }

        override fun decode(decoder: NoritoDecoder): List<T> {
            val length = decoder.readLength(false)
            require(length <= Int.MAX_VALUE) { "Sequence too large" }
            val count = length.toInt()
            if ((decoder.flags and NoritoHeader.PACKED_SEQ) != 0) {
                return decodePacked(decoder, count)
            }
            val values = ArrayList<T>(count)
            val compact = decoder.compactLenActive()
            for (i in 0 until count) {
                val elemLen = decoder.readLength(compact)
                require(elemLen <= Int.MAX_VALUE) { "Sequence element too large" }
                val payload = decoder.readBytes(elemLen.toInt())
                val child = NoritoDecoder(payload, decoder.flags, decoder.flagsHint)
                val value = element.decode(child)
                require(child.remaining() == 0) {
                    "Sequence element did not consume all bytes"
                }
                values.add(value)
            }
            return values
        }

        override fun isSelfDelimiting(): Boolean = true

        private fun decodePacked(decoder: NoritoDecoder, count: Int): List<T> {
            if (count == 0) {
                val tailLen = decoder.remaining()
                if (tailLen == 0) return emptyList()
                if (tailLen >= java.lang.Long.BYTES) {
                    val prefix = decoder.readBytes(java.lang.Long.BYTES)
                    for (b in prefix) {
                        require(b.toInt() == 0) {
                            "Packed sequence declared zero length but carried trailing data"
                        }
                    }
                    return emptyList()
                }
                throw IllegalArgumentException(
                    "Packed sequence declared zero length but carried trailing data"
                )
            }

            val sizes = ArrayList<Int>(count)
            var previous = decoder.readUInt(64)
            require(previous == 0L) { "Packed offsets must start at 0" }
            for (i in 0 until count) {
                val current = decoder.readUInt(64)
                val delta = current - previous
                require(delta in 0..Int.MAX_VALUE) { "Invalid packed offsets" }
                sizes.add(delta.toInt())
                previous = current
            }

            val values = ArrayList<T>(count)
            for (size in sizes) {
                val chunk = decoder.readBytes(size)
                val child = NoritoDecoder(chunk, decoder.flags, decoder.flagsHint)
                val value = element.decode(child)
                require(child.remaining() == 0) {
                    "Packed element did not consume all bytes"
                }
                values.add(value)
            }
            return values
        }
    }

    private class MapAdapter<K, V>(
        private val key: TypeAdapter<K>,
        private val value: TypeAdapter<V>,
    ) : TypeAdapter<Map<K, V>> {

        override fun encode(encoder: NoritoEncoder, map: Map<K, V>) {
            val entries = sortedEntries(map)
            encoder.writeLength(entries.size.toLong(), false)
            if ((encoder.flags and NoritoHeader.PACKED_SEQ) != 0) {
                encodePacked(encoder, entries)
            } else {
                encodeCompat(encoder, entries)
            }
        }

        override fun decode(decoder: NoritoDecoder): Map<K, V> {
            val length = decoder.readLength(false)
            require(length <= Int.MAX_VALUE) { "Map too large" }
            val count = length.toInt()
            return if ((decoder.flags and NoritoHeader.PACKED_SEQ) != 0) {
                decodePacked(decoder, count)
            } else {
                decodeCompat(decoder, count)
            }
        }

        override fun isSelfDelimiting(): Boolean = true

        private fun decodeCompat(decoder: NoritoDecoder, count: Int): Map<K, V> {
            val map = LinkedHashMap<K, V>(count)
            val compactLen = decoder.compactLenActive()
            for (i in 0 until count) {
                val keyLen = decoder.readLength(compactLen)
                require(keyLen <= Int.MAX_VALUE) { "Map key too large" }
                val decodedKey = decodeSizedField(key, decoder, keyLen.toInt())
                require(!map.containsKey(decodedKey)) { "Duplicate map key" }
                val valueLen = decoder.readLength(compactLen)
                require(valueLen <= Int.MAX_VALUE) { "Map value too large" }
                val decodedValue = decodeSizedField(value, decoder, valueLen.toInt())
                map[decodedKey] = decodedValue
            }
            return map
        }

        private fun decodePacked(decoder: NoritoDecoder, count: Int): Map<K, V> {
            val keySizes = readFixedOffsets(decoder, count, "Map key")
            val valueSizes = readFixedOffsets(decoder, count, "Map value")

            val keys = ArrayList<K>(count)
            for (size in keySizes) {
                keys.add(decodeSizedField(key, decoder, size))
            }

            val map = LinkedHashMap<K, V>(count)
            for (i in 0 until count) {
                val decodedValue = decodeSizedField(value, decoder, valueSizes[i])
                val decodedKey = keys[i]
                require(!map.containsKey(decodedKey)) { "Duplicate map key" }
                map[decodedKey] = decodedValue
            }
            return map
        }

        private fun sortedEntries(map: Map<K, V>): List<Map.Entry<K, V>> {
            val entries = ArrayList<Map.Entry<K, V>>(map.size)
            entries.addAll(map.entries)
            for (entry in entries) {
                requireNotNull(entry.key) { "Map keys must be non-null" }
            }
            if (map is java.util.SortedMap<*, *>) return entries
            entries.sortWith { left, right -> compareKeys(left.key, right.key) }
            return entries
        }

        @Suppress("UNCHECKED_CAST")
        private fun compareKeys(left: K, right: K): Int {
            if (left is Comparable<*>) {
                try {
                    return (left as Comparable<Any>).compareTo(right as Any)
                } catch (ex: ClassCastException) {
                    throw IllegalArgumentException(
                        "Map keys must be Comparable for deterministic encoding", ex
                    )
                }
            }
            throw IllegalArgumentException(
                "Map keys must be Comparable for deterministic encoding"
            )
        }

        private fun encodeCompat(encoder: NoritoEncoder, entries: List<Map.Entry<K, V>>) {
            val compactLen = (encoder.flags and NoritoHeader.COMPACT_LEN) != 0
            for (entry in entries) {
                val keyBytes = encodeField(encoder, key, entry.key)
                encoder.writeLength(keyBytes.size.toLong(), compactLen)
                encoder.append(keyBytes)

                val valueBytes = encodeField(encoder, value, entry.value)
                encoder.writeLength(valueBytes.size.toLong(), compactLen)
                encoder.append(valueBytes)
            }
        }

        private fun encodePacked(encoder: NoritoEncoder, entries: List<Map.Entry<K, V>>) {
            if (entries.isEmpty()) {
                writeFixedOffsets(encoder, emptyList())
                writeFixedOffsets(encoder, emptyList())
                return
            }
            val keySizes = ArrayList<Int>(entries.size)
            val valueSizes = ArrayList<Int>(entries.size)
            val keyPayloads = ArrayList<ByteArray>(entries.size)
            val valuePayloads = ArrayList<ByteArray>(entries.size)
            for (entry in entries) {
                val keyBytes = encodeField(encoder, key, entry.key)
                val valueBytes = encodeField(encoder, value, entry.value)
                keySizes.add(keyBytes.size)
                valueSizes.add(valueBytes.size)
                keyPayloads.add(keyBytes)
                valuePayloads.add(valueBytes)
            }
            writeFixedOffsets(encoder, keySizes)
            writeFixedOffsets(encoder, valueSizes)
            for (keyBytes in keyPayloads) {
                encoder.append(keyBytes)
            }
            for (valueBytes in valuePayloads) {
                encoder.append(valueBytes)
            }
        }

        private fun encodeField(
            encoder: NoritoEncoder,
            adapter: TypeAdapter<*>,
            value: Any?,
        ): ByteArray {
            val child = encoder.childEncoder()
            encodeAdapter(adapter, child, value)
            return child.toByteArray()
        }

        private fun <T> decodeSizedField(
            adapter: TypeAdapter<T>,
            decoder: NoritoDecoder,
            size: Int,
        ): T {
            val chunk = decoder.readBytes(size)
            val child = NoritoDecoder(chunk, decoder.flags, decoder.flagsHint)
            val value = adapter.decode(child)
            require(child.remaining() == 0) { "Map entry did not consume all bytes" }
            return value
        }

        private fun readFixedOffsets(
            decoder: NoritoDecoder,
            count: Int,
            label: String,
        ): List<Int> {
            val sizes = ArrayList<Int>(count)
            var previous = decoder.readUInt(64)
            require(previous == 0L) { "$label offsets must start at 0" }
            for (i in 0 until count) {
                val current = decoder.readUInt(64)
                val delta = current - previous
                require(delta in 0..Int.MAX_VALUE) { "Invalid $label offsets" }
                sizes.add(delta.toInt())
                previous = current
            }
            return sizes
        }

        private fun writeFixedOffsets(encoder: NoritoEncoder, sizes: List<Int>) {
            var offset = 0L
            encoder.writeUInt(offset, 64)
            for (size in sizes) {
                offset += size
                encoder.writeUInt(offset, 64)
            }
        }
    }

    private class EntryAdapter<K, V>(
        private val key: TypeAdapter<K>,
        private val value: TypeAdapter<V>,
    ) : TypeAdapter<Map.Entry<K, V>> {

        override fun encode(encoder: NoritoEncoder, entry: Map.Entry<K, V>) {
            key.encode(encoder, entry.key)
            value.encode(encoder, entry.value)
        }

        override fun decode(decoder: NoritoDecoder): Map.Entry<K, V> {
            val decodedKey = key.decode(decoder)
            val decodedValue = value.decode(decoder)
            return AbstractMap.SimpleImmutableEntry(decodedKey, decodedValue)
        }

        override fun fixedSize(): Int {
            val keySize = key.fixedSize()
            val valueSize = value.fixedSize()
            return if (keySize >= 0 && valueSize >= 0) keySize + valueSize else -1
        }

        override fun isSelfDelimiting(): Boolean =
            key.isSelfDelimiting() && value.isSelfDelimiting()
    }

    private class TupleAdapter(elements: List<TypeAdapter<*>>) : TypeAdapter<List<Any>> {
        private val elements: List<TypeAdapter<*>> = elements.toList()

        override fun encode(encoder: NoritoEncoder, value: List<Any>) {
            require(value.size == elements.size) { "Tuple size mismatch" }
            for (i in elements.indices) {
                encodeAdapter(elements[i], encoder, value[i])
            }
        }

        override fun decode(decoder: NoritoDecoder): List<Any> {
            val values = ArrayList<Any>(elements.size)
            for (element in elements) {
                values.add(decodeAdapter(element, decoder)!!)
            }
            return values
        }

        override fun isSelfDelimiting(): Boolean = elements.all { it.isSelfDelimiting() }
    }
}

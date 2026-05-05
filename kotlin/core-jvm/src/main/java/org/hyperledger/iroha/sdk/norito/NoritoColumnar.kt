// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.norito

import java.io.ByteArrayOutputStream
import java.nio.charset.StandardCharsets

private const val DESC_U64_DELTA_STR_BOOL = 0x53
private const val DESC_U64_DICT_STR_BOOL = 0x93
private const val DESC_U64_OPTIONAL_BYTES = 0x71
private const val DESC_U64_ENUM_BOOL = 0x61
private const val DESC_U64_DELTA_ENUM_BOOL = 0x63
private const val DESC_U64_ENUM_BOOL_CODEDELTA = 0x65
private const val DESC_U64_DELTA_ENUM_BOOL_CODEDELTA = 0x67
private const val DESC_U64_ENUM_BOOL_DICT = 0xE1
private const val DESC_U64_DELTA_ENUM_BOOL_DICT = 0xE3
private const val DESC_U64_ENUM_BOOL_DICT_CODEDELTA = 0xE5
private const val DESC_U64_DELTA_ENUM_BOOL_DICT_CODEDELTA = 0xE7

private const val TAG_ENUM_NAME = 0
private const val TAG_ENUM_CODE = 1

private const val ADAPTIVE_TAG_AOS = 0x00
private const val ADAPTIVE_TAG_NCB = 0x01

private const val AOS_NCB_SMALL_N = 64
private const val COMBO_NO_DELTA_SMALL_N_IF_EMPTY = 2
private const val COMBO_ID_DELTA_MIN_ROWS = 2
private const val COMBO_ENABLE_ID_DELTA = true
private const val COMBO_ENABLE_NAME_DICT = true
private const val COMBO_DICT_RATIO_MAX = 0.40
private const val COMBO_DICT_AVG_LEN_MIN = 8.0
private const val U32_MAX = 0xFFFF_FFFFL

object NoritoColumnar {

    const val DESC_U64_STR_BOOL = 0x13
    const val DESC_U64_BYTES = 0x21

    data class StrBoolRow(
        @JvmField val id: Long,
        @JvmField val name: String,
        @JvmField val flag: Boolean,
    )

    sealed interface EnumValue

    data class EnumName(@JvmField val name: String) : EnumValue

    data class EnumCode(@JvmField val code: Long) : EnumValue {
        init {
            require(code in 0..U32_MAX) { "code must fit into u32" }
        }
    }

    data class EnumBoolRow(
        @JvmField val id: Long,
        @JvmField val value: EnumValue,
        @JvmField val flag: Boolean,
    )

    class BytesRow(
        @JvmField val id: Long,
        data: ByteArray,
    ) {
        private val _data: ByteArray = data.copyOf()

        val data: ByteArray get() = _data.copyOf()

        internal fun dataRaw(): ByteArray = _data

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is BytesRow) return false
            return id == other.id && _data.contentEquals(other._data)
        }

        override fun hashCode(): Int = 31 * id.hashCode() + _data.contentHashCode()
    }

    class BytesOptionalRow(
        @JvmField val id: Long,
        data: ByteArray?,
    ) {
        private val _data: ByteArray? = data?.copyOf()

        val isPresent: Boolean get() = _data != null

        val data: ByteArray? get() = _data?.copyOf()

        internal fun dataRaw(): ByteArray? = _data

        override fun equals(other: Any?): Boolean {
            if (this === other) return true
            if (other !is BytesOptionalRow) return false
            if (id != other.id) return false
            return when {
                _data == null && other._data == null -> true
                _data != null && other._data != null -> _data.contentEquals(other._data)
                else -> false
            }
        }

        override fun hashCode(): Int = 31 * id.hashCode() + (_data?.contentHashCode() ?: 0)
    }

    @JvmStatic
    fun encodeNcbU64StrBool(rows: List<StrBoolRow>): ByteArray {
        val dict = buildDict(rows)
        if (dict.useDict) return encodeNcbDict(rows, dict)
        if (shouldUseIdDelta(rows)) return encodeNcbDelta(rows)
        return encodeNcbOffsets(rows)
    }

    @JvmStatic
    fun encodeRowsU64StrBoolAdaptive(rows: List<StrBoolRow>): ByteArray {
        if (rows.size <= AOS_NCB_SMALL_N) {
            val aos = NoritoAoS.encodeU64StrBool(rows)
            val ncb = encodeNcbU64StrBool(rows)
            if (ncb.size < aos.size) return concat(ADAPTIVE_TAG_NCB, ncb)
            return concat(ADAPTIVE_TAG_AOS, aos)
        }
        return concat(ADAPTIVE_TAG_AOS, NoritoAoS.encodeU64StrBool(rows))
    }

    @JvmStatic
    fun decodeRowsU64StrBoolAdaptive(payload: ByteArray): List<StrBoolRow> {
        require(payload.isNotEmpty()) { "Adaptive payload is empty" }
        val tag = payload[0].toInt() and 0xFF
        val body = payload.copyOfRange(1, payload.size)
        return when (tag) {
            ADAPTIVE_TAG_AOS -> NoritoAoS.decodeU64StrBool(body)
            ADAPTIVE_TAG_NCB -> decodeNcbU64StrBool(body)
            else -> throw IllegalArgumentException("Unknown adaptive tag: $tag")
        }
    }

    @JvmStatic
    fun decodeNcbU64StrBool(data: ByteArray): List<StrBoolRow> {
        require(data.size >= 5) { "NCB payload too short" }
        var offset = 0
        val n = readU32(data, offset)
        offset += 4
        val desc = data[offset++].toInt() and 0xFF
        require(desc == DESC_U64_STR_BOOL || desc == DESC_U64_DELTA_STR_BOOL || desc == DESC_U64_DICT_STR_BOOL) {
            "Unsupported descriptor 0x${"%02x".format(desc)}"
        }
        val ids = ArrayList<Long>(n)
        offset = align(offset, 8)
        if (desc == DESC_U64_DELTA_STR_BOOL) {
            val base = readU64(data, offset)
            offset += 8
            ids.add(base)
            while (ids.size < n) {
                val res = Varint.decode(data, offset)
                offset = res.nextOffset
                val delta = zigzagDecode(res.value)
                ids.add(ids[ids.size - 1] + delta)
            }
        } else {
            for (i in 0 until n) {
                ids.add(readU64(data, offset))
                offset += 8
            }
        }
        offset = align(offset, 4)
        val names = ArrayList<String>(n)
        if (desc == DESC_U64_DICT_STR_BOOL) {
            val dictLen = readU32(data, offset)
            offset += 4
            val offs = IntArray(dictLen + 1)
            for (i in 0..dictLen) {
                offs[i] = readU32(data, offset)
                offset += 4
            }
            val blobLen = offs[dictLen]
            val blob = data.copyOfRange(offset, offset + blobLen)
            offset += blobLen
            val dictionary = Array(dictLen) {
                String(blob, offs[it], offs[it + 1] - offs[it], StandardCharsets.UTF_8)
            }
            offset = align(offset, 4)
            for (i in 0 until n) {
                val code = readU32(data, offset)
                offset += 4
                names.add(dictionary[code])
            }
        } else {
            val offs = IntArray(n + 1)
            for (i in 0..n) {
                offs[i] = readU32(data, offset)
                offset += 4
            }
            val blobLen = offs[n]
            val blob = data.copyOfRange(offset, offset + blobLen)
            offset += blobLen
            for (i in 0 until n) {
                names.add(String(blob, offs[i], offs[i + 1] - offs[i], StandardCharsets.UTF_8))
            }
        }
        val bitBytes = (n + 7) / 8
        val flags = data.copyOfRange(offset, offset + bitBytes)
        val rows = ArrayList<StrBoolRow>(n)
        for (i in 0 until n) {
            val flag = ((flags[i / 8].toInt() shr (i % 8)) and 1) != 0
            rows.add(StrBoolRow(ids[i], names[i], flag))
        }
        return rows
    }

    @JvmStatic
    fun encodeNcbU64EnumBool(rows: List<EnumBoolRow>): ByteArray {
        val useDeltaIds = shouldUseIdDeltaEnum(rows)
        val useNameDict = shouldUseNameDictEnum(rows)
        val useCodeDelta = shouldUseCodeDeltaEnum(rows)
        return encodeNcbU64EnumBool(rows, useDeltaIds, useNameDict, useCodeDelta)
    }

    @JvmStatic
    fun encodeRowsU64EnumBoolAdaptive(rows: List<EnumBoolRow>): ByteArray {
        if (rows.size <= AOS_NCB_SMALL_N) {
            val aos = NoritoAoS.encodeU64EnumBool(rows)
            val ncb = encodeNcbU64EnumBool(rows)
            if (ncb.size < aos.size) return concat(ADAPTIVE_TAG_NCB, ncb)
            return concat(ADAPTIVE_TAG_AOS, aos)
        }
        return concat(ADAPTIVE_TAG_AOS, NoritoAoS.encodeU64EnumBool(rows))
    }

    @JvmStatic
    fun decodeRowsU64EnumBoolAdaptive(payload: ByteArray): List<EnumBoolRow> {
        require(payload.isNotEmpty()) { "Adaptive payload is empty" }
        val tag = payload[0].toInt() and 0xFF
        val body = payload.copyOfRange(1, payload.size)
        return when (tag) {
            ADAPTIVE_TAG_AOS -> NoritoAoS.decodeU64EnumBool(body)
            ADAPTIVE_TAG_NCB -> decodeNcbU64EnumBool(body)
            else -> throw IllegalArgumentException("Unknown adaptive tag: $tag")
        }
    }

    @JvmStatic
    fun decodeNcbU64EnumBool(data: ByteArray): List<EnumBoolRow> {
        require(data.size >= 5) { "NCB enum payload too short" }
        var offset = 0
        val n = readU32(data, offset)
        offset += 4
        val desc = data[offset++].toInt() and 0xFF
        val descriptor = parseEnumDescriptor(desc)
        val ids = ArrayList<Long>(n)
        offset = align(offset, 8)
        if (descriptor.deltaIds) {
            if (n > 0) {
                val base = readU64(data, offset)
                offset += 8
                ids.add(base)
                while (ids.size < n) {
                    val res = Varint.decode(data, offset)
                    offset = res.nextOffset
                    val delta = zigzagDecode(res.value)
                    ids.add(ids[ids.size - 1] + delta)
                }
            }
        } else {
            for (i in 0 until n) {
                ids.add(readU64(data, offset))
                offset += 8
            }
        }
        require(offset + n <= data.size) { "NCB enum payload truncated (tags)" }
        val tags = data.copyOfRange(offset, offset + n)
        offset += n
        var nameCount = 0
        for (tag in tags) {
            val value = tag.toInt() and 0xFF
            if (value == TAG_ENUM_NAME) {
                nameCount += 1
            } else {
                require(value == TAG_ENUM_CODE) { "Invalid enum tag: $value" }
            }
        }
        val codeCount = n - nameCount
        val names = ArrayList<String>(nameCount)
        if (descriptor.nameDict) {
            offset = align(offset, 4)
            require(offset + 4 <= data.size) { "NCB enum payload truncated (dict len)" }
            val dictLen = readU32(data, offset)
            offset += 4
            val offs = IntArray(dictLen + 1)
            for (i in 0..dictLen) {
                require(offset + 4 <= data.size) { "NCB enum payload truncated (dict offsets)" }
                offs[i] = readU32(data, offset)
                offset += 4
            }
            val blobLen = offs[dictLen]
            require(offset + blobLen <= data.size) { "NCB enum payload truncated (dict blob)" }
            val blob = data.copyOfRange(offset, offset + blobLen)
            offset += blobLen
            val dictionary = Array(dictLen) {
                String(blob, offs[it], offs[it + 1] - offs[it], StandardCharsets.UTF_8)
            }
            offset = align(offset, 4)
            for (i in 0 until nameCount) {
                require(offset + 4 <= data.size) { "NCB enum payload truncated (dict codes)" }
                val code = readU32(data, offset)
                offset += 4
                require(code in 0 until dictLen) { "NCB enum payload invalid dict index" }
                names.add(dictionary[code])
            }
        } else {
            offset = align(offset, 4)
            val offs = IntArray(nameCount + 1)
            for (i in 0..nameCount) {
                require(offset + 4 <= data.size) { "NCB enum payload truncated (name offsets)" }
                offs[i] = readU32(data, offset)
                offset += 4
            }
            val blobLen = offs[nameCount]
            require(offset + blobLen <= data.size) { "NCB enum payload truncated (name blob)" }
            val blob = data.copyOfRange(offset, offset + blobLen)
            offset += blobLen
            for (i in 0 until nameCount) {
                names.add(String(blob, offs[i], offs[i + 1] - offs[i], StandardCharsets.UTF_8))
            }
        }
        offset = align(offset, 4)
        val codes = ArrayList<Long>(codeCount)
        if (codeCount > 0) {
            if (descriptor.codeDelta) {
                require(offset + 4 <= data.size) { "NCB enum payload truncated (code base)" }
                val base = Integer.toUnsignedLong(readU32(data, offset))
                offset += 4
                codes.add(base)
                var prev = base
                while (codes.size < codeCount) {
                    val res = Varint.decode(data, offset)
                    offset = res.nextOffset
                    val delta = zigzagDecode(res.value)
                    val next = (prev + delta) and U32_MAX
                    codes.add(next)
                    prev = next
                }
            } else {
                for (i in 0 until codeCount) {
                    require(offset + 4 <= data.size) { "NCB enum payload truncated (codes)" }
                    codes.add(Integer.toUnsignedLong(readU32(data, offset)))
                    offset += 4
                }
            }
        }
        val bitBytes = (n + 7) / 8
        require(offset + bitBytes <= data.size) { "NCB enum payload truncated (flags)" }
        val flags = data.copyOfRange(offset, offset + bitBytes)
        offset += bitBytes
        require(offset == data.size) { "Trailing bytes after enum decode" }
        val rows = ArrayList<EnumBoolRow>(n)
        var nameIndex = 0
        var codeIndex = 0
        for (i in 0 until n) {
            val tag = tags[i].toInt() and 0xFF
            val value: EnumValue = if (tag == TAG_ENUM_NAME) {
                require(nameIndex < names.size) { "Enum name column underflow" }
                EnumName(names[nameIndex++])
            } else if (tag == TAG_ENUM_CODE) {
                require(codeIndex < codes.size) { "Enum code column underflow" }
                EnumCode(codes[codeIndex++])
            } else {
                throw IllegalArgumentException("Invalid enum tag: $tag")
            }
            val flag = ((flags[i / 8].toInt() shr (i % 8)) and 1) != 0
            rows.add(EnumBoolRow(ids[i], value, flag))
        }
        return rows
    }

    @JvmStatic
    fun encodeNcbU64Bytes(rows: List<BytesRow>): ByteArray {
        val out = ByteArrayOutputStream()
        writeU32(out, rows.size)
        out.write(DESC_U64_BYTES)
        padTo(out, 8)
        for (row in rows) {
            writeU64(out, row.id)
        }
        padTo(out, 4)
        val offs = IntArray(rows.size + 1)
        var acc = 0
        val blob = ByteArrayOutputStream()
        offs[0] = 0
        for (i in rows.indices) {
            val value = rows[i].dataRaw()
            acc += value.size
            offs[i + 1] = acc
            blob.write(value)
        }
        for (value in offs) {
            writeU32(out, value)
        }
        out.write(blob.toByteArray())
        return out.toByteArray()
    }

    @JvmStatic
    fun decodeNcbU64Bytes(data: ByteArray): List<BytesRow> {
        require(data.size >= 5) { "NCB payload too short" }
        var offset = 0
        val n = readU32(data, offset)
        offset += 4
        val desc = data[offset++].toInt() and 0xFF
        require(desc == DESC_U64_BYTES) { "Unsupported descriptor 0x${"%02x".format(desc)}" }
        val ids = ArrayList<Long>(n)
        offset = align(offset, 8)
        for (i in 0 until n) {
            ids.add(readU64(data, offset))
            offset += 8
        }
        offset = align(offset, 4)
        val offs = IntArray(n + 1)
        for (i in 0..n) {
            offs[i] = readU32(data, offset)
            offset += 4
        }
        val blobLen = offs[n]
        require(offset + blobLen <= data.size) { "Invalid blob length in columnar payload" }
        val blob = data.copyOfRange(offset, offset + blobLen)
        offset += blobLen
        require(offset == data.size) { "Trailing bytes after columnar decode" }
        val rows = ArrayList<BytesRow>(n)
        for (i in 0 until n) {
            val start = offs[i]
            val end = offs[i + 1]
            require(start <= end && end <= blob.size) { "Invalid offset table in columnar payload" }
            rows.add(BytesRow(ids[i], blob.copyOfRange(start, end)))
        }
        return rows
    }

    @JvmStatic
    fun encodeRowsU64BytesAdaptive(rows: List<BytesRow>): ByteArray {
        if (rows.size <= AOS_NCB_SMALL_N) {
            val aos = NoritoAoS.encodeU64Bytes(rows)
            val ncb = encodeNcbU64Bytes(rows)
            if (ncb.size < aos.size) return concat(ADAPTIVE_TAG_NCB, ncb)
            return concat(ADAPTIVE_TAG_AOS, aos)
        }
        return concat(ADAPTIVE_TAG_AOS, NoritoAoS.encodeU64Bytes(rows))
    }

    @JvmStatic
    fun decodeRowsU64BytesAdaptive(payload: ByteArray): List<BytesRow> {
        require(payload.isNotEmpty()) { "Adaptive payload is empty" }
        val tag = payload[0].toInt() and 0xFF
        val body = payload.copyOfRange(1, payload.size)
        return when (tag) {
            ADAPTIVE_TAG_AOS -> NoritoAoS.decodeU64Bytes(body)
            ADAPTIVE_TAG_NCB -> decodeNcbU64Bytes(body)
            else -> throw IllegalArgumentException("Unknown adaptive tag: $tag")
        }
    }

    @JvmStatic
    fun encodeNcbU64OptionalBytes(rows: List<BytesOptionalRow>): ByteArray {
        val out = ByteArrayOutputStream()
        writeU32(out, rows.size)
        out.write(DESC_U64_OPTIONAL_BYTES)
        padTo(out, 8)
        for (row in rows) {
            writeU64(out, row.id)
        }
        padTo(out, 4)
        val offs = IntArray(rows.size + 1)
        var acc = 0
        val blob = ByteArrayOutputStream()
        offs[0] = 0
        for (i in rows.indices) {
            val value = rows[i].dataRaw()
            if (value != null) {
                acc += value.size
                blob.write(value)
            }
            offs[i + 1] = acc
        }
        for (value in offs) {
            writeU32(out, value)
        }
        out.write(blob.toByteArray())
        out.write(buildPresenceFlags(rows))
        return out.toByteArray()
    }

    @JvmStatic
    fun decodeNcbU64OptionalBytes(data: ByteArray): List<BytesOptionalRow> {
        require(data.size >= 5) { "NCB payload too short" }
        var offset = 0
        val n = readU32(data, offset)
        offset += 4
        val desc = data[offset++].toInt() and 0xFF
        require(desc == DESC_U64_OPTIONAL_BYTES) { "Unsupported descriptor 0x${"%02x".format(desc)}" }
        val ids = ArrayList<Long>(n)
        offset = align(offset, 8)
        for (i in 0 until n) {
            ids.add(readU64(data, offset))
            offset += 8
        }
        offset = align(offset, 4)
        val offs = IntArray(n + 1)
        for (i in 0..n) {
            offs[i] = readU32(data, offset)
            offset += 4
        }
        val blobLen = offs[n]
        require(offset + blobLen <= data.size) { "Invalid blob length in optional columnar payload" }
        val blob = data.copyOfRange(offset, offset + blobLen)
        offset += blobLen
        val bitBytes = (n + 7) / 8
        require(offset + bitBytes <= data.size) { "Optional columnar payload missing presence bitmap" }
        val flags = data.copyOfRange(offset, offset + bitBytes)
        offset += bitBytes
        require(offset == data.size) { "Trailing bytes after optional columnar decode" }
        val rows = ArrayList<BytesOptionalRow>(n)
        for (i in 0 until n) {
            val present = ((flags[i / 8].toInt() shr (i % 8)) and 1) != 0
            val start = offs[i]
            val end = offs[i + 1]
            require(start <= end && end <= blob.size) { "Invalid offset table in optional columnar payload" }
            if (present) {
                rows.add(BytesOptionalRow(ids[i], blob.copyOfRange(start, end)))
            } else {
                require(end == start) { "Absent entry must have zero-length slice" }
                rows.add(BytesOptionalRow(ids[i], null))
            }
        }
        return rows
    }

    private fun concat(tag: Int, payload: ByteArray): ByteArray {
        val out = ByteArray(payload.size + 1)
        out[0] = tag.toByte()
        System.arraycopy(payload, 0, out, 1, payload.size)
        return out
    }

    private fun encodeNcbOffsets(rows: List<StrBoolRow>): ByteArray {
        val out = ByteArrayOutputStream()
        writeU32(out, rows.size)
        out.write(DESC_U64_STR_BOOL)
        padTo(out, 8)
        for (row in rows) {
            writeU64(out, row.id)
        }
        padTo(out, 4)
        val offs = IntArray(rows.size + 1)
        var acc = 0
        val blob = ByteArrayOutputStream()
        offs[0] = 0
        for (i in rows.indices) {
            val encoded = rows[i].name.toByteArray(StandardCharsets.UTF_8)
            acc += encoded.size
            offs[i + 1] = acc
            blob.write(encoded)
        }
        for (value in offs) {
            writeU32(out, value)
        }
        out.write(blob.toByteArray())
        out.write(buildFlags(rows))
        return out.toByteArray()
    }

    private fun encodeNcbDelta(rows: List<StrBoolRow>): ByteArray {
        val out = ByteArrayOutputStream()
        writeU32(out, rows.size)
        out.write(DESC_U64_DELTA_STR_BOOL)
        padTo(out, 8)
        val base = rows[0].id
        writeU64(out, base)
        var prev = base
        for (i in 1 until rows.size) {
            val delta = rows[i].id - prev
            out.write(Varint.encode(zigzagEncode(delta)))
            prev = rows[i].id
        }
        padTo(out, 4)
        val offs = IntArray(rows.size + 1)
        var acc = 0
        val blob = ByteArrayOutputStream()
        offs[0] = 0
        for (i in rows.indices) {
            val encoded = rows[i].name.toByteArray(StandardCharsets.UTF_8)
            acc += encoded.size
            offs[i + 1] = acc
            blob.write(encoded)
        }
        for (value in offs) {
            writeU32(out, value)
        }
        out.write(blob.toByteArray())
        out.write(buildFlags(rows))
        return out.toByteArray()
    }

    private fun encodeNcbDict(rows: List<StrBoolRow>, dict: DictResult): ByteArray {
        val out = ByteArrayOutputStream()
        writeU32(out, rows.size)
        out.write(DESC_U64_DICT_STR_BOOL)
        padTo(out, 8)
        for (row in rows) {
            writeU64(out, row.id)
        }
        padTo(out, 4)
        writeU32(out, dict.dictionary.size)
        val offs = IntArray(dict.dictionary.size + 1)
        var acc = 0
        val blob = ByteArrayOutputStream()
        offs[0] = 0
        for (i in dict.dictionary.indices) {
            val encoded = dict.dictionary[i].toByteArray(StandardCharsets.UTF_8)
            acc += encoded.size
            offs[i + 1] = acc
            blob.write(encoded)
        }
        for (value in offs) {
            writeU32(out, value)
        }
        out.write(blob.toByteArray())
        padTo(out, 4)
        for (row in rows) {
            writeU32(out, dict.mapping[row.name]!!)
        }
        out.write(buildFlags(rows))
        return out.toByteArray()
    }

    private fun encodeNcbU64EnumBool(
        rows: List<EnumBoolRow>,
        useDeltaIds: Boolean,
        useNameDict: Boolean,
        useCodeDelta: Boolean,
    ): ByteArray {
        val out = ByteArrayOutputStream()
        writeU32(out, rows.size)
        val desc = (if (useNameDict) DESC_U64_ENUM_BOOL_DICT else DESC_U64_ENUM_BOOL) or
            (if (useDeltaIds) 0x02 else 0) or
            (if (useCodeDelta) 0x04 else 0)
        out.write(desc)
        padTo(out, 8)
        if (useDeltaIds && rows.isNotEmpty()) {
            val base = rows[0].id
            writeU64(out, base)
            var prev = base
            for (i in 1 until rows.size) {
                val delta = rows[i].id - prev
                out.write(Varint.encode(zigzagEncode(delta)))
                prev = rows[i].id
            }
        } else {
            for (row in rows) {
                writeU64(out, row.id)
            }
        }
        val tags = ByteArray(rows.size)
        val names = ArrayList<String>()
        val codes = ArrayList<Long>()
        for (i in rows.indices) {
            when (val value = rows[i].value) {
                is EnumName -> {
                    tags[i] = TAG_ENUM_NAME.toByte()
                    names.add(value.name)
                }
                is EnumCode -> {
                    tags[i] = TAG_ENUM_CODE.toByte()
                    codes.add(value.code)
                }
            }
        }
        out.write(tags)
        if (useNameDict) {
            val mapping = HashMap<String, Int>()
            val dictionary = ArrayList<String>()
            for (name in names) {
                if (!mapping.containsKey(name)) {
                    mapping[name] = dictionary.size
                    dictionary.add(name)
                }
            }
            padTo(out, 4)
            writeU32(out, dictionary.size)
            val offs = IntArray(dictionary.size + 1)
            var acc = 0
            val blob = ByteArrayOutputStream()
            offs[0] = 0
            for (i in dictionary.indices) {
                val encoded = dictionary[i].toByteArray(StandardCharsets.UTF_8)
                acc += encoded.size
                offs[i + 1] = acc
                blob.write(encoded)
            }
            for (value in offs) {
                writeU32(out, value)
            }
            out.write(blob.toByteArray())
            padTo(out, 4)
            for (name in names) {
                writeU32(out, mapping[name]!!)
            }
        } else {
            padTo(out, 4)
            val offs = IntArray(names.size + 1)
            var acc = 0
            val blob = ByteArrayOutputStream()
            offs[0] = 0
            for (i in names.indices) {
                val encoded = names[i].toByteArray(StandardCharsets.UTF_8)
                acc += encoded.size
                offs[i + 1] = acc
                blob.write(encoded)
            }
            for (value in offs) {
                writeU32(out, value)
            }
            out.write(blob.toByteArray())
        }
        padTo(out, 4)
        if (useCodeDelta && codes.isNotEmpty()) {
            val base = codes[0]
            writeU32(out, base.toInt())
            var prev = base
            for (i in 1 until codes.size) {
                val delta = codes[i] - prev
                out.write(Varint.encode(zigzagEncode(delta)))
                prev = codes[i]
            }
        } else {
            for (code in codes) {
                writeU32(out, code.toInt())
            }
        }
        out.write(buildEnumFlags(rows))
        return out.toByteArray()
    }

    private fun buildFlags(rows: List<StrBoolRow>): ByteArray {
        val bytes = (rows.size + 7) / 8
        val bits = ByteArray(bytes)
        for (i in rows.indices) {
            if (rows[i].flag) {
                bits[i / 8] = (bits[i / 8].toInt() or (1 shl (i % 8))).toByte()
            }
        }
        return bits
    }

    private fun buildEnumFlags(rows: List<EnumBoolRow>): ByteArray {
        val bytes = (rows.size + 7) / 8
        val bits = ByteArray(bytes)
        for (i in rows.indices) {
            if (rows[i].flag) {
                bits[i / 8] = (bits[i / 8].toInt() or (1 shl (i % 8))).toByte()
            }
        }
        return bits
    }

    private fun buildPresenceFlags(rows: List<BytesOptionalRow>): ByteArray {
        val bytes = (rows.size + 7) / 8
        val bits = ByteArray(bytes)
        for (i in rows.indices) {
            if (rows[i].isPresent) {
                bits[i / 8] = (bits[i / 8].toInt() or (1 shl (i % 8))).toByte()
            }
        }
        return bits
    }

    private fun buildDict(rows: List<StrBoolRow>): DictResult {
        if (!COMBO_ENABLE_NAME_DICT || rows.isEmpty()) return DictResult.disabled()
        val mapping = HashMap<String, Int>()
        var totalLen = 0
        for (row in rows) {
            totalLen += row.name.length
            mapping.computeIfAbsent(row.name) { mapping.size }
        }
        val ratio = mapping.size.toDouble() / rows.size
        val avg = totalLen.toDouble() / rows.size
        if (ratio <= COMBO_DICT_RATIO_MAX && avg >= COMBO_DICT_AVG_LEN_MIN) {
            val dictionary = ArrayList<String>(mapping.size)
            repeat(mapping.size) { dictionary.add("") }
            for ((key, value) in mapping) {
                dictionary[value] = key
            }
            return DictResult.enabled(mapping, dictionary)
        }
        return DictResult.disabled()
    }

    private fun shouldUseIdDelta(rows: List<StrBoolRow>): Boolean {
        if (!COMBO_ENABLE_ID_DELTA || rows.size < COMBO_ID_DELTA_MIN_ROWS) return false
        if (rows.size <= COMBO_NO_DELTA_SMALL_N_IF_EMPTY) {
            for (row in rows) {
                if (row.name.isEmpty()) return false
            }
        }
        var prev = rows[0].id
        var varintBytes = 0
        for (i in 1 until rows.size) {
            val delta = rows[i].id - prev
            val zz = zigzagEncode(delta)
            varintBytes += varintLength(zz)
            if (varintBytes >= 8 * (rows.size - 1)) return false
            prev = rows[i].id
        }
        return true
    }

    private fun shouldUseIdDeltaEnum(rows: List<EnumBoolRow>): Boolean {
        if (rows.size < 2) return false
        var prev = rows[0].id
        var varintBytes = 0
        for (i in 1 until rows.size) {
            val delta = rows[i].id - prev
            val zz = zigzagEncode(delta)
            varintBytes += varintLength(zz)
            if (varintBytes >= 8 * (rows.size - 1)) return false
            prev = rows[i].id
        }
        return true
    }

    private fun shouldUseNameDictEnum(rows: List<EnumBoolRow>): Boolean {
        var totalLen = 0
        var nameCount = 0
        val distinct = HashMap<String, Int>()
        for (row in rows) {
            val value = row.value
            if (value is EnumName) {
                totalLen += value.name.length
                nameCount += 1
                distinct.computeIfAbsent(value.name) { distinct.size }
            }
        }
        if (nameCount == 0) return false
        val ratio = distinct.size.toDouble() / nameCount
        val avg = totalLen.toDouble() / nameCount
        return ratio <= COMBO_DICT_RATIO_MAX && avg >= COMBO_DICT_AVG_LEN_MIN
    }

    private fun shouldUseCodeDeltaEnum(rows: List<EnumBoolRow>): Boolean {
        val codes = ArrayList<Long>()
        for (row in rows) {
            val value = row.value
            if (value is EnumCode) {
                codes.add(value.code)
            }
        }
        if (codes.size < 2) return false
        var prev = codes[0]
        var varintBytes = 0
        for (i in 1 until codes.size) {
            val delta = codes[i] - prev
            val zz = zigzagEncode(delta)
            varintBytes += varintLength(zz)
            if (varintBytes >= 4 * (codes.size - 1)) return false
            prev = codes[i]
        }
        return true
    }

    private fun varintLength(value: Long): Int {
        var length = 1
        var v = value
        while (v >= 0x80) {
            v = v ushr 7
            length += 1
        }
        return length
    }

    private fun zigzagEncode(value: Long): Long = (value shl 1) xor (value shr 63)

    private fun zigzagDecode(value: Long): Long = (value ushr 1) xor -(value and 1L)

    private fun writeU32(out: ByteArrayOutputStream, value: Int) {
        out.write(value and 0xFF)
        out.write((value ushr 8) and 0xFF)
        out.write((value ushr 16) and 0xFF)
        out.write((value ushr 24) and 0xFF)
    }

    private fun writeU64(out: ByteArrayOutputStream, value: Long) {
        out.write((value and 0xFF).toInt())
        out.write(((value ushr 8) and 0xFF).toInt())
        out.write(((value ushr 16) and 0xFF).toInt())
        out.write(((value ushr 24) and 0xFF).toInt())
        out.write(((value ushr 32) and 0xFF).toInt())
        out.write(((value ushr 40) and 0xFF).toInt())
        out.write(((value ushr 48) and 0xFF).toInt())
        out.write(((value ushr 56) and 0xFF).toInt())
    }

    private fun padTo(out: ByteArrayOutputStream, alignment: Int) {
        val mis = out.size() % alignment
        if (mis != 0) {
            val pad = alignment - mis
            out.write(ByteArray(pad), 0, pad)
        }
    }

    private fun align(offset: Int, alignment: Int): Int {
        val mis = offset % alignment
        return if (mis == 0) offset else offset + (alignment - mis)
    }

    private fun readU32(data: ByteArray, offset: Int): Int =
        (data[offset].toInt() and 0xFF) or
            ((data[offset + 1].toInt() and 0xFF) shl 8) or
            ((data[offset + 2].toInt() and 0xFF) shl 16) or
            ((data[offset + 3].toInt() and 0xFF) shl 24)

    private fun readU64(data: ByteArray, offset: Int): Long {
        var value = 0L
        for (i in 0 until 8) {
            value = value or ((data[offset + i].toLong() and 0xFF) shl (8 * i))
        }
        return value
    }

    private fun parseEnumDescriptor(desc: Int): EnumDescriptor = when (desc) {
        DESC_U64_ENUM_BOOL -> EnumDescriptor(deltaIds = false, nameDict = false, codeDelta = false)
        DESC_U64_DELTA_ENUM_BOOL -> EnumDescriptor(deltaIds = true, nameDict = false, codeDelta = false)
        DESC_U64_ENUM_BOOL_CODEDELTA -> EnumDescriptor(deltaIds = false, nameDict = false, codeDelta = true)
        DESC_U64_DELTA_ENUM_BOOL_CODEDELTA -> EnumDescriptor(deltaIds = true, nameDict = false, codeDelta = true)
        DESC_U64_ENUM_BOOL_DICT -> EnumDescriptor(deltaIds = false, nameDict = true, codeDelta = false)
        DESC_U64_DELTA_ENUM_BOOL_DICT -> EnumDescriptor(deltaIds = true, nameDict = true, codeDelta = false)
        DESC_U64_ENUM_BOOL_DICT_CODEDELTA -> EnumDescriptor(deltaIds = false, nameDict = true, codeDelta = true)
        DESC_U64_DELTA_ENUM_BOOL_DICT_CODEDELTA -> EnumDescriptor(deltaIds = true, nameDict = true, codeDelta = true)
        else -> throw IllegalArgumentException("Unsupported enum descriptor 0x${"%02x".format(desc)}")
    }

    private data class EnumDescriptor(val deltaIds: Boolean, val nameDict: Boolean, val codeDelta: Boolean)

    private data class DictResult(val useDict: Boolean, val mapping: Map<String, Int>, val dictionary: List<String>) {
        companion object {
            fun enabled(mapping: Map<String, Int>, dictionary: List<String>) =
                DictResult(true, mapping, dictionary)

            fun disabled() = DictResult(false, emptyMap(), emptyList())
        }
    }
}

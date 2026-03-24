// Copyright 2024 Hyperledger Iroha Contributors
// SPDX-License-Identifier: Apache-2.0

package org.hyperledger.iroha.sdk.norito

import java.io.ByteArrayOutputStream
import java.nio.charset.StandardCharsets

private const val VERSION = 0x1

internal object NoritoAoS {

    @JvmStatic
    fun encodeU64StrBool(rows: List<NoritoColumnar.StrBoolRow>): ByteArray {
        val out = ByteArrayOutputStream()
        writeVarint(out, rows.size)
        out.write(VERSION)
        for (row in rows) {
            writeLong(out, row.id)
            val data = row.name.toByteArray(StandardCharsets.UTF_8)
            writeVarint(out, data.size)
            out.writeBytes(data)
            out.write(if (row.flag) 1 else 0)
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun decodeU64StrBool(body: ByteArray): List<NoritoColumnar.StrBoolRow> {
        val lenRes = Varint.decode(body, 0)
        val length = lenRes.value.toInt()
        var offset = lenRes.nextOffset
        val version = body[offset++].toInt() and 0xFF
        require(version == VERSION) { "Unsupported AoS version byte: $version" }
        val rows = ArrayList<NoritoColumnar.StrBoolRow>(length)
        for (i in 0 until length) {
            val id = readLong(body, offset)
            offset += 8
            val nameRes = Varint.decode(body, offset)
            val nameLen = nameRes.value.toInt()
            offset = nameRes.nextOffset
            val name = String(body, offset, nameLen, StandardCharsets.UTF_8)
            offset += nameLen
            val flag = (body[offset++].toInt() and 0xFF) != 0
            rows.add(NoritoColumnar.StrBoolRow(id, name, flag))
        }
        require(offset == body.size) { "Trailing bytes after AoS decode" }
        return rows
    }

    @JvmStatic
    fun encodeU64Bytes(rows: List<NoritoColumnar.BytesRow>): ByteArray {
        val out = ByteArrayOutputStream()
        writeVarint(out, rows.size)
        out.write(VERSION)
        for (row in rows) {
            writeLong(out, row.id)
            val data = row.data
            writeVarint(out, data.size)
            out.writeBytes(data)
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun decodeU64Bytes(body: ByteArray): List<NoritoColumnar.BytesRow> {
        val lenRes = Varint.decode(body, 0)
        val length = lenRes.value.toInt()
        var offset = lenRes.nextOffset
        val version = body[offset++].toInt() and 0xFF
        require(version == VERSION) { "Unsupported AoS version byte: $version" }
        val rows = ArrayList<NoritoColumnar.BytesRow>(length)
        for (i in 0 until length) {
            val id = readLong(body, offset)
            offset += 8
            val lenVal = Varint.decode(body, offset)
            val dataLen = lenVal.value.toInt()
            offset = lenVal.nextOffset
            require(offset + dataLen <= body.size) { "AoS bytes row exceeds payload bounds" }
            val data = ByteArray(dataLen)
            System.arraycopy(body, offset, data, 0, dataLen)
            offset += dataLen
            rows.add(NoritoColumnar.BytesRow(id, data))
        }
        require(offset == body.size) { "Trailing bytes after AoS decode" }
        return rows
    }

    @JvmStatic
    fun encodeU64OptionalBytes(rows: List<NoritoColumnar.BytesOptionalRow>): ByteArray {
        val out = ByteArrayOutputStream()
        writeVarint(out, rows.size)
        out.write(VERSION)
        for (row in rows) {
            writeLong(out, row.id)
            val data = row.data
            if (data == null) {
                out.write(0)
            } else {
                out.write(1)
                writeVarint(out, data.size)
                out.writeBytes(data)
            }
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun decodeU64OptionalBytes(body: ByteArray): List<NoritoColumnar.BytesOptionalRow> {
        val lenRes = Varint.decode(body, 0)
        val length = lenRes.value.toInt()
        var offset = lenRes.nextOffset
        val version = body[offset++].toInt() and 0xFF
        require(version == VERSION) { "Unsupported AoS version byte: $version" }
        val rows = ArrayList<NoritoColumnar.BytesOptionalRow>(length)
        for (i in 0 until length) {
            val id = readLong(body, offset)
            offset += 8
            val present = body[offset++].toInt() and 0xFF
            when (present) {
                0 -> rows.add(NoritoColumnar.BytesOptionalRow(id, null))
                1 -> {
                    val lenVal = Varint.decode(body, offset)
                    val dataLen = lenVal.value.toInt()
                    offset = lenVal.nextOffset
                    require(offset + dataLen <= body.size) {
                        "AoS optional bytes row exceeds payload bounds"
                    }
                    val data = ByteArray(dataLen)
                    System.arraycopy(body, offset, data, 0, dataLen)
                    offset += dataLen
                    rows.add(NoritoColumnar.BytesOptionalRow(id, data))
                }
                else -> throw IllegalArgumentException(
                    "Invalid presence flag in AoS optional bytes payload"
                )
            }
        }
        require(offset == body.size) { "Trailing bytes after AoS decode" }
        return rows
    }

    @JvmStatic
    fun encodeU64EnumBool(rows: List<NoritoColumnar.EnumBoolRow>): ByteArray {
        val out = ByteArrayOutputStream()
        // Historical enum AoS layout omits the version byte to preserve golden payloads.
        writeVarint(out, rows.size)
        for (row in rows) {
            writeLong(out, row.id)
            when (val value = row.value) {
                is NoritoColumnar.EnumName -> {
                    out.write(0)
                    val data = value.name.toByteArray(StandardCharsets.UTF_8)
                    writeVarint(out, data.size)
                    out.writeBytes(data)
                }
                is NoritoColumnar.EnumCode -> {
                    out.write(1)
                    writeU32(out, value.code)
                }
                else -> throw IllegalArgumentException(
                    "Unsupported enum value type: ${value.javaClass}"
                )
            }
            out.write(if (row.flag) 1 else 0)
        }
        return out.toByteArray()
    }

    @JvmStatic
    fun decodeU64EnumBool(body: ByteArray): List<NoritoColumnar.EnumBoolRow> {
        val lenRes = Varint.decode(body, 0)
        val length = lenRes.value.toInt()
        var offset = lenRes.nextOffset
        val rows = ArrayList<NoritoColumnar.EnumBoolRow>(length)
        for (i in 0 until length) {
            require(offset + 8 <= body.size) { "AoS enum payload truncated (id)" }
            val id = readLong(body, offset)
            offset += 8
            require(offset < body.size) { "AoS enum payload truncated (tag)" }
            val tag = body[offset++].toInt() and 0xFF
            val value: NoritoColumnar.EnumValue = when (tag) {
                0 -> {
                    val nameRes = Varint.decode(body, offset)
                    val nameLen = nameRes.value.toInt()
                    offset = nameRes.nextOffset
                    require(offset + nameLen <= body.size) {
                        "AoS enum payload truncated (name)"
                    }
                    val name = String(body, offset, nameLen, StandardCharsets.UTF_8)
                    offset += nameLen
                    NoritoColumnar.EnumName(name)
                }
                1 -> {
                    require(offset + 4 <= body.size) { "AoS enum payload truncated (code)" }
                    val code = Integer.toUnsignedLong(readU32(body, offset))
                    offset += 4
                    NoritoColumnar.EnumCode(code)
                }
                else -> throw IllegalArgumentException("Invalid enum tag: $tag")
            }
            require(offset < body.size) { "AoS enum payload truncated (flag)" }
            val flag = (body[offset++].toInt() and 0xFF) != 0
            rows.add(NoritoColumnar.EnumBoolRow(id, value, flag))
        }
        require(offset == body.size) { "Trailing bytes after AoS enum decode" }
        return rows
    }

    private fun writeVarint(out: ByteArrayOutputStream, value: Int) {
        out.writeBytes(Varint.encode(value.toLong()))
    }

    private fun writeLong(out: ByteArrayOutputStream, value: Long) {
        out.write((value and 0xFF).toInt())
        out.write(((value ushr 8) and 0xFF).toInt())
        out.write(((value ushr 16) and 0xFF).toInt())
        out.write(((value ushr 24) and 0xFF).toInt())
        out.write(((value ushr 32) and 0xFF).toInt())
        out.write(((value ushr 40) and 0xFF).toInt())
        out.write(((value ushr 48) and 0xFF).toInt())
        out.write(((value ushr 56) and 0xFF).toInt())
    }

    private fun writeU32(out: ByteArrayOutputStream, value: Long) {
        out.write((value and 0xFF).toInt())
        out.write(((value ushr 8) and 0xFF).toInt())
        out.write(((value ushr 16) and 0xFF).toInt())
        out.write(((value ushr 24) and 0xFF).toInt())
    }

    private fun readLong(data: ByteArray, offset: Int): Long {
        var value = 0L
        for (i in 0 until 8) {
            value = value or ((data[offset + i].toLong() and 0xFF) shl (8 * i))
        }
        return value
    }

    private fun readU32(data: ByteArray, offset: Int): Int =
        (data[offset].toInt() and 0xFF) or
            ((data[offset + 1].toInt() and 0xFF) shl 8) or
            ((data[offset + 2].toInt() and 0xFF) shl 16) or
            ((data[offset + 3].toInt() and 0xFF) shl 24)
}
